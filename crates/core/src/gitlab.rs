//! GitLab Issues → board cards (one-way pull, no webhooks).
//!
//! Auth: settings token or `GITLAB_TOKEN` / `GITLAB_PRIVATE_TOKEN`.
//! Host: settings `base_url` or `GITLAB_URL` (default `https://gitlab.com`).
//! See `docs/gitlab.md`.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{CreateCard, Status, UpdateCard};
use crate::store::BoardStore;
use crate::{Error, Result};

const DEFAULT_BASE: &str = "https://gitlab.com";
const USER_AGENT: &str = "laneboard-gitlab/0.1";
pub const SETTINGS_META_KEY: &str = "gitlab_settings";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabSettings {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub project: String,
    /// Board status for open issues (default ready).
    #[serde(default = "default_open_status")]
    pub open_status: String,
    /// Optional GitLab label filter.
    #[serde(default)]
    pub label: String,
    /// opened | closed | all
    #[serde(default = "default_state")]
    pub state: String,
}

fn default_base_url() -> String {
    DEFAULT_BASE.to_string()
}
fn default_open_status() -> String {
    "ready".into()
}
fn default_state() -> String {
    "opened".into()
}

impl Default for GitLabSettings {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            token: String::new(),
            project: String::new(),
            open_status: default_open_status(),
            label: String::new(),
            state: default_state(),
        }
    }
}

/// Public view — never echoes the raw token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabSettingsView {
    pub base_url: String,
    pub token_set: bool,
    pub project: String,
    pub open_status: String,
    pub label: String,
    pub state: String,
}

impl GitLabSettings {
    pub fn view(&self) -> GitLabSettingsView {
        GitLabSettingsView {
            base_url: self.base_url.clone(),
            token_set: !self.token.trim().is_empty(),
            project: self.project.clone(),
            open_status: self.open_status.clone(),
            label: self.label.clone(),
            state: self.state.clone(),
        }
    }

    /// Merge a UI save payload. Empty `token` keeps the previous token.
    pub fn apply_update(&mut self, update: GitLabSettingsUpdate) {
        if let Some(v) = update.base_url {
            self.base_url = v.trim().trim_end_matches('/').to_string();
            if self.base_url.is_empty() {
                self.base_url = default_base_url();
            }
        }
        if let Some(v) = update.token {
            let t = v.trim().to_string();
            if !t.is_empty() {
                self.token = t;
            }
        }
        if let Some(v) = update.project {
            self.project = v.trim().to_string();
        }
        if let Some(v) = update.open_status {
            self.open_status = v.trim().to_string();
        }
        if let Some(v) = update.label {
            self.label = v.trim().to_string();
        }
        if let Some(v) = update.state {
            self.state = v.trim().to_string();
        }
    }

