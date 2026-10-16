use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const DEFAULT_COLUMNS: &[&str] = &["backlog", "ready", "doing", "blocked", "done"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    Ready,
    Doing,
    Blocked,
    Done,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Ready => "ready",
            Self::Doing => "doing",
            Self::Blocked => "blocked",
            Self::Done => "done",
        }
    }

    pub fn parse(s: &str) -> crate::Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "backlog" => Ok(Self::Backlog),
            "ready" => Ok(Self::Ready),
            "doing" => Ok(Self::Doing),
            "blocked" => Ok(Self::Blocked),
            "done" => Ok(Self::Done),
            other => Err(crate::Error::InvalidStatus(other.to_string())),
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssigneeKind {
    Unassigned,
    Human,
    Agent,
}

impl AssigneeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unassigned => "unassigned",
            Self::Human => "human",
            Self::Agent => "agent",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "human" => Self::Human,
            "agent" => Self::Agent,
            _ => Self::Unassigned,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    Blocks,
    BlockedBy,
    Relates,
    Parent,
    Child,
    External,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::BlockedBy => "blocked_by",
            Self::Relates => "relates",
            Self::Parent => "parent",
            Self::Child => "child",
            Self::External => "external",
        }
    }

    pub fn parse(s: &str) -> crate::Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "blocks" => Ok(Self::Blocks),
            "blocked_by" | "blocked-by" => Ok(Self::BlockedBy),
            "relates" | "related" => Ok(Self::Relates),
            "parent" => Ok(Self::Parent),
            "child" => Ok(Self::Child),
            "external" => Ok(Self::External),
            other => Err(crate::Error::InvalidLinkType(other.to_string())),
        }
    }

    pub fn inverse(&self) -> Option<Self> {
        match self {
            Self::Blocks => Some(Self::BlockedBy),
            Self::BlockedBy => Some(Self::Blocks),
            Self::Parent => Some(Self::Child),
            Self::Child => Some(Self::Parent),
            Self::Relates => Some(Self::Relates),
            Self::External => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub key: String,
    pub title: String,
    pub body: String,
    pub status: Status,
    pub labels: Vec<String>,
    pub assignee_kind: AssigneeKind,
    pub assignee_name: Option<String>,
    pub priority: i32,
    pub external_url: Option<String>,
    /// When set and in the future, another agent holds an exclusive claim.
    #[serde(default)]
    pub claim_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub holder_kind: String,
    pub holder_name: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardLink {
    pub id: i64,
    pub from_id: String,
    pub to_id: String,
    pub link_type: LinkType,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub actor_kind: String,
    pub actor_name: Option<String>,
    pub card_id: Option<String>,
    pub kind: String,
    pub payload: serde_json::Value,
}

/// Timeline row enriched with card key/title for the desktop activity feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub actor_kind: String,
    pub actor_name: Option<String>,
    pub card_id: Option<String>,
    pub card_key: Option<String>,
    pub card_title: Option<String>,
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDetail {
    pub card: Card,
    pub links: Vec<CardLink>,
    pub recent_events: Vec<Event>,
    #[serde(default)]
    pub lease: Option<Lease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimCard {
    pub ttl_minutes: i64,
    pub note: Option<String>,
    pub actor_kind: String,
    pub actor_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateCard {
    pub title: String,
    pub body: Option<String>,
    pub status: Option<Status>,
    pub labels: Vec<String>,
    pub assignee_kind: Option<AssigneeKind>,
    pub assignee_name: Option<String>,
    pub priority: Option<i32>,
    pub external_url: Option<String>,
    pub actor_kind: Option<String>,
    pub actor_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCard {
    pub title: Option<String>,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignee_kind: Option<AssigneeKind>,
    pub assignee_name: Option<String>,
    pub priority: Option<i32>,
    pub external_url: Option<String>,
    pub actor_kind: Option<String>,
    pub actor_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListFilter {
    pub status: Option<Status>,
    pub assignee_kind: Option<AssigneeKind>,
    pub label: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardSnapshot {
    pub columns: Vec<ColumnSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSnapshot {
    pub status: Status,
    pub cards: Vec<Card>,
}