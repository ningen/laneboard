import type { ActivityItem } from "./types";

const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Notable status transitions that deserve an OS notification. */
export function shouldNotify(item: ActivityItem): boolean {
  // Skip echoes of local desktop / board-ui actions.
  if (item.actor_name === "desktop" || item.actor_name === "ui") {
    return false;
  }
  if (item.kind === "moved") {
    const to =
      item.payload &&
      typeof item.payload === "object" &&
      item.payload !== null &&
      "to" in item.payload
        ? String((item.payload as { to?: unknown }).to ?? "")
        : "";
    return to === "blocked" || to === "done";
  }
  if (item.kind === "claimed" || item.kind === "released") {
    return item.actor_kind === "agent";
  }
  return false;
}

export function notificationCopy(item: ActivityItem): { title: string; body: string } {
  const key = item.card_key || "Card";
  const title = item.card_title || key;
  const actor =
    item.actor_name != null ? `${item.actor_kind}/${item.actor_name}` : item.actor_kind;

  if (item.kind === "moved") {
    const to =
      item.payload &&
      typeof item.payload === "object" &&
      item.payload !== null &&
      "to" in item.payload
        ? String((item.payload as { to?: unknown }).to ?? "")
        : "updated";
    return {
      title: `${key} → ${to}`,
      body: `${actor}: ${title}`,
    };
  }

  return {
    title: `${key} · ${item.kind}`,
    body: `${actor}: ${title}`,
  };
}

let permissionReady = false;

async function ensurePermission(): Promise<boolean> {
  if (!isTauri()) return false;
  if (permissionReady) return true;
  try {
    const { isPermissionGranted, requestPermission } = await import(
      "@tauri-apps/plugin-notification"
    );
    let granted = await isPermissionGranted();
    if (!granted) {
      const perm = await requestPermission();
      granted = perm === "granted";
    }
    permissionReady = granted;
    return granted;
  } catch {
    return false;
  }
}

export async function notifyActivity(items: ActivityItem[]): Promise<void> {
  const notable = items.filter(shouldNotify);
  if (notable.length === 0) return;
  if (!(await ensurePermission())) return;

  try {
    const { sendNotification } = await import("@tauri-apps/plugin-notification");
    // Cap burst notifications when catching up after sleep.
    for (const item of notable.slice(0, 3)) {
      const copy = notificationCopy(item);
      sendNotification({ title: copy.title, body: copy.body });
    }
  } catch {
    /* notification plugin unavailable (browser / board ui) */
  }
}
