#!/usr/bin/env node
/**
 * Local mock API + static preview for UI verification without Tauri/SQLite.
 */
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const dist = path.resolve(__dirname, "../dist");
const port = Number(process.env.PORT || 4173);

const now = () => new Date().toISOString();
let seq = 4;

const cards = [
  {
    id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1",
    key: "CARD-1",
    title: "Define board CLI contract",
    body: "Stable --json surface for agents.",
    status: "doing",
    labels: ["core", "cli"],
    assignee_kind: "agent",
    assignee_name: "cursor",
    priority: 0,
    external_url: null,
    created_at: now(),
    updated_at: now(),
  },
  {
    id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2",
    key: "CARD-2",
    title: "Kanban desktop shell",
    body: "Tauri window + board layout.",
    status: "ready",
    labels: ["ui"],
    assignee_kind: "human",
    assignee_name: "you",
    priority: 0,
    external_url: null,
    created_at: now(),
    updated_at: now(),
  },
  {
    id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3",
    key: "CARD-3",
    title: "Card links + timeline",
    body: "blocks/relates edges and append-only events.",
    status: "backlog",
    labels: ["core"],
    assignee_kind: "unassigned",
    assignee_name: null,
    priority: 0,
    external_url: null,
    created_at: now(),
    updated_at: now(),
  },
];

let events = [
  {
    id: 1,
    at: now(),
    actor_kind: "human",
    actor_name: "ai",
    card_id: cards[0].id,
    kind: "created",
    payload: {},
  },
  {
    id: 2,
    at: now(),
    actor_kind: "human",
    actor_name: "ai",
    card_id: cards[0].id,
    kind: "moved",
    payload: { to: "doing" },
  },
  {
    id: 3,
    at: now(),
    actor_kind: "human",
    actor_name: "ai",
    card_id: cards[2].id,
    kind: "linked",
    payload: {},
  },
];

const COLUMNS = ["backlog", "ready", "doing", "blocked", "done"];

function board() {
  return {
    columns: COLUMNS.map((status) => ({
      status,
      cards: cards.filter((c) => c.status === status),
    })),
  };
}

function detail(id) {
  const card = cards.find((c) => c.id === id || c.key === id);
  if (!card) return null;
  return {
    card,
    links:
      card.key === "CARD-3"
        ? [
            {
              id: 1,
              from_id: card.id,
              to_id: cards[0].id,
              link_type: "blocked_by",
              note: "CLI contract needed before rich links UX",
            },
          ]
        : [],
    recent_events: events.filter((e) => e.card_id === card.id),
  };
}

function json(res, status, body) {
  const payload = body === undefined ? "" : JSON.stringify(body);
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Access-Control-Allow-Origin": "*",
  });
  res.end(payload);
}

function mime(file) {
  if (file.endsWith(".html")) return "text/html";
  if (file.endsWith(".js")) return "application/javascript";
  if (file.endsWith(".css")) return "text/css";
  if (file.endsWith(".svg")) return "image/svg+xml";
  if (file.endsWith(".png")) return "image/png";
  return "application/octet-stream";
}

function serveStatic(req, res) {
  let urlPath = req.url.split("?")[0];
  if (urlPath === "/") urlPath = "/index.html";
  const file = path.join(dist, urlPath);
  if (!file.startsWith(dist) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.writeHead(404);
    res.end("Not found");
    return;
  }
  res.writeHead(200, { "Content-Type": mime(file) });
  fs.createReadStream(file).pipe(res);
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${port}`);
  const { pathname } = url;

  if (req.method === "OPTIONS") {
    res.writeHead(204, {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET,POST,PATCH,OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
    });
    res.end();
    return;
  }

  let body = "";
  if (req.method === "POST" || req.method === "PATCH") {
    for await (const chunk of req) body += chunk;
  }
  const parsed = body ? JSON.parse(body) : {};

  if (pathname === "/api/board") return json(res, 200, board());
  if (pathname === "/api/db-path") return json(res, 200, "/tmp/laneboard-mock/board.db");
  if (pathname === "/api/timeline") return json(res, 200, events.slice().reverse());
  if (pathname === "/api/cards" && req.method === "GET") {
    const q = (url.searchParams.get("q") || "").toLowerCase();
    const list = q
      ? cards.filter(
          (c) =>
            c.title.toLowerCase().includes(q) ||
            c.key.toLowerCase().includes(q) ||
            c.labels.some((l) => l.includes(q)),
        )
      : cards;
    return json(res, 200, list);
  }
  if (pathname === "/api/cards" && req.method === "POST") {
    const card = {
      id: `aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaa${seq}`,
      key: `CARD-${seq}`,
      title: parsed.title || "Untitled",
      body: parsed.body || "",
      status: parsed.status || "backlog",
      labels: [],
      assignee_kind: "unassigned",
      assignee_name: null,
      priority: 0,
      external_url: null,
      created_at: now(),
      updated_at: now(),
    };
    seq += 1;
    cards.push(card);
    events.push({
      id: events.length + 1,
      at: now(),
      actor_kind: "human",
      actor_name: "you",
      card_id: card.id,
      kind: "created",
      payload: {},
    });
    return json(res, 200, card);
  }
  if (pathname.startsWith("/api/cards/") && pathname.endsWith("/move") && req.method === "POST") {
    const id = decodeURIComponent(pathname.split("/")[3]);
    const card = cards.find((c) => c.id === id || c.key === id);
    if (!card) return json(res, 404, { error: "not found" });
    card.status = parsed.to;
    card.updated_at = now();
    events.push({
      id: events.length + 1,
      at: now(),
      actor_kind: "human",
      actor_name: "you",
      card_id: card.id,
      kind: "moved",
      payload: { to: parsed.to },
    });
    return json(res, 200, card);
  }
  if (pathname.startsWith("/api/cards/") && req.method === "GET") {
    const id = decodeURIComponent(pathname.split("/")[3]);
    const d = detail(id);
    if (!d) return json(res, 404, { error: "not found" });
    return json(res, 200, d);
  }
  if (pathname === "/api/seed" && req.method === "POST") {
    return json(res, 200, { ok: true });
  }

  return serveStatic(req, res);
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Laneboard mock UI → http://127.0.0.1:${port}`);
});
