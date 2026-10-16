mod gitlab;
mod mcp;
mod ui_server;

use std::path::PathBuf;

use anyhow::{Context, Result};
use board_core::{
    AssigneeKind, BoardStore, ClaimCard, CreateCard, LinkType, ListFilter, Status, UpdateCard,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "board", about = "Agent-friendly kanban board CLI", version)]
struct Cli {
    /// Path to SQLite database (default: platform data dir /board/board.db)
    #[arg(long, global = true, env = "BOARD_DB")]
    db: Option<PathBuf>,

    /// Emit JSON instead of human text
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show default database path
    Path,
    /// Seed a few demo cards if the board is empty
    Seed,
    /// Create a card
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long = "label", value_delimiter = ',')]
        labels: Vec<String>,
        #[arg(long)]
        assignee: Option<AssigneeCli>,
        #[arg(long)]
        assignee_name: Option<String>,
        #[arg(long)]
        priority: Option<i32>,
        /// Link to an external issue / MR (e.g. GitLab)
        #[arg(long)]
        external_url: Option<String>,
        #[arg(long, default_value = "human")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// List cards
    Ls {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        assignee: Option<AssigneeCli>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long, short = 'q')]
        query: Option<String>,
    },
    /// Full-text search
    Search {
        query: String,
        #[arg(long)]
        status: Option<String>,
    },
    /// Get one card (with links + recent events)
    Get {
        id: String,
        #[arg(long, default_value_t = 20)]
        events: usize,
    },
    /// Move a card to another column
    Move {
        id: String,
        #[arg(long = "to")]
        to: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, default_value = "agent")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// Update card fields
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long = "label")]
        labels: Option<Vec<String>>,
        #[arg(long)]
        assignee: Option<AssigneeCli>,
        #[arg(long)]
        assignee_name: Option<String>,
        #[arg(long)]
        priority: Option<i32>,
        #[arg(long)]
        external_url: Option<String>,
        #[arg(long, default_value = "human")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// Delete a card
    Rm {
        id: String,
        #[arg(long, default_value = "human")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// Link two cards
    Link {
        from: String,
        to: String,
        #[arg(long = "type", default_value = "relates")]
        link_type: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, default_value = "agent")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// Show board columns snapshot
    Board,
    /// Show activity timeline
    Timeline {
        #[arg(long)]
        card: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Export cards/links/events as JSON
    Export,
    /// Serve the embedded kanban UI (also works as the bundled desktop surface)
    Ui {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 7841)]
        port: u16,
    },
    /// Run MCP stdio server (primary agent path; see docs/mcp-tools.md)
    Mcp,
    /// Claim a ready card (lease + move to doing)
    Claim {
        id: String,
        #[arg(long, default_value_t = 60)]
        ttl_minutes: i64,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, default_value = "agent")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// Release a claim back to ready/blocked
    Release {
        id: String,
        #[arg(long = "to", default_value = "ready")]
        to: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, default_value = "agent")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// Append a timeline note
    Note {
        id: String,
        note: String,
        #[arg(long, default_value = "agent")]
        actor: String,
        #[arg(long)]
        actor_name: Option<String>,
    },
    /// GitLab issue import / sync (see docs/gitlab.md)
    Gitlab {
        #[command(subcommand)]
        command: GitlabCmd,
    },
}

#[derive(Subcommand, Debug)]
enum GitlabCmd {
    /// Verify GITLAB_TOKEN against the API
    Doctor,
    /// Import issues from a GitLab project into the board
    Import {
        /// `group/project` or numeric project id
        project: String,
        /// opened | closed | all
        #[arg(long, default_value = "opened")]
        state: String,
        /// Only issues with this GitLab label
        #[arg(long)]
        label: Option<String>,
        /// Board status for open issues (default: ready)
        #[arg(long, default_value = "ready")]
        open_status: String,
        /// Preview without writing
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value = "gitlab")]
        actor_name: String,
    },
    /// Refresh cards already linked via external_url to this project
    Sync {
        project: String,
        #[arg(long, default_value = "ready")]
        open_status: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value = "gitlab")]
        actor_name: String,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum AssigneeCli {
    Unassigned,
    Human,
    Agent,
}

