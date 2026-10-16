# GitLab integration

One-way **pull** of GitLab Issues into Laneboard cards. No webhooks, no cloud sync — fits the local SQLite model (MCP non-goal: network webhooks).

## Setup

1. Create a GitLab [Personal Access Token](https://docs.gitlab.com/ee/user/profile/personal_access_tokens.html) with `read_api`.
2. Export:

```bash
export GITLAB_TOKEN=glpat-…
# self-managed / Dedicated:
# export GITLAB_URL=https://gitlab.example.com
```

## Commands

```bash
# Auth check
board gitlab doctor

# Import open issues → ready cards (dedupe by issue URL)
board gitlab import group/project
board gitlab import group/project --state all --label plane --dry-run
board gitlab import group/project --open-status backlog --json

# Refresh cards already linked to that project via external_url
board gitlab sync group/project
```

| Flag | Meaning |
|---|---|
| `--state opened\|closed\|all` | Which GitLab issues to import (default `opened`) |
| `--label NAME` | Only issues with that GitLab label |
| `--open-status STATUS` | Board column for open issues (default `ready`) |
| `--dry-run` | Report actions without writing |
| `--actor-name` | Timeline actor (default `gitlab`) |

## Mapping

| GitLab | Board |
|---|---|
| Issue title / description | Card title / body |
| `opened` | `--open-status` (default `ready`) |
| `closed` | `done` |
| `web_url` | `external_url` (dedupe key) |
| Labels | Kept + `gitlab` + `gitlab:#iid` |

Re-importing the same project updates existing cards matched by `external_url`. Status moves use the normal `move_card` path (dep gate); if a card is blocked by deps, title/body/labels still refresh and status stays put.

## CI

[`.gitlab-ci.yml`](../.gitlab-ci.yml) runs `cargo test` / `cargo build -p board` on Linux GitLab runners (desktop installers still ship from GitHub Actions macOS/Windows).

## Non-goals (this cut)

- Bidirectional write-back to GitLab
- Merge request automation
- Webhooks / live push
- MCP tools (agents can shell `board gitlab …` when needed)
