export type Status = "backlog" | "ready" | "doing" | "blocked" | "done";

export type AssigneeKind = "unassigned" | "human" | "agent";

export interface Card {
  id: string;
  key: string;
  title: string;
  body: string;
  status: Status;
  labels: string[];
  assignee_kind: AssigneeKind;
  assignee_name: string | null;
  priority: number;
  external_url: string | null;
  claim_expires_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface Lease {
  holder_kind: string;
  holder_name: string | null;
  expires_at: string;
}

export interface ColumnSnapshot {
  status: Status;
  cards: Card[];
}

export interface BoardSnapshot {
  columns: ColumnSnapshot[];
}

export interface Event {
  id: number;
  at: string;
  actor_kind: string;
  actor_name: string | null;
  card_id: string | null;
  kind: string;
  payload: unknown;
}

export interface CardDetail {
  card: Card;
  links: Array<{
    id: number;
    from_id: string;
    to_id: string;
    link_type: string;
    note: string | null;
  }>;
  recent_events: Event[];
  lease?: Lease | null;
}

export const COLUMNS: Status[] = [
  "backlog",
  "ready",
  "doing",
  "blocked",
  "done",
];

export const COLUMN_LABELS: Record<Status, string> = {
  backlog: "Backlog",
  ready: "Ready",
  doing: "Doing",
  blocked: "Blocked",
  done: "Done",
};