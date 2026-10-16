# MCP tool surface (agent DX)

Laneboard MCP is the **primary agent path** for Claude Code with sandbox enabled.
Bash `board …` remains a fallback; MCP runs on the host (outside sandboxed Bash) and
talks to the same SQLite store as the desktop app.

## Goals

- Small tool set a coding agent can use without a handbook
- Workflow verbs: **list → claim → work → note → move**
- Structured JSON in / out; card identity always includes `key` (`CARD-n`)
- Actor identity comes from **server config**, not from the model guessing names
- Safe defaults for sandbox / multi-session work (claim lease, no delete)

## Non-goals (v1)

- Spawning or supervising Claude Code / other agent CLIs from the board
- Network webhooks or cloud sync
- Full CRUD parity with the human CLI (`rm`, `export`, `ui`)

## Runtime assumptions

| Concern | Choice |
|---|---|
| Transport | stdio MCP server (`board-mcp` or `board mcp`) |
| DB | Same as CLI/desktop: `BOARD_DB` or default `data_dir/board/board.db` |
| Actor | `BOARD_ACTOR_KIND` (default `agent`), `BOARD_ACTOR_NAME` (e.g. `claude`) |
| Card id | Accept `CARD-12` **or** UUID everywhere |
| Errors | Machine-readable `code` + human `message` (see below) |

Repo-local DB tip for Bash fallback:

```bash
export BOARD_DB="$PWD/.board/board.db"
```

## Conventions

### Every mutating tool

- Stamps events with configured actor (`actor_kind` / `actor_name`)
- Returns the affected **card summary** (and extra fields as needed)
- Rejects unknown ids with `card_not_found`

### Card summary (shared shape)

```json
{
  "id": "uuid",
  "key": "CARD-12",
  "title": "…",
  "status": "doing",
  "labels": ["backend"],
  "assignee_kind": "agent",
  "assignee_name": "claude",
  "priority": 1,
  "external_url": null,
  "updated_at": "2026-10-16T00:00:00Z"
}
```

### Error envelope

```json
{
  "ok": false,
  "error": {
    "code": "already_claimed",
    "message": "CARD-12 is claimed by agent:cursor until 2026-10-16T01:00:00Z"
  }
}
```

| code | When |
|---|---|
| `card_not_found` | id/key does not resolve |
| `already_claimed` | active lease held by another actor |
| `not_claimable` | status not in `{ready}` (v1) or card not free |
| `blocked_by_deps` | open `blocked_by` links prevent `doing` / `done` |
| `invalid_status` | bad status string |
| `invalid_link_type` | bad link type |
| `lease_required` | move to `doing`/`done` without holding claim (optional strict mode) |
| `validation` | schema / argument errors |

### MCP annotations

| Tool | `readOnlyHint` | `destructiveHint` | `idempotentHint` |
|---|---|---|---|
| `board_list` | true | false | true |
| `board_get` | true | false | true |
| `board_claim` | false | false | false |
| `board_release` | false | false | true |
| `board_move` | false | false | false |
| `board_note` | false | false | false |
| `board_update` | false | false | false |
| `board_link` | false | false | true |
| `board_create` | false | false | false |

---

## v1 tools

Minimal set for a sandboxed Claude Code session. Prefer these nine; do not expose
`delete` / `export` on the agent server.

### 1. `board_list`

List cards for the agent’s next action (default: claimable work).

**When to use:** Session start; after finishing a card; looking for more work.

```json
{
  "name": "board_list",
  "description": "List Laneboard cards. Default: status=ready and unassigned/expired claim. Use before board_claim.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "status": {
        "type": "string",
        "enum": ["backlog", "ready", "doing", "blocked", "done"],
        "description": "Column filter. Omit for claimable ready cards."
      },
      "assignee": {
        "type": "string",
        "enum": ["unassigned", "human", "agent", "me"],
        "description": "me = configured BOARD_ACTOR_NAME under assignee_kind=agent."
      },
      "label": { "type": "string" },
      "query": { "type": "string", "description": "Substring / FTS query." },
      "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 }
    }
  }
}
```

**Output:** `{ "ok": true, "cards": [CardSummary, ...] }`

**Default filter (no args):** `status=ready` AND (unassigned OR lease expired).

---

### 2. `board_get`

Full card detail for the card being worked.

