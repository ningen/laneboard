//! stdio MCP server — primary agent path for sandboxed Claude Code.
//! See `docs/mcp-tools.md`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use board_core::{
    AssigneeKind, BoardStore, ClaimCard, CreateCard, Error as BoardError, LinkType, ListFilter,
    Status, UpdateCard,
};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone)]
struct Actor {
    kind: String,
    name: Option<String>,
}

impl Actor {
    fn from_env() -> Self {
        let kind = std::env::var("BOARD_ACTOR_KIND").unwrap_or_else(|_| "agent".into());
        let name = std::env::var("BOARD_ACTOR_NAME").ok().filter(|s| !s.is_empty());
        Self { kind, name }
    }
}

#[derive(Clone)]
pub struct BoardMcp {
    store: Arc<Mutex<BoardStore>>,
    actor: Actor,
    tool_router: ToolRouter<Self>,
}

fn json_ok(value: Value) -> Result<CallToolResult, McpError> {
    let body = json!({ "ok": true }).as_object().cloned().unwrap();
    let mut out = Value::Object(body);
    if let Value::Object(map) = value {
        for (k, v) in map {
            out[k] = v;
        }
    } else {
        out["data"] = value;
    }
    Ok(CallToolResult::success(vec![ContentBlock::text(
        out.to_string(),
    )]))
}

fn json_err(err: BoardError) -> Result<CallToolResult, McpError> {
    let body = json!({
        "ok": false,
        "error": {
            "code": err.code(),
            "message": err.to_string(),
        }
    });
    Ok(CallToolResult::success(vec![ContentBlock::text(
        body.to_string(),
    )]))
}

fn map_store<T: serde::Serialize>(
    result: board_core::Result<T>,
    wrap: impl FnOnce(T) -> Value,
) -> Result<CallToolResult, McpError> {
    match result {
        Ok(v) => json_ok(wrap(v)),
        Err(e) => json_err(e),
    }
}

