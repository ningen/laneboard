# Board design system

Visual language blends **Linear’s restraint** (hierarchy via opacity, sparse accent, quiet chrome) with **Apple HIG** light surfaces (separators, type scale, calm toolbar).

## Principles

1. **Content over chrome** — neutrals first; accent only for keys, focus, primary action.
2. **Hierarchy via opacity / weight**, not size explosions.
3. **Hairline structure** — `1px` separators, no heavy panel frames on columns.
4. **Cards only where interactive** — kanban items are the cards; columns are rails.
5. **One soft shadow** — no stacked glow stacks.

## Tokens

Source of truth: `apps/desktop/src/design-system.css`

| Group | Examples |
|---|---|
| Surfaces | `--bg`, `--bg-elevated`, `--bg-subtle` |
| Ink | `--ink` → `--ink-quaternary` |
| Lines | `--line`, `--line-strong`, `--line-focus` |
| Accent | `--accent` Apple blue `#0071E3` (dark: `#0A84FF`) |
| Radius | `--r-sm` 6 / `--r-md` 10 / `--r-lg` 14 |
| Type | IBM Plex Sans + IBM Plex Mono |

Light is default; `prefers-color-scheme: dark` maps to Apple dark neutrals.

## Typography

| Role | Size | Weight |
|---|---|---|
| Brand | 17px | 600 |
| Card title | 13px | 500 |
| Column label | 11px uppercase | 600 |
| Mono keys | 11px | 500 |

Tracking: tight on titles (`-0.022em`), slight positive on labels.

## Components

Mapped in `styles.css`: `.topbar`, `.btn` (+ `.primary` / `.ghost` / `.tiny`), `.column`, `.card`, `.chip`, `.side`, `.composer`.

## Anti-patterns (avoid)

Purple/indigo brand washes, cream+terracotta editorial kits, newspaper grids, neon glow, pill clusters, multi-layer shadows, emoji ornament.
