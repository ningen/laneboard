# Laneboard design system

Visual language follows patterns proven in **Linear**, **Plane**, and **Height** boards — dense scan-friendly cards, semantic status color, and a focused detail drawer — without copying any one product’s chrome.

## Principles

1. **Board first** — the kanban is the primary surface; chrome stays quiet.
2. **Card anatomy** — key, title, assignee/labels only. Descriptions live in the drawer.
3. **Status is color** — column dots and status picker share one palette.
4. **Detail on demand** — selecting an issue opens a drawer (Properties / Activity). Composer is a modal (`⌘N`).
5. **One soft elevation** — cards use a single light shadow; no glow stacks.

## Tokens

Source of truth: `apps/desktop/src/design-system.css`

| Group | Examples |
|---|---|
| Surfaces | `--bg-board`, `--bg-elevated`, `--bg-column` |
| Ink | `--ink` → `--ink-quaternary` |
| Brand | Teal `--accent` `#0F766E` (dark: `#2DD4BF`) |
| Status | backlog / ready / doing / blocked / done |
| Type | Plus Jakarta Sans + IBM Plex Mono |

Light is default; `prefers-color-scheme: dark` remaps neutrals and accents.

## Layout

| Region | Role |
|---|---|
| Top bar | Brand, search (`⌘K`), Refresh / Seed, **New issue** |
| Board | Five status columns with drop targets |
| Drawer | Issue detail + status move + Properties / Activity tabs |
| Modal | Create issue → backlog |

## Typography

| Role | Size | Weight |
|---|---|---|
| Brand | 14px | 700 |
| Issue title (drawer) | 20px | 700 |
| Card title | 13px | 600 |
| Column label | 11px uppercase | 700 |
| Mono keys | 11px | 500 |

## Anti-patterns (avoid)

Purple/indigo brand washes, cream+terracotta editorial kits, newspaper grids, neon glow, pill clusters, multi-layer shadows, emoji ornament, always-visible stacked sidebars that compete with the board.
