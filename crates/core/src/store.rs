use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use uuid::Uuid;

use crate::models::*;
use crate::{Error, Result};

pub struct BoardStore {
    conn: Connection,
}

impl BoardStore {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Ok(Self {
            conn: crate::db::open_db(path)?,
        })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(crate::db::default_db_path())
    }

    pub fn path_hint() -> std::path::PathBuf {
        crate::db::default_db_path()
    }

    fn next_key(&self) -> Result<String> {
        let n: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(CAST(SUBSTR(key, 6) AS INTEGER)), 0) + 1 FROM cards WHERE key LIKE 'CARD-%'",
            [],
            |r| r.get(0),
        )?;
        Ok(format!("CARD-{n}"))
    }

    fn parse_labels(raw: &str) -> Vec<String> {
        serde_json::from_str(raw).unwrap_or_default()
    }

    fn row_to_card(row: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
        let status: String = row.get("status")?;
        let assignee_kind: String = row.get("assignee_kind")?;
        let labels_json: String = row.get("labels_json")?;
        let created_at: String = row.get("created_at")?;
        let updated_at: String = row.get("updated_at")?;
        let claim_expires_at: Option<String> = row.get("claim_expires_at")?;
        Ok(Card {
            id: row.get("id")?,
            key: row.get("key")?,
            title: row.get("title")?,
            body: row.get("body")?,
            status: Status::parse(&status).unwrap_or(Status::Backlog),
            labels: Self::parse_labels(&labels_json),
            assignee_kind: AssigneeKind::parse(&assignee_kind),
            assignee_name: row.get("assignee_name")?,
            priority: row.get("priority")?,
            external_url: row.get("external_url")?,
            claim_expires_at: claim_expires_at.and_then(|s| s.parse().ok()),
            created_at: created_at.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: updated_at.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    /// Active lease if assignee holds an unexpired claim.
    pub fn lease_of(card: &Card) -> Option<Lease> {
        let expires_at = card.claim_expires_at?;
        if expires_at <= Utc::now() {
            return None;
        }
        if matches!(card.assignee_kind, AssigneeKind::Unassigned) {
            return None;
        }
        Some(Lease {
            holder_kind: card.assignee_kind.as_str().to_string(),
            holder_name: card.assignee_name.clone(),
            expires_at,
        })
    }

    fn same_actor(card: &Card, actor_kind: &str, actor_name: Option<&str>) -> bool {
        card.assignee_kind.as_str() == actor_kind
            && card.assignee_name.as_deref() == actor_name
            && !matches!(card.assignee_kind, AssigneeKind::Unassigned)
    }

    /// Cards that block `id` and are not yet done (`blocked_by` edges).
    pub fn open_blockers(&self, id_or_key: &str) -> Result<Vec<Card>> {
        let id = self.resolve_card_id(id_or_key)?;
        let mut stmt = self.conn.prepare(
            "SELECT c.* FROM card_links l
             JOIN cards c ON c.id = l.to_id
             WHERE l.from_id = ?1 AND l.link_type = 'blocked_by' AND c.status != 'done'
             ORDER BY c.key",
        )?;
        let rows = stmt.query_map([&id], Self::row_to_card)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn ensure_unblocked(&self, id_or_key: &str) -> Result<()> {
        let blockers = self.open_blockers(id_or_key)?;
        if blockers.is_empty() {
            return Ok(());
        }
        let keys: Vec<&str> = blockers.iter().map(|c| c.key.as_str()).collect();
        Err(Error::BlockedByDeps(format!(
            "{} is blocked by: {}",
            id_or_key,
            keys.join(", ")
        )))
    }

    fn resolve_card_id(&self, id_or_key: &str) -> Result<String> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM cards WHERE id = ?1 OR key = ?1 OR lower(key) = lower(?1)",
                [id_or_key],
                |r| r.get(0),
            )
            .optional()?;
        found.ok_or_else(|| Error::NotFound(id_or_key.to_string()))
    }

    /// Find a card previously imported/linked by its external URL (exact match).
    pub fn find_by_external_url(&self, url: &str) -> Result<Option<Card>> {
        let url = url.trim();
        if url.is_empty() {
            return Ok(None);
        }
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM cards WHERE external_url = ?1 LIMIT 1")?;
        let mut rows = stmt.query_map([url], Self::row_to_card)?;
        match rows.next() {
            Some(Ok(card)) => Ok(Some(card)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Cards whose external_url is set (GitLab / GitHub / … links).
    pub fn list_with_external_url(&self) -> Result<Vec<Card>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM cards WHERE external_url IS NOT NULL AND external_url != ''
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_card)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(found)
    }

    pub fn put_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn gitlab_settings(&self) -> Result<crate::gitlab::GitLabSettings> {
        match self.get_meta(crate::gitlab::SETTINGS_META_KEY)? {
            Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            None => Ok(crate::gitlab::GitLabSettings::default()),
        }
    }

    pub fn gitlab_settings_view(&self) -> Result<crate::gitlab::GitLabSettingsView> {
        Ok(self.gitlab_settings()?.view())
    }

    pub fn save_gitlab_settings(
        &self,
        update: crate::gitlab::GitLabSettingsUpdate,
    ) -> Result<crate::gitlab::GitLabSettingsView> {
        let mut settings = self.gitlab_settings()?;
        settings.apply_update(update);
        let raw = serde_json::to_string(&settings)?;
        self.put_meta(crate::gitlab::SETTINGS_META_KEY, &raw)?;
        Ok(settings.view())
    }

    fn append_event(
        &self,
        card_id: Option<&str>,
        kind: &str,
        actor_kind: &str,
        actor_name: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events(at, actor_kind, actor_name, card_id, kind, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Utc::now().to_rfc3339(),
                actor_kind,
                actor_name,
                card_id,
                kind,
                payload.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn create_card(&self, input: CreateCard) -> Result<Card> {
        let id = Uuid::new_v4().to_string();
        let key = self.next_key()?;
        let now = Utc::now();
        let status = input.status.unwrap_or(Status::Backlog);
        let assignee_kind = input.assignee_kind.unwrap_or(AssigneeKind::Unassigned);
        let body = input.body.unwrap_or_default();
        let labels_json = serde_json::to_string(&input.labels)?;
        let priority = input.priority.unwrap_or(0);
        let actor_kind = input.actor_kind.as_deref().unwrap_or("human");
        let actor_name = input.actor_name.as_deref();

        self.conn.execute(
            "INSERT INTO cards(id, key, title, body, status, labels_json, assignee_kind, assignee_name, priority, external_url, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                id,
                key,
                input.title,
                body,
                status.as_str(),
                labels_json,
                assignee_kind.as_str(),
                input.assignee_name,
                priority,
                input.external_url,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )?;

        self.append_event(
            Some(&id),
            "created",
            actor_kind,
            actor_name,
            json!({ "key": key, "title": input.title, "status": status.as_str() }),
        )?;

        self.get_card(&id)
    }

    pub fn get_card(&self, id_or_key: &str) -> Result<Card> {
        let id = self.resolve_card_id(id_or_key)?;
        self.conn
            .query_row(
                "SELECT * FROM cards WHERE id = ?1",
                [&id],
                Self::row_to_card,
            )
            .map_err(|_| Error::NotFound(id_or_key.to_string()))
    }

    pub fn get_card_detail(&self, id_or_key: &str, event_limit: usize) -> Result<CardDetail> {
        let card = self.get_card(id_or_key)?;
        let lease = Self::lease_of(&card);
        let links = self.list_links(&card.id)?;
        let recent_events = self.list_events(Some(&card.id), event_limit)?;
        Ok(CardDetail {
            card,
            links,
            recent_events,
            lease,
        })
    }

    /// Ready cards with no active lease (default MCP `board_list`).
    pub fn list_claimable(&self, limit: usize) -> Result<Vec<Card>> {
        let now = Utc::now().to_rfc3339();
        let limit = limit.max(1) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT * FROM cards
             WHERE status = 'ready'
               AND (
                 assignee_kind = 'unassigned'
                 OR claim_expires_at IS NULL
                 OR claim_expires_at <= ?1
               )
             ORDER BY priority DESC, updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![now, limit], Self::row_to_card)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn claim_card(&self, id_or_key: &str, input: ClaimCard) -> Result<Card> {
        let id = self.resolve_card_id(id_or_key)?;
        let card = self.get_card(&id)?;
        if let Some(lease) = Self::lease_of(&card) {
            if !Self::same_actor(&card, &input.actor_kind, input.actor_name.as_deref()) {
                return Err(Error::AlreadyClaimed(format!(
                    "{} is claimed by {}:{} until {}",
                    card.key,
                    lease.holder_kind,
                    lease.holder_name.as_deref().unwrap_or("?"),
                    lease.expires_at.to_rfc3339()
                )));
            }
            // Same actor may renew an active lease while already doing.
        } else if !matches!(card.status, Status::Ready) {
            return Err(Error::NotClaimable(format!(
                "{} is {}, only ready cards can be claimed",
                card.key, card.status
            )));
        }
        self.ensure_unblocked(&id)?;

        let ttl = input.ttl_minutes.clamp(5, 240);
        let expires = Utc::now() + chrono::Duration::minutes(ttl);
        let now = Utc::now();
        self.conn.execute(
            "UPDATE cards SET status = 'doing', assignee_kind = ?1, assignee_name = ?2,
             claim_expires_at = ?3, updated_at = ?4 WHERE id = ?5",
            params![
                input.actor_kind,
                input.actor_name,
                expires.to_rfc3339(),
                now.to_rfc3339(),
                id,
            ],
        )?;
        self.append_event(
            Some(&id),
            "claimed",
            &input.actor_kind,
            input.actor_name.as_deref(),
            json!({
                "ttl_minutes": ttl,
                "expires_at": expires.to_rfc3339(),
                "note": input.note,
            }),
        )?;
        self.get_card(&id)
    }

    pub fn release_card(
        &self,
        id_or_key: &str,
        to: Status,
        note: Option<&str>,
        actor_kind: &str,
        actor_name: Option<&str>,
    ) -> Result<Card> {
        if !matches!(to, Status::Ready | Status::Blocked) {
            return Err(Error::Validation(
                "release target must be ready or blocked".into(),
            ));
        }
        let id = self.resolve_card_id(id_or_key)?;
        let card = self.get_card(&id)?;
        if let Some(lease) = Self::lease_of(&card) {
            if !Self::same_actor(&card, actor_kind, actor_name) {
                return Err(Error::AlreadyClaimed(format!(
                    "{} is claimed by {}:{} until {}",
                    card.key,
                    lease.holder_kind,
                    lease.holder_name.as_deref().unwrap_or("?"),
                    lease.expires_at.to_rfc3339()
                )));
            }
        }
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE cards SET status = ?1, assignee_kind = 'unassigned', assignee_name = NULL,
             claim_expires_at = NULL, updated_at = ?2 WHERE id = ?3",
            params![to.as_str(), now, id],
        )?;
        self.append_event(
            Some(&id),
            "released",
            actor_kind,
            actor_name,
            json!({ "to": to.as_str(), "note": note }),
        )?;
        self.get_card(&id)
    }

    pub fn add_note(
        &self,
        id_or_key: &str,
        note: &str,
        actor_kind: &str,
        actor_name: Option<&str>,
    ) -> Result<Card> {
        let note = note.trim();
        if note.is_empty() {
            return Err(Error::Validation("note must not be empty".into()));
        }
        let id = self.resolve_card_id(id_or_key)?;
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute("UPDATE cards SET updated_at = ?1 WHERE id = ?2", params![now, id])?;
        self.append_event(
            Some(&id),
            "note",
            actor_kind,
            actor_name,
            json!({ "text": note }),
        )?;
        self.get_card(&id)
    }

    pub fn list_cards(&self, filter: &ListFilter) -> Result<Vec<Card>> {
        if let Some(q) = filter.query.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            return self.search(q, filter);
        }

        let mut sql = String::from("SELECT * FROM cards WHERE 1=1");
        let mut vals: Vec<String> = Vec::new();

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            vals.push(status.as_str().to_string());
        }
        if let Some(kind) = &filter.assignee_kind {
            sql.push_str(" AND assignee_kind = ?");
            vals.push(kind.as_str().to_string());
        }
        if let Some(label) = &filter.label {
            sql.push_str(" AND labels_json LIKE ?");
            vals.push(format!("%{label}%"));
        }
        sql.push_str(" ORDER BY priority DESC, updated_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = vals
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_card)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn search(&self, query: &str, filter: &ListFilter) -> Result<Vec<Card>> {
        let mut sql = String::from(
            "SELECT c.* FROM cards c
             JOIN cards_fts f ON c.rowid = f.rowid
             WHERE cards_fts MATCH ?1",
        );
        let mut vals: Vec<String> = vec![query.to_string()];

        if let Some(status) = &filter.status {
            sql.push_str(" AND c.status = ?");
            vals.push(status.as_str().to_string());
        }
        if let Some(kind) = &filter.assignee_kind {
            sql.push_str(" AND c.assignee_kind = ?");
            vals.push(kind.as_str().to_string());
        }
        if let Some(label) = &filter.label {
            sql.push_str(" AND c.labels_json LIKE ?");
            vals.push(format!("%{label}%"));
        }
        sql.push_str(" ORDER BY c.priority DESC, c.updated_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = vals
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_card)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn board_snapshot(&self) -> Result<BoardSnapshot> {
        let cards = self.list_cards(&ListFilter::default())?;
        let mut columns = Vec::new();
        for status in [
            Status::Backlog,
            Status::Ready,
            Status::Doing,
            Status::Blocked,
            Status::Done,
        ] {
            let col_cards: Vec<Card> = cards
                .iter()
                .filter(|c| c.status == status)
                .cloned()
                .collect();
            columns.push(ColumnSnapshot {
                status,
                cards: col_cards,
            });
        }
        Ok(BoardSnapshot { columns })
    }

    pub fn move_card(
        &self,
        id_or_key: &str,
        to: Status,
        note: Option<&str>,
        actor_kind: &str,
        actor_name: Option<&str>,
    ) -> Result<Card> {
        let id = self.resolve_card_id(id_or_key)?;
        let before = self.get_card(&id)?;
        if matches!(to, Status::Done | Status::Doing) {
            self.ensure_unblocked(&id)?;
        }
        let now = Utc::now().to_rfc3339();
        let clear_lease = matches!(to, Status::Done | Status::Blocked | Status::Backlog | Status::Ready);
        if clear_lease {
            self.conn.execute(
                "UPDATE cards SET status = ?1, claim_expires_at = NULL, updated_at = ?2 WHERE id = ?3",
                params![to.as_str(), now, id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE cards SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![to.as_str(), now, id],
            )?;
        }
        self.append_event(
            Some(&id),
            "moved",
            actor_kind,
            actor_name,
            json!({
                "from": before.status.as_str(),
                "to": to.as_str(),
                "note": note,
            }),
        )?;
        self.get_card(&id)
    }

    pub fn update_card(&self, id_or_key: &str, input: UpdateCard) -> Result<Card> {
        let id = self.resolve_card_id(id_or_key)?;
        let mut card = self.get_card(&id)?;
        if let Some(title) = input.title {
            card.title = title;
        }
        if let Some(body) = input.body {
            card.body = body;
        }
        if let Some(labels) = input.labels {
            card.labels = labels;
        }
        if let Some(kind) = input.assignee_kind {
            card.assignee_kind = kind;
        }
        if input.assignee_name.is_some() {
            card.assignee_name = input.assignee_name;
        }
        if let Some(priority) = input.priority {
            card.priority = priority;
        }
        if input.external_url.is_some() {
            card.external_url = input.external_url;
        }
        let now = Utc::now();
        card.updated_at = now;
        let labels_json = serde_json::to_string(&card.labels)?;
        let claim_expires = card
            .claim_expires_at
            .map(|t| t.to_rfc3339());
        self.conn.execute(
            "UPDATE cards SET title=?1, body=?2, labels_json=?3, assignee_kind=?4, assignee_name=?5, priority=?6, external_url=?7, claim_expires_at=?8, updated_at=?9 WHERE id=?10",
            params![
                card.title,
                card.body,
                labels_json,
                card.assignee_kind.as_str(),
                card.assignee_name,
                card.priority,
                card.external_url,
                claim_expires,
                now.to_rfc3339(),
                id,
            ],
        )?;
        let actor_kind = input.actor_kind.as_deref().unwrap_or("human");
        let actor_name = input.actor_name.as_deref();
        self.append_event(
            Some(&id),
            "updated",
            actor_kind,
            actor_name,
            json!({ "key": card.key }),
        )?;
        self.get_card(&id)
    }

    pub fn delete_card(&self, id_or_key: &str, actor_kind: &str, actor_name: Option<&str>) -> Result<()> {
        let card = self.get_card(id_or_key)?;
        self.append_event(
            Some(&card.id),
            "deleted",
            actor_kind,
            actor_name,
            json!({ "key": card.key, "title": card.title }),
        )?;
        self.conn
            .execute("DELETE FROM cards WHERE id = ?1", [&card.id])?;
        Ok(())
    }

    pub fn link_cards(
        &self,
        from: &str,
        to: &str,
        link_type: LinkType,
        note: Option<&str>,
        actor_kind: &str,
        actor_name: Option<&str>,
    ) -> Result<CardLink> {
        let from_id = self.resolve_card_id(from)?;
        let to_id = self.resolve_card_id(to)?;
        if from_id == to_id {
            return Err(Error::msg("cannot link a card to itself"));
        }
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR IGNORE INTO card_links(from_id, to_id, link_type, note, created_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![from_id, to_id, link_type.as_str(), note, now],
        )?;
        if let Some(inv) = link_type.inverse() {
            if !matches!(link_type, LinkType::Relates) {
                self.conn.execute(
                    "INSERT OR IGNORE INTO card_links(from_id, to_id, link_type, note, created_at)
                     VALUES (?1,?2,?3,?4,?5)",
                    params![to_id, from_id, inv.as_str(), note, now],
                )?;
            } else {
                self.conn.execute(
                    "INSERT OR IGNORE INTO card_links(from_id, to_id, link_type, note, created_at)
                     VALUES (?1,?2,?3,?4,?5)",
                    params![to_id, from_id, LinkType::Relates.as_str(), note, now],
                )?;
            }
        }
        self.append_event(
            Some(&from_id),
            "linked",
            actor_kind,
            actor_name,
            json!({ "to": to_id, "type": link_type.as_str(), "note": note }),
        )?;
        self.list_links(&from_id)?
            .into_iter()
            .find(|l| l.to_id == to_id && l.link_type == link_type)
            .ok_or_else(|| Error::msg("link not found after insert"))
    }

    pub fn list_links(&self, id_or_key: &str) -> Result<Vec<CardLink>> {
        let id = self.resolve_card_id(id_or_key)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, from_id, to_id, link_type, note, created_at FROM card_links
             WHERE from_id = ?1 OR to_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([&id], |row| {
            let link_type: String = row.get(3)?;
            let created_at: String = row.get(5)?;
            Ok(CardLink {
                id: row.get(0)?,
                from_id: row.get(1)?,
                to_id: row.get(2)?,
                link_type: LinkType::parse(&link_type).unwrap_or(LinkType::Relates),
                note: row.get(4)?,
                created_at: created_at.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn list_events(&self, card_id: Option<&str>, limit: usize) -> Result<Vec<Event>> {
        let limit = limit.max(1) as i64;
        if let Some(id_or_key) = card_id {
            let id = self.resolve_card_id(id_or_key)?;
            let mut stmt = self.conn.prepare(
                "SELECT id, at, actor_kind, actor_name, card_id, kind, payload_json
                 FROM events WHERE card_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![id, limit], Self::row_to_event)?;
            collect_events(rows)
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, at, actor_kind, actor_name, card_id, kind, payload_json
                 FROM events ORDER BY at DESC, id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit], Self::row_to_event)?;
            collect_events(rows)
        }
    }

    /// Highest event id (0 when the board has no events yet). Used for live polling.
    pub fn latest_event_id(&self) -> Result<i64> {
        let id: i64 = self
            .conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |row| row.get(0))?;
        Ok(id)
    }

    /// Activity feed with card key/title joined in (newest first).
    pub fn list_activity(&self, limit: usize) -> Result<Vec<ActivityItem>> {
        self.list_activity_since(0, limit)
    }

    /// Events with `id > after_id`, newest first (for incremental live updates).
    pub fn list_activity_since(&self, after_id: i64, limit: usize) -> Result<Vec<ActivityItem>> {
        let limit = limit.max(1) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.at, e.actor_kind, e.actor_name, e.card_id, e.kind, e.payload_json,
                    c.key, c.title
             FROM events e
             LEFT JOIN cards c ON c.id = e.card_id
             WHERE e.id > ?1
             ORDER BY e.at DESC, e.id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_id, limit], Self::row_to_activity)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
        let at: String = row.get(1)?;
        let payload_json: String = row.get(6)?;
        Ok(Event {
            id: row.get(0)?,
            at: at.parse().unwrap_or_else(|_| Utc::now()),
            actor_kind: row.get(2)?,
            actor_name: row.get(3)?,
            card_id: row.get(4)?,
            kind: row.get(5)?,
            payload: serde_json::from_str(&payload_json).unwrap_or(json!({})),
        })
    }

    fn row_to_activity(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityItem> {
        let at: String = row.get(1)?;
        let payload_json: String = row.get(6)?;
        Ok(ActivityItem {
            id: row.get(0)?,
            at: at.parse().unwrap_or_else(|_| Utc::now()),
            actor_kind: row.get(2)?,
            actor_name: row.get(3)?,
            card_id: row.get(4)?,
            kind: row.get(5)?,
            payload: serde_json::from_str(&payload_json).unwrap_or(json!({})),
            card_key: row.get(7)?,
            card_title: row.get(8)?,
        })
    }

    pub fn export_json(&self) -> Result<serde_json::Value> {
        let cards = self.list_cards(&ListFilter::default())?;
        let mut all_links = Vec::new();
        for card in &cards {
            all_links.extend(self.list_links(&card.id)?);
        }
        // dedupe by link id
        all_links.sort_by_key(|l| l.id);
        all_links.dedup_by_key(|l| l.id);
        let events = self.list_events(None, 500)?;
        Ok(json!({
            "cards": cards,
            "links": all_links,
            "events": events,
        }))
    }

    pub fn seed_demo(&self) -> Result<Vec<Card>> {
        let existing = self.list_cards(&ListFilter::default())?;
        if !existing.is_empty() {
            return Ok(existing);
        }
        let a = self.create_card(CreateCard {
            title: "Define board CLI contract".into(),
            body: Some("Ship create/list/move/search/get --json for agents.".into()),
            status: Some(Status::Doing),
            labels: vec!["core".into(), "cli".into()],
            assignee_kind: Some(AssigneeKind::Agent),
            assignee_name: Some("cursor".into()),
            priority: Some(10),
            actor_kind: Some("agent".into()),
            actor_name: Some("seed".into()),
            ..Default::default()
        })?;
        let b = self.create_card(CreateCard {
            title: "Kanban desktop shell".into(),
            body: Some("Tauri window with columns humans can drag.".into()),
            status: Some(Status::Ready),
            labels: vec!["ui".into()],
            assignee_kind: Some(AssigneeKind::Human),
            assignee_name: Some("you".into()),
            priority: Some(8),
            ..Default::default()
        })?;
        let c = self.create_card(CreateCard {
            title: "Card links + timeline".into(),
            body: Some("blocks/relates edges and append-only events.".into()),
            status: Some(Status::Backlog),
            labels: vec!["core".into()],
            priority: Some(5),
            ..Default::default()
        })?;
        self.link_cards(
            &a.key,
            &c.key,
            LinkType::Blocks,
            Some("CLI contract needed before rich links UX"),
            "agent",
            Some("seed"),
        )?;
        self.move_card(&b.key, Status::Ready, None, "human", Some("seed"))?;
        Ok(vec![a, b, c])
    }
}

fn collect_events(
    rows: impl IntoIterator<Item = rusqlite::Result<Event>>,
) -> Result<Vec<Event>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}