    pub fn to_client_config(&self) -> Result<GitLabConfig> {
        let mut token = self.token.trim().to_string();
        if token.is_empty() {
            token = std::env::var("GITLAB_TOKEN")
                .or_else(|_| std::env::var("GITLAB_PRIVATE_TOKEN"))
                .unwrap_or_default();
        }
        if token.is_empty() {
            return Err(Error::Validation(
                "GitLab token not set (Settings or GITLAB_TOKEN)".into(),
            ));
        }
        let mut base_url = self.base_url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            base_url = std::env::var("GITLAB_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
            base_url = base_url.trim_end_matches('/').to_string();
        }
        Ok(GitLabConfig { base_url, token })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GitLabSettingsUpdate {
    pub base_url: Option<String>,
    pub token: Option<String>,
    pub project: Option<String>,
    pub open_status: Option<String>,
    pub label: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitLabConfig {
    pub base_url: String,
    pub token: String,
}

impl GitLabConfig {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("GITLAB_TOKEN")
            .or_else(|_| std::env::var("GITLAB_PRIVATE_TOKEN"))
            .map_err(|_| {
                Error::Validation(
                    "set GITLAB_TOKEN (or GITLAB_PRIVATE_TOKEN) to a PAT with read_api".into(),
                )
            })?;
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
            other => Err(Error::Validation(format!(
                "invalid state {other:?} (opened|closed|all)"
            ))),
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
    pub config: GitLabConfig,
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
            .map_err(|e| Error::msg(format!("GET {url}: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.into_body().read_to_string().unwrap_or_default();
            return Err(Error::msg(format!("GitLab HTTP {status}: {body}")));
        }
        resp.into_body()
            .read_json::<T>()
            .map_err(|e| Error::msg(format!("decode GitLab JSON: {e}")))
    }

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

pub fn status_for_issue(state: &str, open_status: Status) -> Status {
    match state.trim().to_ascii_lowercase().as_str() {
        "closed" => Status::Done,
        _ => open_status,
    }
}

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
        if existing.status != status {
            if let Err(e) = store.move_card(
                &existing.id,
                status,
                Some("gitlab sync"),
                "system",
                Some(actor),
            ) {
                if !matches!(e, Error::BlockedByDeps(_)) {
                    return Err(e);
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

pub fn parse_issue_iid(url: &str, base_url: &str, _project: &str) -> Option<i64> {
    let url = url.trim();
    let base = base_url.trim_end_matches('/');
    if !url.starts_with(base) {
        return None;
    }
    let marker = "/-/issues/";
    let idx = url.find(marker)?;
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

/// Run import using persisted settings (project required).
pub fn import_from_settings(store: &BoardStore, dry_run: bool) -> Result<SyncReport> {
    let settings = store.gitlab_settings()?;
    if settings.project.trim().is_empty() {
        return Err(Error::Validation("GitLab project not set".into()));
    }
    let client = GitLabClient::new(settings.to_client_config()?);
    let open_status = Status::parse(&settings.open_status)?;
    let state = IssueStateFilter::parse(&settings.state)?;
    let label = settings.label.trim();
    import_issues(
        store,
        &client,
        ImportOpts {
            project: &settings.project,
            state,
            label: if label.is_empty() { None } else { Some(label) },
            open_status,
            dry_run,
            actor_name: Some("gitlab"),
        },
    )
}

pub fn sync_from_settings(store: &BoardStore, dry_run: bool) -> Result<SyncReport> {
    let settings = store.gitlab_settings()?;
    if settings.project.trim().is_empty() {
        return Err(Error::Validation("GitLab project not set".into()));
    }
    let client = GitLabClient::new(settings.to_client_config()?);
    let open_status = Status::parse(&settings.open_status)?;
    sync_linked(
        store,
        &client,
        &settings.project,
        open_status,
        dry_run,
        Some("gitlab"),
    )
}

pub fn doctor_from_settings(store: &BoardStore) -> Result<serde_json::Value> {
    let settings = store.gitlab_settings()?;
    let client = GitLabClient::new(settings.to_client_config()?);
    doctor(&client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_closed_to_done() {
        assert_eq!(status_for_issue("closed", Status::Ready), Status::Done);
        assert_eq!(status_for_issue("opened", Status::Ready), Status::Ready);
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
        assert_eq!(
            parse_issue_iid(
                "https://gitlab.com/acme/app/-/issues/7",
                "https://gitlab.com",
                "acme/app"
            ),
            Some(7)
        );
    }

    #[test]
    fn settings_apply_keeps_token() {
        let mut s = GitLabSettings {
            token: "secret".into(),
            project: "a/b".into(),
            ..Default::default()
        };
        s.apply_update(GitLabSettingsUpdate {
            token: Some("".into()),
            project: Some("x/y".into()),
            ..Default::default()
        });
        assert_eq!(s.token, "secret");
        assert_eq!(s.project, "x/y");
        assert!(s.view().token_set);
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
        let store = BoardStore::open(dir.join("board.db")).unwrap();

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