fn parse_status(s: &str) -> board_core::Result<Status> {
    Status::parse(s)
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListArgs {
    /// Column filter. Omit for claimable ready cards.
    #[serde(default)]
    status: Option<String>,
    /// unassigned | human | agent | me
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CardArgs {
    /// CARD-n or UUID
    card: String,
    #[serde(default)]
    events: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ClaimArgs {
    card: String,
    #[serde(default)]
    ttl_minutes: Option<i64>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReleaseArgs {
    card: String,
    /// ready | blocked
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MoveArgs {
    card: String,
    to: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct NoteArgs {
    card: String,
    note: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateArgs {
    card: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    labels: Option<Vec<String>>,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    external_url: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LinkArgs {
    from: String,
    to: String,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateArgs {
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    labels: Option<Vec<String>>,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    external_url: Option<String>,
}

#[tool_router(router = tool_router)]
impl BoardMcp {
    fn new(store: BoardStore, actor: Actor) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            actor,
            tool_router: Self::tool_router(),
        }
    }

    fn with_store<R>(&self, f: impl FnOnce(&BoardStore) -> board_core::Result<R>) -> board_core::Result<R> {
        let guard = self
            .store
            .lock()
            .map_err(|_| BoardError::msg("board store lock poisoned"))?;
        f(&guard)
    }

    #[tool(
        name = "board_list",
        description = "List Laneboard cards. Default: status=ready and unassigned/expired claim. Use before board_claim.",
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    fn board_list(&self, Parameters(args): Parameters<ListArgs>) -> Result<CallToolResult, McpError> {
        let limit = args.limit.unwrap_or(20).clamp(1, 100) as usize;
        let result = self.with_store(|store| {
            // Default: claimable ready
            if args.status.is_none()
                && args.assignee.is_none()
                && args.label.is_none()
                && args.query.is_none()
            {
                return store.list_claimable(limit);
            }
            let status = args
                .status
                .as_deref()
                .map(parse_status)
                .transpose()?;
            let assignee_kind = match args.assignee.as_deref() {
                None => None,
                Some("me") => Some(AssigneeKind::Agent),
                Some(s) => Some(AssigneeKind::parse(s)),
            };
            let mut cards = store.list_cards(&ListFilter {
                status,
                assignee_kind,
                label: args.label.clone(),
                query: args.query.clone(),
            })?;
            if args.assignee.as_deref() == Some("me") {
                let name = self.actor.name.clone();
                cards.retain(|c| {
                    matches!(c.assignee_kind, AssigneeKind::Agent)
                        && c.assignee_name == name
                });
            }
            cards.truncate(limit);
            Ok(cards)
        });
        map_store(result, |cards| json!({ "cards": cards }))
    }

    #[tool(
        name = "board_get",
        description = "Get one card with body, links, lease, and recent events. Pass CARD-n or UUID.",
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    fn board_get(&self, Parameters(args): Parameters<CardArgs>) -> Result<CallToolResult, McpError> {
        let events = args.events.unwrap_or(20).min(100) as usize;
        let result = self.with_store(|s| s.get_card_detail(&args.card, events));
        map_store(result, |detail| {
            json!({
                "card": detail.card,
                "links": detail.links.iter().map(|l| json!({
                    "from": l.from_id,
                    "to": l.to_id,
                    "type": l.link_type.as_str(),
                    "note": l.note,
                })).collect::<Vec<_>>(),
                "recent_events": detail.recent_events,
                "lease": detail.lease,
            })
        })
    }

    #[tool(
        name = "board_claim",
        description = "Claim a ready card for this agent: assign to me, move to doing, start lease. Fails if another actor holds a live lease.",
        annotations(read_only_hint = false, idempotent_hint = false, destructive_hint = false)
    )]
    fn board_claim(
        &self,
        Parameters(args): Parameters<ClaimArgs>,
    ) -> Result<CallToolResult, McpError> {
        let actor = self.actor.clone();
        let result = self.with_store(|s| {
            let card = s.claim_card(
                &args.card,
                ClaimCard {
                    ttl_minutes: args.ttl_minutes.unwrap_or(60),
                    note: args.note.clone(),
                    actor_kind: actor.kind.clone(),
                    actor_name: actor.name.clone(),
                },
            )?;
            let lease = BoardStore::lease_of(&card);
            Ok((card, lease))
        });
        map_store(result, |(card, lease)| json!({ "card": card, "lease": lease }))
    }

    #[tool(
        name = "board_release",
        description = "Release this agent's claim. Use when stopping work early. Default status=ready.",
        annotations(read_only_hint = false, idempotent_hint = true, destructive_hint = false)
    )]
    fn board_release(
        &self,
        Parameters(args): Parameters<ReleaseArgs>,
    ) -> Result<CallToolResult, McpError> {
        let actor = self.actor.clone();
        let result = self.with_store(|s| {
            let to = match args.to.as_deref() {
                None | Some("ready") => Status::Ready,
                Some("blocked") => Status::Blocked,
                Some(other) => {
                    return Err(BoardError::Validation(format!(
                        "release to must be ready or blocked, got {other}"
                    )))
                }
            };
            s.release_card(
                &args.card,
                to,
                args.note.as_deref(),
                &actor.kind,
                actor.name.as_deref(),
            )
        });
        map_store(result, |card| json!({ "card": card }))
    }

    #[tool(
        name = "board_move",
        description = "Move a card to another column. For completion use to=done with a short note. Prefer board_claim before to=doing.",
        annotations(read_only_hint = false, idempotent_hint = false, destructive_hint = false)
    )]
    fn board_move(&self, Parameters(args): Parameters<MoveArgs>) -> Result<CallToolResult, McpError> {
        let actor = self.actor.clone();
        let result = self.with_store(|s| {
            let to = parse_status(&args.to)?;
            s.move_card(
                &args.card,
                to,
                args.note.as_deref(),
                &actor.kind,
                actor.name.as_deref(),
            )
        });
        map_store(result, |card| json!({ "card": card }))
    }

    #[tool(
        name = "board_note",
        description = "Append a progress/blocker note to the card timeline. Does not edit the body.",
        annotations(read_only_hint = false, idempotent_hint = false, destructive_hint = false)
    )]
    fn board_note(&self, Parameters(args): Parameters<NoteArgs>) -> Result<CallToolResult, McpError> {
        let actor = self.actor.clone();
        let result = self.with_store(|s| {
            s.add_note(
                &args.card,
                &args.note,
                &actor.kind,
                actor.name.as_deref(),
            )
        });
        map_store(result, |card| json!({ "card": card }))
    }

    #[tool(
        name = "board_update",
        description = "Update card fields (title, body, labels, priority, external_url). Do not use for status changes.",
        annotations(read_only_hint = false, idempotent_hint = false, destructive_hint = false)
    )]
    fn board_update(
        &self,
        Parameters(args): Parameters<UpdateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let actor = self.actor.clone();
        let result = self.with_store(|s| {
            s.update_card(
                &args.card,
                UpdateCard {
                    title: args.title.clone(),
                    body: args.body.clone(),
                    labels: args.labels.clone(),
                    priority: args.priority,
                    external_url: args.external_url.clone(),
                    actor_kind: Some(actor.kind.clone()),
                    actor_name: actor.name.clone(),
                    ..Default::default()
                },
            )
        });
        map_store(result, |card| json!({ "card": card }))
    }

    #[tool(
        name = "board_link",
        description = "Link two cards. Use type=blocks when from must finish before to. Inverse links are stored automatically.",
        annotations(read_only_hint = false, idempotent_hint = true, destructive_hint = false)
    )]
    fn board_link(&self, Parameters(args): Parameters<LinkArgs>) -> Result<CallToolResult, McpError> {
        let actor = self.actor.clone();
        let result = self.with_store(|s| {
            let link_type = LinkType::parse(args.r#type.as_deref().unwrap_or("relates"))?;
            s.link_cards(
                &args.from,
                &args.to,
                link_type,
                args.note.as_deref(),
                &actor.kind,
                actor.name.as_deref(),
            )
        });
        map_store(result, |link| json!({ "link": link }))
    }

    #[tool(
        name = "board_create",
        description = "Create a card. Default status=backlog. Use when splitting work; link back to the parent with board_link.",
        annotations(read_only_hint = false, idempotent_hint = false, destructive_hint = false)
    )]
    fn board_create(
        &self,
        Parameters(args): Parameters<CreateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let actor = self.actor.clone();
        let result = self.with_store(|s| {
            let status = args
                .status
                .as_deref()
                .map(parse_status)
                .transpose()?;
            s.create_card(CreateCard {
                title: args.title.clone(),
                body: args.body.clone(),
                status,
                labels: args.labels.clone().unwrap_or_default(),
                priority: args.priority,
                external_url: args.external_url.clone(),
                actor_kind: Some(actor.kind.clone()),
                actor_name: actor.name.clone(),
                ..Default::default()
            })
        });
        map_store(result, |card| json!({ "card": card }))
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "laneboard",
    version = "0.1.0",
    instructions = "Laneboard agent tools. Session recipe: board_list → board_claim → board_get → work → board_note → board_move(done|blocked). Actor comes from BOARD_ACTOR_NAME. Do not shell out to `board` when these tools are available."
)]
impl ServerHandler for BoardMcp {}

pub async fn run(db: Option<PathBuf>) -> anyhow::Result<()> {
    // Keep MCP logs off stdout (protocol uses stdio).
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();

    let store = match db {
        Some(p) => BoardStore::open(p)?,
        None => BoardStore::open_default()?,
    };
    let actor = Actor::from_env();
    let server = BoardMcp::new(store, actor);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
