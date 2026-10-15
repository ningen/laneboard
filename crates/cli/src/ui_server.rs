use std::net::SocketAddr;
use std::sync::Mutex;

use anyhow::{Context, Result};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use board_core::{
    AssigneeKind, BoardStore, CreateCard, Status, UpdateCard,
};
use rust_embed::Embed;
use serde::Deserialize;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    store: std::sync::Arc<Mutex<BoardStore>>,
}

#[derive(Embed)]
#[folder = "embedded-ui/"]
struct Assets;

pub async fn run(db: Option<std::path::PathBuf>, host: &str, port: u16) -> Result<()> {
    let store = match db {
        Some(p) => BoardStore::open(&p).with_context(|| format!("open db {}", p.display()))?,
        None => BoardStore::open_default().context("open default db")?,
    };
    let _ = store.seed_demo();

    let state = AppState {
        store: std::sync::Arc::new(Mutex::new(store)),
    };

    let app = Router::new()
        .route("/api/board", get(get_board))
        .route("/api/cards", get(list_cards).post(create_card))
        .route("/api/cards/{id}", get(get_card).patch(update_card))
        .route("/api/cards/{id}/move", post(move_card))
        .route("/api/seed", post(seed_demo))
        .route("/api/timeline", get(timeline))
        .route("/api/db-path", get(db_path))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("invalid listen address {host}:{port}"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;

    println!("Board UI → http://{addr}");
    println!("API       → http://{addr}/api/board");
    println!("Ctrl+C to stop");

    axum::serve(listener, app).await.context("serve ui")?;
    Ok(())
}

fn lock(state: &AppState) -> Result<std::sync::MutexGuard<'_, BoardStore>, ApiError> {
    state
        .store
        .lock()
        .map_err(|_| ApiError::msg("database lock poisoned"))
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn msg(s: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: s.into(),
        }
    }

    fn from_board(e: board_core::Error) -> Self {
        let message = e.to_string();
        let status = match &e {
            board_core::Error::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::BAD_REQUEST,
        };
        Self { status, message }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}

async fn get_board(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let snap = store.board_snapshot().map_err(ApiError::from_board)?;
    Ok(Json(snap))
}

#[derive(Deserialize)]
struct ListQuery {
    q: Option<String>,
}

async fn list_cards(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let cards = store
        .list_cards(&board_core::ListFilter {
            query: query.q,
            ..Default::default()
        })
        .map_err(ApiError::from_board)?;
    Ok(Json(cards))
}

async fn get_card(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let detail = store
        .get_card_detail(&id, 30)
        .map_err(ApiError::from_board)?;
    Ok(Json(detail))
}

#[derive(Deserialize)]
struct CreateBody {
    title: String,
    body: Option<String>,
    status: Option<String>,
    labels: Option<Vec<String>>,
}

async fn create_card(
    State(state): State<AppState>,
    Json(input): Json<CreateBody>,
) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let card = store
        .create_card(CreateCard {
            title: input.title,
            body: input.body,
            status: input
                .status
                .as_deref()
                .map(Status::parse)
                .transpose()
                .map_err(ApiError::from_board)?,
            labels: input.labels.unwrap_or_default(),
            assignee_kind: Some(AssigneeKind::Human),
            actor_kind: Some("human".into()),
            actor_name: Some("ui".into()),
            ..Default::default()
        })
        .map_err(ApiError::from_board)?;
    Ok(Json(card))
}

#[derive(Deserialize)]
struct MoveBody {
    to: String,
    note: Option<String>,
}

async fn move_card(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<MoveBody>,
) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let card = store
        .move_card(
            &id,
            Status::parse(&input.to).map_err(ApiError::from_board)?,
            input.note.as_deref(),
            "human",
            Some("ui"),
        )
        .map_err(ApiError::from_board)?;
    Ok(Json(card))
}

#[derive(Deserialize)]
struct UpdateBody {
    title: Option<String>,
    body: Option<String>,
}

async fn update_card(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateBody>,
) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let card = store
        .update_card(
            &id,
            UpdateCard {
                title: input.title,
                body: input.body,
                actor_kind: Some("human".into()),
                actor_name: Some("ui".into()),
                ..Default::default()
            },
        )
        .map_err(ApiError::from_board)?;
    Ok(Json(card))
}

async fn seed_demo(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let cards = store.seed_demo().map_err(ApiError::from_board)?;
    Ok(Json(cards))
}

#[derive(Deserialize)]
struct TimelineQuery {
    limit: Option<usize>,
}

async fn timeline(
    State(state): State<AppState>,
    Query(query): Query<TimelineQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let store = lock(&state)?;
    let events = store
        .list_events(None, query.limit.unwrap_or(40))
        .map_err(ApiError::from_board)?;
    Ok(Json(events))
}

async fn db_path(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let _store = lock(&state)?;
    Ok(Json(BoardStore::path_hint().display().to_string()))
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback
            match Assets::get("index.html") {
                Some(index) => Html(
                    std::str::from_utf8(&index.data)
                        .unwrap_or("<p>Board UI missing</p>")
                        .to_string(),
                )
                .into_response(),
                None => (
                    StatusCode::NOT_FOUND,
                    "UI assets missing. Rebuild with scripts/embed-ui.sh",
                )
                    .into_response(),
            }
        }
    }
}