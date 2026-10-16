import { invoke } from "@tauri-apps/api/core";

const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function http<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, {
    headers: { "Content-Type": "application/json", ...(init?.headers || {}) },
    ...init,
  });
  if (!res.ok) {
    let msg = res.statusText;
    try {
      const body = await res.json();
      if (body?.error) msg = body.error;
    } catch {
      /* ignore */
    }
    throw new Error(msg);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

/** Works in Tauri (invoke) and in `board ui` (HTTP). */
export async function api<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri()) {
    return invoke<T>(cmd, args);
  }

  switch (cmd) {
    case "get_board":
      return http("/api/board");
    case "list_cards": {
      const q = (args?.query as string | undefined) || "";
      const qs = q ? `?q=${encodeURIComponent(q)}` : "";
      return http(`/api/cards${qs}`);
    }
    case "get_card":
      return http(`/api/cards/${encodeURIComponent(String(args?.id))}`);
    case "create_card":
      return http("/api/cards", {
        method: "POST",
        body: JSON.stringify(args?.input ?? {}),
      });
    case "move_card": {
      const input = (args?.input || {}) as {
        id: string;
        to: string;
        note?: string | null;
      };
      return http(`/api/cards/${encodeURIComponent(input.id)}/move`, {
        method: "POST",
        body: JSON.stringify({ to: input.to, note: input.note ?? null }),
      });
    }
    case "update_card": {
      const input = (args?.input || {}) as {
        id: string;
        title?: string;
        body?: string;
      };
      return http(`/api/cards/${encodeURIComponent(input.id)}`, {
        method: "PATCH",
        body: JSON.stringify({ title: input.title, body: input.body }),
      });
    }
    case "seed_demo":
      return http("/api/seed", { method: "POST", body: "{}" });
    case "timeline": {
      const limit = (args?.limit as number | undefined) ?? 40;
      return http(`/api/timeline?limit=${limit}`);
    }
    case "latest_event_id":
      return http("/api/events/latest-id");
    case "list_activity": {
      const limit = (args?.limit as number | undefined) ?? 40;
      const after = (args?.after_id as number | undefined) ?? 0;
      return http(`/api/activity?limit=${limit}&after_id=${after}`);
    }
    case "db_path":
      return http("/api/db-path");
    case "get_gitlab_settings":
      return http("/api/settings/gitlab");
    case "save_gitlab_settings":
      return http("/api/settings/gitlab", {
        method: "PUT",
        body: JSON.stringify(args?.input ?? {}),
      });
    case "gitlab_doctor":
      return http("/api/gitlab/doctor", { method: "POST", body: "{}" });
    case "gitlab_import":
      return http("/api/gitlab/import", {
        method: "POST",
        body: JSON.stringify(args?.input ?? {}),
      });
    case "gitlab_sync":
      return http("/api/gitlab/sync", {
        method: "POST",
        body: JSON.stringify(args?.input ?? {}),
      });
    default:
      throw new Error(`Unknown API command: ${cmd}`);
  }
}