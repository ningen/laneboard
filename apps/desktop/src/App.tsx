import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "./api";
import {
  BoardSnapshot,
  Card,
  CardDetail,
  COLUMNS,
  COLUMN_LABELS,
  Event,
  Status,
} from "./types";

type DetailTab = "properties" | "activity";

export default function App() {
  const [board, setBoard] = useState<BoardSnapshot | null>(null);
  const [selected, setSelected] = useState<CardDetail | null>(null);
  const [events, setEvents] = useState<Event[]>([]);
  const [query, setQuery] = useState("");
  const [dbPath, setDbPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [dragId, setDragId] = useState<string | null>(null);
  const [composerOpen, setComposerOpen] = useState(false);
  const [detailTab, setDetailTab] = useState<DetailTab>("properties");
  const [drawerOpen, setDrawerOpen] = useState(false);

  const refresh = useCallback(async (q?: string) => {
    setError(null);
    try {
      if (q && q.trim()) {
        const cards = await api<Card[]>("list_cards", { query: q.trim() });
        const columns = COLUMNS.map((status) => ({
          status,
          cards: cards.filter((c) => c.status === status),
        }));
        setBoard({ columns });
      } else {
        setBoard(await api<BoardSnapshot>("get_board"));
      }
      setEvents(await api<Event[]>("timeline", { limit: 40 }));
      setDbPath(await api<string>("db_path"));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const t = window.setTimeout(() => {
      void refresh(query);
    }, 180);
    return () => window.clearTimeout(t);
  }, [query, refresh]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (composerOpen) setComposerOpen(false);
        else closeDrawer();
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        document.getElementById("board-search")?.focus();
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        setComposerOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [composerOpen]);

  const openCard = async (id: string) => {
    try {
      setSelected(await api<CardDetail>("get_card", { id }));
      setDetailTab("properties");
      setDrawerOpen(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const closeDrawer = () => {
    setDrawerOpen(false);
    setSelected(null);
  };

  const onCreate = async (e: FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    try {
      const card = await api<Card>("create_card", {
        input: { title: title.trim(), body: body.trim() || null, status: "backlog" },
      });
      setTitle("");
      setBody("");
      setComposerOpen(false);
      await refresh(query);
      await openCard(card.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const moveTo = async (id: string, to: Status) => {
    try {
      await api<Card>("move_card", { input: { id, to, note: null } });
      await refresh(query);
      if (selected?.card.id === id || selected?.card.key === id) {
        setSelected(await api<CardDetail>("get_card", { id }));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const visibleCount = useMemo(
    () => board?.columns.reduce((n, c) => n + c.cards.length, 0) ?? 0,
    [board],
  );

  if (!board && !error) {
    return (
      <div className="loading" role="status">
        <span className="loading-dot" aria-hidden />
        Loading board…
      </div>
    );
  }

  return (
    <div className={`app${drawerOpen ? " drawer-open" : ""}`}>
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden />
          <div className="brand-text">
            <strong>Laneboard</strong>
            <span>Human &amp; agent</span>
          </div>
        </div>

        <div className="search">
          <svg className="search-icon" viewBox="0 0 16 16" aria-hidden>
            <path
              fill="currentColor"
              d="M11.5 10.4 14.6 13.5a.75.75 0 1 1-1.06 1.06l-3.1-3.1a5.5 5.5 0 1 1 1.06-1.06ZM7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9Z"
            />
          </svg>
          <input
            id="board-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search issues…"
            aria-label="Search issues"
          />
          <kbd className="search-hint">⌘K</kbd>
        </div>

        <div className="top-actions">
          <span className="board-meta" title={dbPath || undefined}>
            {visibleCount} issue{visibleCount === 1 ? "" : "s"}
          </span>
          <button className="btn ghost" type="button" onClick={() => void refresh(query)}>
            Refresh
          </button>
          <button
            className="btn ghost"
            type="button"
            onClick={async () => {
              await api("seed_demo");
              await refresh(query);
            }}
          >
            Seed
          </button>
          <button className="btn primary" type="button" onClick={() => setComposerOpen(true)}>
            New issue
          </button>
        </div>
      </header>

      <div className="workspace">
        {error && (
          <div className="banner error" role="alert">
            <span>{error}</span>
            <button type="button" className="btn tiny ghost" onClick={() => setError(null)}>
              Dismiss
            </button>
          </div>
        )}

        <div className="main">
        <section className="board" aria-label="Kanban board">
          {board?.columns.map((col) => (
            <div
              key={col.status}
              className={`column status-${col.status}`}
              onDragOver={(e) => {
                e.preventDefault();
                e.currentTarget.classList.add("drag-over");
              }}
              onDragLeave={(e) => e.currentTarget.classList.remove("drag-over")}
              onDrop={(e) => {
                e.preventDefault();
                e.currentTarget.classList.remove("drag-over");
                const id = dragId || e.dataTransfer.getData("text/plain");
                if (id) void moveTo(id, col.status);
                setDragId(null);
              }}
            >
              <div className="column-head">
                <div className="column-title">
                  <span className={`status-dot status-${col.status}`} aria-hidden />
                  <h2>{COLUMN_LABELS[col.status]}</h2>
                </div>
                <span className="count">{col.cards.length}</span>
              </div>
              <div className="column-body">
                {col.cards.length === 0 ? (
                  <div className="column-empty">Drop here</div>
                ) : (
                  col.cards.map((card) => (
                    <button
                      key={card.id}
                      type="button"
                      className={`card${selected?.card.id === card.id ? " selected" : ""}`}
                      draggable
                      onDragStart={(e) => {
                        setDragId(card.id);
                        e.dataTransfer.setData("text/plain", card.id);
                        e.dataTransfer.effectAllowed = "move";
                      }}
                      onDragEnd={() => setDragId(null)}
                      onClick={() => void openCard(card.id)}
                    >
                      <div className="card-key">{card.key}</div>
                      <p className="card-title">{card.title}</p>
                      <div className="card-meta">
                        {card.assignee_kind !== "unassigned" && (
                          <span className={`chip ${card.assignee_kind}`}>
                            {card.assignee_kind}
                            {card.assignee_name ? ` · ${card.assignee_name}` : ""}
                          </span>
                        )}
                        {card.labels.map((l) => (
                          <span key={l} className="chip">
                            {l}
                          </span>
                        ))}
                      </div>
                    </button>
                  ))
                )}
              </div>
            </div>
          ))}
        </section>

        <aside
          className={`drawer${drawerOpen && selected ? " open" : ""}`}
          aria-hidden={!drawerOpen || !selected}
          aria-label="Issue detail"
        >
          {selected && (
            <>
              <div className="drawer-head">
                <div className="drawer-key-row">
                  <span className="detail-key">{selected.card.key}</span>
                  <button
                    type="button"
                    className="btn icon ghost"
                    aria-label="Close detail"
                    onClick={closeDrawer}
                  >
                    <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden>
                      <path
                        fill="currentColor"
                        d="M3.28 3.28a.75.75 0 0 1 1.06 0L8 6.94l3.66-3.66a.75.75 0 1 1 1.06 1.06L9.06 8l3.66 3.66a.75.75 0 1 1-1.06 1.06L8 9.06l-3.66 3.66a.75.75 0 0 1-1.06-1.06L6.94 8 3.28 4.34a.75.75 0 0 1 0-1.06Z"
                      />
                    </svg>
                  </button>
                </div>
                <h3 className="detail-title">{selected.card.title}</h3>
                <div className="status-picker" role="group" aria-label="Move issue">
                  {COLUMNS.map((s) => (
                    <button
                      key={s}
                      type="button"
                      className={`status-option${selected.card.status === s ? " active" : ""} status-${s}`}
                      disabled={selected.card.status === s}
                      onClick={() => void moveTo(selected.card.id, s)}
                    >
                      <span className={`status-dot status-${s}`} aria-hidden />
                      {COLUMN_LABELS[s]}
                    </button>
                  ))}
                </div>
              </div>

              <div className="drawer-tabs" role="tablist">
                <button
                  type="button"
                  role="tab"
                  aria-selected={detailTab === "properties"}
                  className={detailTab === "properties" ? "active" : ""}
                  onClick={() => setDetailTab("properties")}
                >
                  Properties
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={detailTab === "activity"}
                  className={detailTab === "activity" ? "active" : ""}
                  onClick={() => setDetailTab("activity")}
                >
                  Activity
                </button>
              </div>

              <div className="drawer-body">
                {detailTab === "properties" ? (
                  <>
                    <section className="props-block">
                      <h4>Description</h4>
                      <p className="detail-body">
                        {selected.card.body || "No description."}
                      </p>
                    </section>

                    <section className="props-block">
                      <h4>Assignee</h4>
                      <p className="prop-value">
                        {selected.card.assignee_kind === "unassigned"
                          ? "Unassigned"
                          : `${selected.card.assignee_kind}${
                              selected.card.assignee_name
                                ? ` · ${selected.card.assignee_name}`
                                : ""
                            }`}
                      </p>
                    </section>

                    {selected.card.external_url && (
                      <section className="props-block">
                        <h4>External</h4>
                        <p className="prop-value">
                          <a
                            href={selected.card.external_url}
                            target="_blank"
                            rel="noreferrer"
                          >
                            {selected.card.external_url}
                          </a>
                        </p>
                      </section>
                    )}

                    {selected.card.labels.length > 0 && (
                      <section className="props-block">
                        <h4>Labels</h4>
                        <div className="card-meta">
                          {selected.card.labels.map((l) => (
                            <span key={l} className="chip">
                              {l}
                            </span>
                          ))}
                        </div>
                      </section>
                    )}

                    {selected.links.length > 0 && (
                      <section className="props-block">
                        <h4>Links</h4>
                        {selected.links.map((l) => (
                          <div key={l.id} className="event">
                            <span className="event-kind">{l.link_type}</span>
                            <span>
                              {l.from_id.slice(0, 8)}… → {l.to_id.slice(0, 8)}…
                            </span>
                            {l.note && <span className="event-note">{l.note}</span>}
                          </div>
                        ))}
                      </section>
                    )}
                  </>
                ) : (
                  <>
                    <section className="props-block">
                      <h4>Issue timeline</h4>
                      {selected.recent_events.length === 0 ? (
                        <p className="empty">No events on this issue.</p>
                      ) : (
                        selected.recent_events.map((ev) => (
                          <div key={ev.id} className="event">
                            <time dateTime={ev.at}>{formatWhen(ev.at)}</time>
                            <span>
                              {ev.kind} · {ev.actor_kind}
                              {ev.actor_name ? `/${ev.actor_name}` : ""}
                            </span>
                          </div>
                        ))
                      )}
                    </section>
                    <section className="props-block">
                      <h4>Board activity</h4>
                      {events.length === 0 ? (
                        <p className="empty">No board events yet.</p>
                      ) : (
                        events.slice(0, 16).map((ev) => (
                          <div key={ev.id} className="event">
                            <time dateTime={ev.at}>{formatWhen(ev.at)}</time>
                            <span>
                              {ev.kind}
                              {ev.card_id ? ` · ${ev.card_id.slice(0, 8)}…` : ""}
                            </span>
                          </div>
                        ))
                      )}
                    </section>
                  </>
                )}
              </div>

              {dbPath && (
                <p className="db-path" title={dbPath}>
                  {dbPath}
                </p>
              )}
            </>
          )}
        </aside>
        </div>
      </div>

      {composerOpen &&
        createPortal(
          <div
            className="modal-root"
            role="dialog"
            aria-modal="true"
            aria-labelledby="composer-title"
            onMouseDown={(e) => {
              if (e.target === e.currentTarget) setComposerOpen(false);
            }}
          >
            <form className="modal" onSubmit={onCreate}>
              <div className="modal-head">
                <h2 id="composer-title">New issue</h2>
                <button
                  type="button"
                  className="btn icon ghost"
                  aria-label="Close"
                  onClick={() => setComposerOpen(false)}
                >
                  <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden>
                    <path
                      fill="currentColor"
                      d="M3.28 3.28a.75.75 0 0 1 1.06 0L8 6.94l3.66-3.66a.75.75 0 1 1 1.06 1.06L9.06 8l3.66 3.66a.75.75 0 1 1-1.06 1.06L8 9.06l-3.66 3.66a.75.75 0 0 1-1.06-1.06L6.94 8 3.28 4.34a.75.75 0 0 1 0-1.06Z"
                    />
                  </svg>
                </button>
              </div>
              <input
                className="composer-title"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="Issue title"
                required
                autoFocus
              />
              <textarea
                value={body}
                onChange={(e) => setBody(e.target.value)}
                placeholder="Description (optional)"
              />
              <div className="modal-actions">
                <button className="btn ghost" type="button" onClick={() => setComposerOpen(false)}>
                  Cancel
                </button>
                <button className="btn primary" type="submit">
                  Add to backlog
                </button>
              </div>
            </form>
          </div>,
          document.body,
        )}
    </div>
  );
}

function formatWhen(iso: string): string {
  try {
    return new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}
