import { ActivityItem } from "./types";

type Props = {
  items: ActivityItem[];
  open: boolean;
  onToggle: () => void;
  onOpenCard: (idOrKey: string) => void;
  live: boolean;
};

export function ActivityFeed({ items, open, onToggle, onOpenCard, live }: Props) {
  return (
    <aside
      className={`activity-feed${open ? " open" : ""}`}
      aria-label="Agent activity"
      aria-hidden={!open}
    >
      <div className="activity-feed-head">
        <div className="activity-feed-title">
          <h2>Activity</h2>
          {live && (
            <span className="live-pill" title="Polling for new events">
              <span className="live-dot" aria-hidden />
              Live
            </span>
          )}
        </div>
        <button
          type="button"
          className="btn icon ghost"
          aria-label={open ? "Collapse activity" : "Expand activity"}
          onClick={onToggle}
        >
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden>
            <path
              fill="currentColor"
              d={
                open
                  ? "M9.78 3.22a.75.75 0 0 1 0 1.06L6.06 8l3.72 3.72a.75.75 0 1 1-1.06 1.06l-4.25-4.25a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0Z"
                  : "M6.22 3.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 1 1-1.06-1.06L9.94 8 6.22 4.28a.75.75 0 0 1 0-1.06Z"
              }
            />
          </svg>
        </button>
      </div>

      {open && (
        <div className="activity-feed-body">
          {items.length === 0 ? (
            <p className="empty">No activity yet. Agents will show up here.</p>
          ) : (
            items.map((ev) => (
              <button
                key={ev.id}
                type="button"
                className="activity-row"
                disabled={!ev.card_id && !ev.card_key}
                onClick={() => {
                  const target = ev.card_key || ev.card_id;
                  if (target) onOpenCard(target);
                }}
              >
                <div className="activity-row-top">
                  <span className={`activity-actor ${ev.actor_kind}`}>
                    {formatActor(ev.actor_kind, ev.actor_name)}
                  </span>
                  <time dateTime={ev.at}>{formatWhen(ev.at)}</time>
                </div>
                <div className="activity-row-main">
                  <span className="activity-kind">{formatKind(ev)}</span>
                  {ev.card_key && <span className="activity-key">{ev.card_key}</span>}
                </div>
                {ev.card_title && <p className="activity-title">{ev.card_title}</p>}
              </button>
            ))
          )}
        </div>
      )}
    </aside>
  );
}

function formatActor(kind: string, name: string | null): string {
  if (name) return `${kind}/${name}`;
  return kind;
}

function formatKind(ev: ActivityItem): string {
  if (ev.kind === "moved") {
    const to =
      ev.payload &&
      typeof ev.payload === "object" &&
      ev.payload !== null &&
      "to" in ev.payload
        ? String((ev.payload as { to?: unknown }).to ?? "")
        : "";
    return to ? `moved → ${to}` : "moved";
  }
  return ev.kind;
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
