import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
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

  const openCard = async (id: string) => {
    try {
      setSelected(await api<CardDetail>("get_card", { id }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
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
        await openCard(id);
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
    return <div className="loading">Loading board…</div>;
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <strong>Board</strong>
          <span>Human &amp; agent</span>
        </div>
        <div className="search">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search"
            aria-label="Search"
          />
        </div>
        <div className="top-actions">
          <button className="btn ghost" type="button" onClick={() => void refresh(query)}>
            Refresh
          </button>
          <button
            className="btn"
            type="button"
            onClick={async () => {
              await api("seed_demo");
              await refresh(query);
            }}
          >
            Seed
          </button>
        </div>
      </header>

      <div className="main">
        <section className="board" aria-label="Kanban board">
          {board?.columns.map((col) => (
            <div
              key={col.status}
              className="column"
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
                <h2>{COLUMN_LABELS[col.status]}</h2>
                <span className="count">{col.cards.length}</span>
              </div>
              <div className="column-body">
                {col.cards.map((card) => (
                  <button
                    key={card.id}
                    type="button"
                    className={`card${selected?.card.id === card.id ? " selected" : ""}`}
                    draggable
                    onDragStart={(e) => {
                      setDragId(card.id);
                      e.dataTransfer.setData("text/plain", card.id);
                    }}
                    onClick={() => void openCard(card.id)}
                  >
                    <div className="card-key">{card.key}</div>
                    <p className="card-title">{card.title}</p>
                    <div className="card-meta">
                      {card.assignee_kind !== "unassigned" && (
                        <span className={`chip ${card.assignee_kind}`}>
                          {card.assignee_kind}
                          {card.assignee_name ? `:${card.assignee_name}` : ""}
                        </span>
                      )}
                      {card.labels.map((l) => (
                        <span key={l} className="chip">
                          {l}
                        </span>
                      ))}
                    </div>
                  </button>
                ))}
              </div>
            </div>
          ))}
        </section>

        <aside className="side">
          {error && <p className="error">{error}</p>}

          <div className="panel">
            <h3>New card</h3>
            <form className="composer" onSubmit={onCreate}>
              <input
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="Title"
                required
              />
              <textarea
                value={body}
                onChange={(e) => setBody(e.target.value)}
                placeholder="Notes (optional)"
              />
              <button className="btn primary" type="submit">
                Add to backlog
              </button>
            </form>
          </div>

          <div className="panel">
            <h3>Detail</h3>
            {!selected ? (
              <p className="empty">Select a card. {visibleCount} visible.</p>
            ) : (
              <>
                <div className="detail-key">{selected.card.key}</div>
                <h4 className="detail-title">{selected.card.title}</h4>
                <p className="detail-body">
                  {selected.card.body || "No description."}
                </p>
                <div className="move-row">
                  {COLUMNS.map((s) => (
                    <button
                      key={s}
                      type="button"
                      className="btn tiny"
                      disabled={selected.card.status === s}
                      onClick={() => void moveTo(selected.card.id, s)}
                    >
                      {COLUMN_LABELS[s]}
                    </button>
                  ))}
                </div>
                {selected.links.length > 0 && (
                  <>
                    <h3 style={{ marginTop: "var(--s-5)" }}>Links</h3>
                    {selected.links.map((l) => (
                      <div key={l.id} className="event">
                        <span>
                          {l.link_type}: {l.from_id.slice(0, 8)}… → {l.to_id.slice(0, 8)}…
                        </span>
                        {l.note && <span>{l.note}</span>}
                      </div>
                    ))}
                  </>
                )}
                {selected.recent_events.length > 0 && (
                  <>
                    <h3 style={{ marginTop: "var(--s-5)" }}>Timeline</h3>
                    {selected.recent_events.slice(0, 8).map((ev) => (
                      <div key={ev.id} className="event">
                        <time>{new Date(ev.at).toLocaleString()}</time>
                        <span>
                          {ev.kind} · {ev.actor_kind}
                          {ev.actor_name ? `/${ev.actor_name}` : ""}
                        </span>
                      </div>
                    ))}
                  </>
                )}
              </>
            )}
          </div>

          <div className="panel">
            <h3>Activity</h3>
            {events.length === 0 ? (
              <p className="empty">No events yet.</p>
            ) : (
              events.slice(0, 12).map((ev) => (
                <div key={ev.id} className="event">
                  <time>{new Date(ev.at).toLocaleString()}</time>
                  <span>
                    {ev.kind}
                    {ev.card_id ? ` · ${ev.card_id.slice(0, 8)}…` : ""}
                  </span>
                </div>
              ))
            )}
          </div>

          {dbPath && (
            <p className="db-path" title={dbPath}>
              db: {dbPath}
            </p>
          )}
        </aside>
      </div>
    </div>
  );
}