```json
{
  "name": "board_get",
  "description": "Get one card with body, links, and recent events. Pass CARD-n or UUID.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["card"],
    "properties": {
      "card": { "type": "string", "description": "CARD-n or UUID." },
      "events": { "type": "integer", "minimum": 0, "maximum": 100, "default": 20 }
    }
  }
}
```

**Output:**

```json
{
  "ok": true,
  "card": { "...CardSummary", "body": "…" },
  "links": [
    {
      "from": "CARD-1",
      "to": "CARD-2",
      "type": "blocks",
      "note": null
    }
  ],
  "recent_events": [
    {
      "at": "…",
      "actor_kind": "agent",
      "actor_name": "claude",
      "kind": "moved",
      "payload": { "from": "ready", "to": "doing" }
    }
  ],
  "lease": {
    "holder_kind": "agent",
    "holder_name": "claude",
    "expires_at": "2026-10-16T01:00:00Z"
  }
}
```

`lease` is `null` when free.

---

### 3. `board_claim`

Take exclusive work on a ready card (lease).

**Core change required:** store lease fields (or encode via assignee + `claim_expires_at`
event/meta). v1 suggested columns on `cards`:

- `claim_expires_at TEXT NULL`
- claiming sets `assignee_kind=agent`, `assignee_name=BOARD_ACTOR_NAME`, status→`doing`

```json
{
  "name": "board_claim",
  "description": "Claim a ready card for this agent: assign to me, move to doing, start lease. Fails if another actor holds a live lease.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["card"],
    "properties": {
      "card": { "type": "string" },
      "ttl_minutes": {
        "type": "integer",
        "minimum": 5,
        "maximum": 240,
        "default": 60,
        "description": "Lease length. Expired leases are stealable."
      },
      "note": { "type": "string", "description": "Optional claim note for the timeline." }
    }
  }
}
```

**Output:** `{ "ok": true, "card": CardSummary, "lease": { ... } }`

**Errors:** `already_claimed`, `not_claimable`, `blocked_by_deps` (if card has unmet `blocked_by`).

**Event:** `claimed` with `{ "ttl_minutes", "expires_at", "note" }`.

---

### 4. `board_release`

Drop a claim without finishing (back to `ready` or `blocked`).

```json
{
  "name": "board_release",
  "description": "Release this agent's claim. Use when stopping work early. Default status=ready.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["card"],
    "properties": {
      "card": { "type": "string" },
      "to": { "type": "string", "enum": ["ready", "blocked"], "default": "ready" },
      "note": { "type": "string" }
    }
  }
}
```

Only the lease holder (or expired lease + explicit force later) may release.
Clears assignee to unassigned unless `to=blocked` and note explains handoff.

---

### 5. `board_move`

Change column. Preferred completion path after claim.

```json
{
  "name": "board_move",
  "description": "Move a card to another column. For completion use to=done with a short note. Prefer board_claim before to=doing.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["card", "to"],
    "properties": {
      "card": { "type": "string" },
      "to": {
        "type": "string",
        "enum": ["backlog", "ready", "doing", "blocked", "done"]
      },
      "note": { "type": "string" }
    }
  }
}
```

**Policy (v1 defaults, configurable later):**

| Transition | Rule |
|---|---|
| → `doing` | Prefer via `board_claim`; bare move allowed if unassigned |
| → `done` | If `blocked_by` open links exist → `blocked_by_deps` |
| → `done` / `blocked` | Clears lease; keeps assignee for audit unless unassigned policy |

**Event:** existing `moved` payload `{ from, to, note }`.

---

### 6. `board_note`

Append a timeline note **without** changing fields. Keeps body clean; humans see agent progress on the desktop timeline.

```json
{
  "name": "board_note",
  "description": "Append a progress/blocker note to the card timeline. Does not edit the body.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["card", "note"],
    "properties": {
      "card": { "type": "string" },
      "note": { "type": "string", "minLength": 1, "maxLength": 4000 }
    }
  }
}
```

**Event:** `note` with `{ "text": "…" }`.  
**Core change:** `append_event` kind `note` (no card row mutation beyond `updated_at` optional).

---

### 7. `board_update`

Patch mutable fields (title/body/labels/priority/external_url). Not for status (use move/claim).

