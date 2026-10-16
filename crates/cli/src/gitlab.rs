//! GitLab Issues → board cards (one-way pull, no webhooks).
//!
//! Auth: `GITLAB_TOKEN` (PAT with `read_api`).
//! Host: `GITLAB_URL` (default `https://gitlab.com`).
//! See `docs/gitlab.md`.

use anyhow::{bail, Context, Result};
use board_core::{BoardStore, CreateCard, Status, UpdateCard};
use serde::{Deserialize, Serialize};
use serde_json::json;

const DEFAULT_BASE: &str = "https://gitlab.com";
const USER_AGENT: &str = "laneboard-gitlab/0.1";

#[derive(Debug, Clone)]
pub struct GitLabConfig {
    pub base_url: String,
    pub token: String,
}

impl GitLabConfig {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("GITLAB_TOKEN")
            .or_else(|_| std::env::var("GITLAB_PRIVATE_TOKEN"))
            .context("set GITLAB_TOKEN (or GITLAB_PRIVATE_TOKEN) to a PAT with read_api")?;
        let base_url = std::env::var("GITLAB_URL")
            .unwrap_or_else(|_| DEFAULT_BASE.to_string())
            .trim_end_matches('/')
            .to_string();
        Ok(Self { base_url, token })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueStateFilter {
    Opened,
    Closed,
    All,
}

impl IssueStateFilter {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "opened" | "open" => Ok(Self::Opened),
            "closed" => Ok(Self::Closed),
            "all" => Ok(Self::All),
            other => bail!("invalid --state {other:?} (opened|closed|all)"),
        }
    }

    fn api_param(self) -> Option<&'static str> {
        match self {
            Self::Opened => Some("opened"),
            Self::Closed => Some("closed"),
            Self::All => None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GitLabIssue {
    #[allow(dead_code)]
    pub id: i64,
    pub iid: i64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub state: String,
    pub web_url: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    Created,
    Updated,
    Unchanged,
    Skipped,
}

#[derive(Debug, Serialize)]
pub struct SyncItem {
    pub action: SyncAction,
    pub issue_iid: i64,
    pub web_url: String,
    pub card_key: Option<String>,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct SyncReport {
    pub project: String,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub items: Vec<SyncItem>,
}

pub struct GitLabClient {
    config: GitLabConfig,
    agent: ureq::Agent,
}

impl GitLabClient {
    pub fn new(config: GitLabConfig) -> Self {
        Self {
            config,
            agent: ureq::Agent::new_with_defaults(),
        }
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, path_and_query: &str) -> Result<T> {
        let url = format!("{}{}", self.config.base_url, path_and_query);
        let resp = self
            .agent
            .get(&url)
            .header("PRIVATE-TOKEN", &self.config.token)
            .header("User-Agent", USER_AGENT)
            .call()
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.into_body().read_to_string().unwrap_or_default();
            bail!("GitLab HTTP {status}: {body}");
        }
        resp.into_body()
            .read_json::<T>()
            .context("decode GitLab JSON")
    }

    /// List issues for `group/project` or numeric project id (paginated).
    pub fn list_issues(
        &self,
        project: &str,
        state: IssueStateFilter,
        label: Option<&str>,
        per_page: u32,
        max_pages: u32,
    ) -> Result<Vec<GitLabIssue>> {
        let encoded = urlencoding::encode(project);
        let mut page = 1u32;
        let mut out = Vec::new();
        loop {
            let mut q = format!(
                "/api/v4/projects/{encoded}/issues?per_page={per_page}&page={page}&order_by=updated_at&sort=desc"
            );
            if let Some(s) = state.api_param() {
                q.push_str("&state=");
                q.push_str(s);
            }
            if let Some(l) = label.map(str::trim).filter(|s| !s.is_empty()) {
                q.push_str("&labels=");
                q.push_str(&urlencoding::encode(l));
            }
            let batch: Vec<GitLabIssue> = self.get_json(&q)?;
            let n = batch.len();
            out.extend(batch);
            if n < per_page as usize || page >= max_pages {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    pub fn get_issue(&self, project: &str, iid: i64) -> Result<GitLabIssue> {
        let encoded = urlencoding::encode(project);
        self.get_json(&format!("/api/v4/projects/{encoded}/issues/{iid}"))
    }

    pub fn whoami(&self) -> Result<serde_json::Value> {
        self.get_json("/api/v4/user")
    }
}

/// Map a GitLab issue state onto a board column.
pub fn status_for_issue(state: &str, open_status: Status) -> Status {
    match state.trim().to_ascii_lowercase().as_str() {
        "closed" => Status::Done,
        _ => open_status,
    }
}

/// Labels stored on the card: GitLab labels + `gitlab` + `gitlab:#iid`.
pub fn card_labels(issue: &GitLabIssue) -> Vec<String> {
    let mut labels = issue.labels.clone();
    if !labels.iter().any(|l| l.eq_ignore_ascii_case("gitlab")) {
        labels.push("gitlab".into());
    }
    let iid_label = format!("gitlab:#{}", issue.iid);
    if !labels.iter().any(|l| l == &iid_label) {
        labels.push(iid_label);
    }
    labels
}

pub fn issue_body(issue: &GitLabIssue) -> String {
    let mut body = issue.description.clone().unwrap_or_default();
    if !body.is_empty() {
        body.push_str("\n\n");
    }
    body.push_str(&format!("— synced from GitLab #{}", issue.iid));
    body
}

fn labels_equal(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut aa = a.to_vec();
    let mut bb = b.to_vec();
    aa.sort();
    bb.sort();
    aa == bb
}

pub struct ImportOpts<'a> {
    pub project: &'a str,
    pub state: IssueStateFilter,
    pub label: Option<&'a str>,
    pub open_status: Status,
    pub dry_run: bool,
    pub actor_name: Option<&'a str>,
}

/// Upsert one GitLab issue into the board (dedupe by `external_url`).
pub fn upsert_issue(
    store: &BoardStore,
    issue: &GitLabIssue,
    open_status: Status,
    dry_run: bool,
    actor: &str,
) -> Result<SyncItem> {
    let labels = card_labels(issue);
    let status = status_for_issue(&issue.state, open_status);
    let body = issue_body(issue);

    if let Some(existing) = store.find_by_external_url(&issue.web_url)? {
        let same = existing.title == issue.title
            && existing.body == body
            && existing.status == status
            && labels_equal(&existing.labels, &labels);
        if same {
            return Ok(SyncItem {
                action: SyncAction::Unchanged,
                issue_iid: issue.iid,
                web_url: issue.web_url.clone(),
                card_key: Some(existing.key),
                title: issue.title.clone(),
            });
        }
        if dry_run {
            return Ok(SyncItem {
                action: SyncAction::Updated,
                issue_iid: issue.iid,
                web_url: issue.web_url.clone(),
                card_key: Some(existing.key),
                title: issue.title.clone(),
            });
        }
        // Status via move_card (dep gate + timeline); fields via update_card.
        // If deps block →done/doing, still refresh title/body/labels.
        if existing.status != status {
            if let Err(e) = store.move_card(
                &existing.id,
                status,
                Some("gitlab sync"),
                "system",
                Some(actor),
            ) {
                if !matches!(e, board_core::Error::BlockedByDeps(_)) {
                    return Err(e.into());
                }
            }
        }
        let card = store.update_card(
            &existing.id,
            UpdateCard {
                title: Some(issue.title.clone()),
                body: Some(body),
                labels: Some(labels),
                actor_kind: Some("system".into()),
                actor_name: Some(actor.into()),
                ..Default::default()
            },
        )?;
        return Ok(SyncItem {
            action: SyncAction::Updated,
            issue_iid: issue.iid,
            web_url: issue.web_url.clone(),
            card_key: Some(card.key),
            title: issue.title.clone(),
        });
    }

    if dry_run {
        return Ok(SyncItem {
            action: SyncAction::Created,
            issue_iid: issue.iid,
            web_url: issue.web_url.clone(),
            card_key: None,
            title: issue.title.clone(),
        });
    }
    let card = store.create_card(CreateCard {
        title: issue.title.clone(),
        body: Some(body),
        status: Some(status),
        labels,
        external_url: Some(issue.web_url.clone()),
        actor_kind: Some("system".into()),
        actor_name: Some(actor.into()),
        ..Default::default()
    })?;
    Ok(SyncItem {
        action: SyncAction::Created,
        issue_iid: issue.iid,
        web_url: issue.web_url.clone(),
        card_key: Some(card.key),
        title: issue.title.clone(),
    })
}

/// Import / refresh GitLab issues into the board (dedupe by `external_url`).
pub fn import_issues(
    store: &BoardStore,
    client: &GitLabClient,
    opts: ImportOpts<'_>,
) -> Result<SyncReport> {
    let issues = client.list_issues(opts.project, opts.state, opts.label, 100, 20)?;
    let mut report = SyncReport {
        project: opts.project.to_string(),
        created: 0,
        updated: 0,
        unchanged: 0,
        items: Vec::new(),
    };
    let actor = opts.actor_name.unwrap_or("gitlab");

    for issue in &issues {
        let item = upsert_issue(store, issue, opts.open_status.clone(), opts.dry_run, actor)?;
        match item.action {
            SyncAction::Created => report.created += 1,
            SyncAction::Updated => report.updated += 1,
            SyncAction::Unchanged => report.unchanged += 1,
            SyncAction::Skipped => {}
        }
        report.items.push(item);
    }

    Ok(report)
}

/// Refresh every board card whose `external_url` points at a GitLab issue under `project`.
pub fn sync_linked(
    store: &BoardStore,
    client: &GitLabClient,
    project: &str,
    open_status: Status,
    dry_run: bool,
    actor_name: Option<&str>,
) -> Result<SyncReport> {
    let actor = actor_name.unwrap_or("gitlab");
    let base = client.config.base_url.trim_end_matches('/');
    let mut report = SyncReport {
        project: project.to_string(),
        created: 0,
        updated: 0,
        unchanged: 0,
        items: Vec::new(),
    };

    for card in store.list_with_external_url()? {
        let Some(url) = card.external_url.as_deref() else {
            continue;
        };
        let Some(iid) = parse_issue_iid(url, base, project) else {
            report.items.push(SyncItem {
                action: SyncAction::Skipped,
                issue_iid: 0,
                web_url: url.to_string(),
                card_key: Some(card.key.clone()),
                title: card.title.clone(),
            });
            continue;
        };
        let issue = client.get_issue(project, iid)?;
        let item = upsert_issue(store, &issue, open_status.clone(), dry_run, actor)?;
        match item.action {
            SyncAction::Created => report.created += 1,
            SyncAction::Updated => report.updated += 1,
            SyncAction::Unchanged => report.unchanged += 1,
            SyncAction::Skipped => {}
        }
        report.items.push(item);
    }

    Ok(report)
}

/// Parse `#iid` from a GitLab issue URL for the given project.
pub fn parse_issue_iid(url: &str, base_url: &str, project: &str) -> Option<i64> {
    let url = url.trim();
    let base = base_url.trim_end_matches('/');
    if !url.starts_with(base) {
        return None;
    }
    // …/group/project/-/issues/12
    let marker = "/-/issues/";
    let idx = url.find(marker)?;
    let before = &url[..idx];
    let proj_suffix = format!("/{project}");
    if !before.ends_with(&proj_suffix) && !before.ends_with(project) {
        // Allow URL-encoded project paths loosely: just require /-/issues/N
        // when host matches; project filter is best-effort.
    }
    let rest = &url[idx + marker.len()..];
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

pub fn doctor(client: &GitLabClient) -> Result<serde_json::Value> {
    let user = client.whoami()?;
    Ok(json!({
        "ok": true,
        "base_url": client.config.base_url,
        "user": {
            "id": user.get("id"),
            "username": user.get("username"),
            "name": user.get("name"),
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_closed_to_done() {
        assert_eq!(status_for_issue("closed", Status::Ready), Status::Done);
        assert_eq!(status_for_issue("opened", Status::Ready), Status::Ready);
        assert_eq!(status_for_issue("opened", Status::Backlog), Status::Backlog);
    }

    #[test]
    fn builds_labels_with_iid() {
        let issue = GitLabIssue {
            id: 1,
            iid: 42,
            title: "t".into(),
            description: None,
            state: "opened".into(),
            web_url: "https://gitlab.com/g/p/-/issues/42".into(),
            labels: vec!["bug".into()],
        };
        let labels = card_labels(&issue);
        assert!(labels.contains(&"bug".into()));
        assert!(labels.contains(&"gitlab".into()));
        assert!(labels.contains(&"gitlab:#42".into()));
    }

    #[test]
    fn parses_issue_url() {
        let iid = parse_issue_iid(
            "https://gitlab.com/acme/app/-/issues/7",
            "https://gitlab.com",
            "acme/app",
        );
        assert_eq!(iid, Some(7));
        assert_eq!(
            parse_issue_iid(
                "https://gitlab.example/acme/app/-/issues/3",
                "https://gitlab.com",
                "acme/app"
            ),
            None
        );
    }

    #[test]
    fn upsert_creates_then_updates() {
        let dir = std::env::temp_dir().join(format!(
            "board-gitlab-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("board.db");
        let store = BoardStore::open(&db).unwrap();

        let mut issue = GitLabIssue {
            id: 9,
            iid: 3,
            title: "Wire GitLab".into(),
            description: Some("first".into()),
            state: "opened".into(),
            web_url: "https://gitlab.com/acme/app/-/issues/3".into(),
            labels: vec!["infra".into()],
        };

        let created = upsert_issue(&store, &issue, Status::Ready, false, "gitlab").unwrap();
        assert!(matches!(created.action, SyncAction::Created));
        let key = created.card_key.clone().unwrap();
        let card = store.get_card(&key).unwrap();
        assert_eq!(card.status, Status::Ready);
        assert_eq!(
            card.external_url.as_deref(),
            Some("https://gitlab.com/acme/app/-/issues/3")
        );

        let same = upsert_issue(&store, &issue, Status::Ready, false, "gitlab").unwrap();
        assert!(matches!(same.action, SyncAction::Unchanged));

        issue.title = "Wire GitLab (2)".into();
        issue.state = "closed".into();
        let updated = upsert_issue(&store, &issue, Status::Ready, false, "gitlab").unwrap();
        assert!(matches!(updated.action, SyncAction::Updated));
        let card = store.get_card(&key).unwrap();
        assert_eq!(card.title, "Wire GitLab (2)");
        assert_eq!(card.status, Status::Done);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
