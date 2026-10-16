import { FormEvent, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "./api";
import { GitLabSettingsView, Status, SyncReport } from "./types";

type Props = {
  open: boolean;
  onClose: () => void;
  onImported: () => void;
  onError: (msg: string) => void;
};

const OPEN_STATUSES: Status[] = ["backlog", "ready", "doing"];

export function SettingsModal({ open, onClose, onImported, onError }: Props) {
  const [baseUrl, setBaseUrl] = useState("https://gitlab.com");
  const [token, setToken] = useState("");
  const [tokenSet, setTokenSet] = useState(false);
  const [project, setProject] = useState("");
  const [openStatus, setOpenStatus] = useState("ready");
  const [label, setLabel] = useState("");
  const [state, setState] = useState("opened");
  const [busy, setBusy] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    if (!open) return;
    setStatusMsg(null);
    setToken("");
    void (async () => {
      try {
        const s = await api<GitLabSettingsView>("get_gitlab_settings");
        setBaseUrl(s.base_url || "https://gitlab.com");
        setTokenSet(s.token_set);
        setProject(s.project || "");
        setOpenStatus(s.open_status || "ready");
        setLabel(s.label || "");
        setState(s.state || "opened");
        setLoaded(true);
      } catch (e) {
        onError(e instanceof Error ? e.message : String(e));
      }
    })();
  }, [open, onError]);

  if (!open) return null;

  const save = async (e?: FormEvent) => {
    e?.preventDefault();
    setBusy(true);
    setStatusMsg(null);
    try {
      const view = await api<GitLabSettingsView>("save_gitlab_settings", {
        input: {
          base_url: baseUrl.trim(),
          token: token.trim() || undefined,
          project: project.trim(),
          open_status: openStatus,
          label: label.trim(),
          state,
        },
      });
      setTokenSet(view.token_set);
      setToken("");
      setStatusMsg("Saved.");
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const run = async (cmd: "gitlab_doctor" | "gitlab_import" | "gitlab_sync") => {
    setBusy(true);
    setStatusMsg(null);
    try {
      // Persist form first so actions use current values.
      await api<GitLabSettingsView>("save_gitlab_settings", {
        input: {
          base_url: baseUrl.trim(),
          token: token.trim() || undefined,
          project: project.trim(),
          open_status: openStatus,
          label: label.trim(),
          state,
        },
      });
      setToken("");
      setTokenSet(true);

      if (cmd === "gitlab_doctor") {
        const info = await api<{
          ok: boolean;
          user?: { username?: string; name?: string };
          base_url?: string;
        }>("gitlab_doctor");
        setStatusMsg(
          `Connected as @${info.user?.username ?? "?"} on ${info.base_url ?? baseUrl}`,
        );
      } else {
        const report = await api<SyncReport>(cmd, { input: { dry_run: false } });
        setStatusMsg(
          `${cmd === "gitlab_import" ? "Import" : "Sync"} ${report.project}: ` +
            `+${report.created} ~${report.updated} =${report.unchanged}`,
        );
        onImported();
      }
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return createPortal(
    <div
      className="modal-root"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-title"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <form className="modal settings-modal" onSubmit={(e) => void save(e)}>
        <div className="modal-head">
          <h2 id="settings-title">Settings</h2>
          <button
            type="button"
            className="btn icon ghost"
            aria-label="Close"
            onClick={onClose}
          >
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden>
              <path
                fill="currentColor"
                d="M3.28 3.28a.75.75 0 0 1 1.06 0L8 6.94l3.66-3.66a.75.75 0 1 1 1.06 1.06L9.06 8l3.66 3.66a.75.75 0 1 1-1.06 1.06L8 9.06l-3.66 3.66a.75.75 0 0 1-1.06-1.06L6.94 8 3.28 4.34a.75.75 0 0 1 0-1.06Z"
              />
            </svg>
          </button>
        </div>

        <section className="settings-section">
          <h3>GitLab</h3>
          <p className="settings-hint">
            Pull issues into this board. Token stays in the local SQLite DB (never
            sent to the UI after save).
          </p>

          {!loaded ? (
            <p className="settings-hint">Loading…</p>
          ) : (
            <div className="settings-grid">
              <label className="field">
                <span>Host</span>
                <input
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder="https://gitlab.com"
                  autoComplete="off"
                />
              </label>
              <label className="field">
                <span>
                  Access token
                  {tokenSet ? " · saved" : ""}
                </span>
                <input
                  type="password"
                  value={token}
                  onChange={(e) => setToken(e.target.value)}
                  placeholder={tokenSet ? "Leave blank to keep current" : "glpat-…"}
                  autoComplete="off"
                />
              </label>
              <label className="field">
                <span>Project</span>
                <input
                  value={project}
                  onChange={(e) => setProject(e.target.value)}
                  placeholder="group/project"
                  autoComplete="off"
                />
              </label>
              <label className="field">
                <span>Issue state</span>
                <select value={state} onChange={(e) => setState(e.target.value)}>
                  <option value="opened">Opened</option>
                  <option value="closed">Closed</option>
                  <option value="all">All</option>
                </select>
              </label>
              <label className="field">
                <span>Open → column</span>
                <select
                  value={openStatus}
                  onChange={(e) => setOpenStatus(e.target.value)}
                >
                  {OPEN_STATUSES.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>Label filter (optional)</span>
                <input
                  value={label}
                  onChange={(e) => setLabel(e.target.value)}
                  placeholder="e.g. board"
                  autoComplete="off"
                />
              </label>
            </div>
          )}

          {statusMsg && <p className="settings-status">{statusMsg}</p>}

          <div className="settings-actions">
            <button
              className="btn ghost"
              type="button"
              disabled={busy}
              onClick={() => void run("gitlab_doctor")}
            >
              Test connection
            </button>
            <button
              className="btn ghost"
              type="button"
              disabled={busy}
              onClick={() => void run("gitlab_import")}
            >
              Import issues
            </button>
            <button
              className="btn ghost"
              type="button"
              disabled={busy}
              onClick={() => void run("gitlab_sync")}
            >
              Sync linked
            </button>
          </div>
        </section>

        <div className="modal-actions">
          <button className="btn ghost" type="button" onClick={onClose}>
            Close
          </button>
          <button className="btn primary" type="submit" disabled={busy || !loaded}>
            Save
          </button>
        </div>
      </form>
    </div>,
    document.body,
  );
}