```json
{
  "name": "board_update",
  "description": "Update card fields (title, body, labels, priority, external_url). Do not use for status changes.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["card"],
    "properties": {
      "card": { "type": "string" },
      "title": { "type": "string", "minLength": 1 },
      "body": { "type": "string" },
      "labels": { "type": "array", "items": { "type": "string" } },
      "priority": { "type": "integer", "minimum": 0, "maximum": 5 },
      "external_url": { "type": "string", "description": "PR, issue, or worktree URI." }
    }
  }
}
```

Maps to existing `UpdateCard`. Assignee changes go through claim/release only.

---

### 8. `board_link`

Relate cards (deps / parent).

```json
{
  "name": "board_link",
  "description": "Link two cards. Use type=blocks when from must finish before to. Inverse links are stored automatically.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["from", "to"],
    "properties": {
      "from": { "type": "string" },
      "to": { "type": "string" },
      "type": {
        "type": "string",
        "enum": ["blocks", "blocked_by", "relates", "parent", "child", "external"],
        "default": "relates"
      },
      "note": { "type": "string" }
    }
  }
}
```

Maps to existing `link_cards`.

---

### 9. `board_create`

Split work or file follow-ups. Humans still own most backlog grooming.

```json
{
  "name": "board_create",
  "description": "Create a card. Default status=backlog. Use when splitting work; link back to the parent with board_link.",
  "inputSchema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["title"],
    "properties": {
      "title": { "type": "string", "minLength": 1 },
      "body": { "type": "string" },
      "status": {
        "type": "string",
        "enum": ["backlog", "ready", "doing", "blocked", "done"],
        "default": "backlog"
      },
      "labels": { "type": "array", "items": { "type": "string" } },
      "priority": { "type": "integer", "minimum": 0, "maximum": 5 },
      "external_url": { "type": "string" }
    }
  }
}
```

---

## Agent session recipe (prompt / skill)

Ship as a Claude Code skill or MCP `instructions` blurb:

1. `board_list` → pick one card  
2. `board_claim` that card  
3. `board_get` → read body, links, acceptance  
4. Work in the repo (Bash sandbox for git/tests)  
5. `board_note` for progress / decisions  
6. `board_move` → `done` (or `blocked` + note)  
7. If stopping early → `board_release`

Do **not** shell out to `board` for these steps when MCP is connected.

---

## Claude Code wiring

```json
{
  "mcpServers": {
    "laneboard": {
      "command": "board",
      "args": ["mcp"],
      "env": {
        "BOARD_DB": "/absolute/path/to/board.db",
        "BOARD_ACTOR_NAME": "claude"
      }
    }
  }
}
```

Sandbox note: MCP runs **outside** sandboxed Bash, so it can use the desktop’s
default DB path even when Bash cannot write there. Keep `BOARD_DB` explicit in
team configs so humans and agents share one file.

---

## Mapping to current core

| Tool | Today (`board-core` / CLI) | New work |
|---|---|---|
| `board_list` | `list_cards` / `ls` | Default “claimable” filter |
| `board_get` | `get_card_detail` / `get` | Include `lease` |
| `board_claim` | — | Lease fields + `claimed` event + status/assignee |
| `board_release` | partial via `move`+`update` | First-class release |
| `board_move` | `move_card` | Optional dep gate |
| `board_note` | — | `note` event kind |
| `board_update` | `update_card` | No status/assignee via this tool |
| `board_link` | `link_cards` | — |
| `board_create` | `create_card` | — |

---

## v1.1 (next, not in first MCP cut)

| Tool / behavior | Why |
|---|---|
| `board_acceptance_list` / `board_acceptance_check` | Structured DoD; sandbox-friendly progress |
| Dep gate enforced on →`doing`/`done` | Stop premature completion |
| `board_heartbeat` | Extend lease while still working |
| `strict_claim` server flag | Require lease for →`doing`/`done` |
| Compact `board_snapshot` | Human debugging only; hide from default agent list |

---

## Implementation sketch

1. `crates/core`: lease columns + `claim` / `release` / `add_note`; dep check helper  
2. `crates/mcp` (or `board mcp` subcommand): rmcp/stdio, tool table above  
3. Desktop: show lease holder + expiry on cards; timeline renders `note` / `claimed`  
4. Docs: Claude Code config snippet + session recipe  
5. Tests: claim conflict, expired steal, note event, blocked_by on done  

Keep the public agent surface at **these nine tools**; add CLI mirrors later for
parity, not the other way around.