impl From<AssigneeCli> for AssigneeKind {
    fn from(v: AssigneeCli) -> Self {
        match v {
            AssigneeCli::Unassigned => AssigneeKind::Unassigned,
            AssigneeCli::Human => AssigneeKind::Human,
            AssigneeCli::Agent => AssigneeKind::Agent,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Ui { host, port } = &cli.command {
        let rt = tokio::runtime::Runtime::new().context("tokio runtime")?;
        return rt.block_on(ui_server::run(cli.db.clone(), host, *port));
    }
    if matches!(cli.command, Commands::Mcp) {
        let rt = tokio::runtime::Runtime::new().context("tokio runtime")?;
        return rt.block_on(mcp::run(cli.db.clone()));
    }

    let store = match &cli.db {
        Some(p) => BoardStore::open(p).with_context(|| format!("open db {}", p.display()))?,
        None => BoardStore::open_default().context("open default db")?,
    };

    match cli.command {
        Commands::Path => {
            let p = cli.db.unwrap_or_else(BoardStore::path_hint);
            print_out(&cli.json, &serde_json::json!({ "path": p }), || {
                println!("{}", p.display());
                Ok(())
            })?;
        }
        Commands::Seed => {
            let cards = store.seed_demo()?;
            print_out(&cli.json, &cards, || {
                println!("seeded {} cards", cards.len());
                for c in &cards {
                    println!("{} [{}] {}", c.key, c.status, c.title);
                }
                Ok(())
            })?;
        }
        Commands::Create {
            title,
            body,
            status,
            labels,
            assignee,
            assignee_name,
            priority,
            external_url,
            actor,
            actor_name,
        } => {
            let card = store.create_card(CreateCard {
                title,
                body,
                status: status.as_deref().map(Status::parse).transpose()?,
                labels,
                assignee_kind: assignee.map(Into::into),
                assignee_name,
                priority,
                external_url,
                actor_kind: Some(actor),
                actor_name,
            })?;
            print_out(&cli.json, &card, || {
                println!("{} created in {}", card.key, card.status);
                Ok(())
            })?;
        }
        Commands::Ls {
            status,
            assignee,
            label,
            query,
        } => {
            let filter = ListFilter {
                status: status.as_deref().map(Status::parse).transpose()?,
                assignee_kind: assignee.map(Into::into),
                label,
                query,
            };
            let cards = store.list_cards(&filter)?;
            print_out(&cli.json, &cards, || {
                if cards.is_empty() {
                    println!("(no cards)");
                }
                for c in &cards {
                    let who = match c.assignee_kind {
                        AssigneeKind::Unassigned => "-".into(),
                        AssigneeKind::Human => format!(
                            "human:{}",
                            c.assignee_name.clone().unwrap_or_else(|| "?".into())
                        ),
                        AssigneeKind::Agent => format!(
                            "agent:{}",
                            c.assignee_name.clone().unwrap_or_else(|| "?".into())
                        ),
                    };
                    println!(
                        "{:<8} {:<8} {:>2}  {:<16} {}",
                        c.key, c.status, c.priority, who, c.title
                    );
                }
                Ok(())
            })?;
        }
        Commands::Search { query, status } => {
            let filter = ListFilter {
                status: status.as_deref().map(Status::parse).transpose()?,
                query: Some(query.clone()),
                ..Default::default()
            };
            let cards = store.search(&query, &filter)?;
            print_out(&cli.json, &cards, || {
                for c in &cards {
                    println!("{} [{}] {}", c.key, c.status, c.title);
                }
                Ok(())
            })?;
        }
        Commands::Get { id, events } => {
            let detail = store.get_card_detail(&id, events)?;
            print_out(&cli.json, &detail, || {
                let c = &detail.card;
                println!("{}  {}", c.key, c.title);
                println!("status: {}  priority: {}", c.status, c.priority);
                println!("labels: {}", c.labels.join(", "));
                if !c.body.is_empty() {
                    println!("\n{}\n", c.body);
                }
                if !detail.links.is_empty() {
                    println!("links:");
                    for l in &detail.links {
                        println!(
                            "  {} -> {} ({}){}",
                            l.from_id,
                            l.to_id,
                            l.link_type.as_str(),
                            l.note
                                .as_ref()
                                .map(|n| format!(" — {n}"))
                                .unwrap_or_default()
                        );
                    }
                }
                if !detail.recent_events.is_empty() {
                    println!("timeline:");
                    for e in &detail.recent_events {
                        println!(
                            "  {}  {}/{}  {}",
                            e.at.to_rfc3339(),
                            e.actor_kind,
                            e.actor_name.as_deref().unwrap_or("-"),
                            e.kind
                        );
                    }
                }
                Ok(())
            })?;
        }
        Commands::Move {
            id,
            to,
            note,
            actor,
            actor_name,
        } => {
            let card = store.move_card(
                &id,
                Status::parse(&to)?,
                note.as_deref(),
                &actor,
                actor_name.as_deref(),
            )?;
            print_out(&cli.json, &card, || {
                println!("{} -> {}", card.key, card.status);
                Ok(())
            })?;
        }
        Commands::Update {
            id,
            title,
            body,
            labels,
            assignee,
            assignee_name,
            priority,
            external_url,
            actor,
            actor_name,
        } => {
            let card = store.update_card(
                &id,
                UpdateCard {
                    title,
                    body,
                    labels,
                    assignee_kind: assignee.map(Into::into),
                    assignee_name,
                    priority,
                    external_url,
                    actor_kind: Some(actor),
                    actor_name,
                },
            )?;
            print_out(&cli.json, &card, || {
                println!("{} updated", card.key);
                Ok(())
            })?;
        }
        Commands::Rm {
            id,
            actor,
            actor_name,
        } => {
            store.delete_card(&id, &actor, actor_name.as_deref())?;
            print_out(
                &cli.json,
                &serde_json::json!({ "deleted": id }),
                || {
                    println!("deleted {id}");
                    Ok(())
                },
            )?;
        }
        Commands::Link {
            from,
            to,
            link_type,
            note,
            actor,
            actor_name,
        } => {
            let link = store.link_cards(
                &from,
                &to,
                LinkType::parse(&link_type)?,
                note.as_deref(),
                &actor,
                actor_name.as_deref(),
            )?;
            print_out(&cli.json, &link, || {
                println!(
                    "linked {} -> {} ({})",
                    link.from_id,
                    link.to_id,
                    link.link_type.as_str()
                );
                Ok(())
            })?;
        }
        Commands::Board => {
            let snap = store.board_snapshot()?;
            print_out(&cli.json, &snap, || {
                for col in &snap.columns {
                    println!("## {}", col.status);
                    for c in &col.cards {
                        println!("  - {} {}", c.key, c.title);
                    }
                }
                Ok(())
            })?;
        }
        Commands::Timeline { card, limit } => {
            let events = store.list_events(card.as_deref(), limit)?;
            print_out(&cli.json, &events, || {
                for e in &events {
                    println!(
                        "{}  {:<8}  {}  {}",
                        e.at.to_rfc3339(),
                        e.kind,
                        e.card_id.as_deref().unwrap_or("-"),
                        e.payload
                    );
                }
                Ok(())
            })?;
        }
        Commands::Export => {
            let data = store.export_json()?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        Commands::Claim {
            id,
            ttl_minutes,
            note,
            actor,
            actor_name,
        } => {
            let card = store.claim_card(
                &id,
                ClaimCard {
                    ttl_minutes,
                    note,
                    actor_kind: actor,
                    actor_name,
                },
            )?;
            print_out(&cli.json, &card, || {
                println!(
                    "{} claimed → {} (lease {:?})",
                    card.key, card.status, card.claim_expires_at
                );
                Ok(())
            })?;
        }
        Commands::Release {
            id,
            to,
            note,
            actor,
            actor_name,
        } => {
            let card = store.release_card(
                &id,
                Status::parse(&to)?,
                note.as_deref(),
                &actor,
                actor_name.as_deref(),
            )?;
            print_out(&cli.json, &card, || {
                println!("{} released → {}", card.key, card.status);
                Ok(())
            })?;
        }
        Commands::Note {
            id,
            note,
            actor,
            actor_name,
        } => {
            let card = store.add_note(&id, &note, &actor, actor_name.as_deref())?;
            print_out(&cli.json, &card, || {
                println!("{} note added", card.key);
                Ok(())
            })?;
        }
        Commands::Gitlab { command } => match command {
            GitlabCmd::Doctor => {
                let info = gitlab::doctor_from_settings(&store)
                    .or_else(|_| {
                        let client =
                            gitlab::GitLabClient::new(gitlab::GitLabConfig::from_env()?);
                        gitlab::doctor(&client)
                    })?;
                print_out(&cli.json, &info, || {
                    println!(
                        "ok  {}  @{}",
                        info["base_url"].as_str().unwrap_or("?"),
                        info["user"]["username"].as_str().unwrap_or("?")
                    );
                    Ok(())
                })?;
            }
            GitlabCmd::Import {
                project,
                state,
                label,
                open_status,
                dry_run,
                actor_name,
            } => {
                let settings = store.gitlab_settings()?;
                let client = gitlab::GitLabClient::new(settings.to_client_config().or_else(|_| {
                    gitlab::GitLabConfig::from_env()
                })?);
                let report = gitlab::import_issues(
                    &store,
                    &client,
                    gitlab::ImportOpts {
                        project: &project,
                        state: gitlab::IssueStateFilter::parse(&state)?,
                        label: label.as_deref(),
                        open_status: Status::parse(&open_status)?,
                        dry_run,
                        actor_name: Some(&actor_name),
                    },
                )?;
                print_out(&cli.json, &report, || {
                    println!(
                        "{}  created={} updated={} unchanged={}",
                        report.project, report.created, report.updated, report.unchanged
                    );
                    for item in &report.items {
                        println!(
                            "  {:?}  #{}  {}  {}",
                            item.action,
                            item.issue_iid,
                            item.card_key.as_deref().unwrap_or("-"),
                            item.title
                        );
                    }
                    Ok(())
                })?;
            }
            GitlabCmd::Sync {
                project,
                open_status,
                dry_run,
                actor_name,
            } => {
                let settings = store.gitlab_settings()?;
                let client = gitlab::GitLabClient::new(settings.to_client_config().or_else(|_| {
                    gitlab::GitLabConfig::from_env()
                })?);
                let report = gitlab::sync_linked(
                    &store,
                    &client,
                    &project,
                    Status::parse(&open_status)?,
                    dry_run,
                    Some(&actor_name),
                )?;
                print_out(&cli.json, &report, || {
                    println!(
                        "{}  updated={} unchanged={}",
                        report.project, report.updated, report.unchanged
                    );
                    for item in &report.items {
                        println!(
                            "  {:?}  #{}  {}  {}",
                            item.action,
                            item.issue_iid,
                            item.card_key.as_deref().unwrap_or("-"),
                            item.title
                        );
                    }
                    Ok(())
                })?;
            }
        },
        Commands::Ui { .. } | Commands::Mcp => unreachable!("handled before store open"),
    }

    Ok(())
}

fn print_out<T: Serialize>(
    as_json: &bool,
    value: &T,
    human: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if *as_json {
        println!("{}", serde_json::to_string_pretty(value)?);
        Ok(())
    } else {
        human()
    }
}