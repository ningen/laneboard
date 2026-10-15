use board_core::{
    AssigneeKind, BoardSnapshot, BoardStore, Card, CardDetail, CreateCard, Event, ListFilter,
    Status, UpdateCard,
};
use serde::Deserialize;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    store: Mutex<BoardStore>,
}

fn with_store<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&BoardStore) -> board_core::Result<T>,
) -> Result<T, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    f(&store).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_board(state: State<'_, AppState>) -> Result<BoardSnapshot, String> {
    with_store(&state, |s| s.board_snapshot())
}

#[tauri::command]
fn list_cards(state: State<'_, AppState>, query: Option<String>) -> Result<Vec<Card>, String> {
    with_store(&state, |s| {
        s.list_cards(&ListFilter {
            query,
            ..Default::default()
        })
    })
}

#[tauri::command]
fn get_card(state: State<'_, AppState>, id: String) -> Result<CardDetail, String> {
    with_store(&state, |s| s.get_card_detail(&id, 30))
}

#[derive(Deserialize)]
struct CreateInput {
    title: String,
    body: Option<String>,
    status: Option<String>,
    labels: Option<Vec<String>>,
}

#[tauri::command]
fn create_card(state: State<'_, AppState>, input: CreateInput) -> Result<Card, String> {
    with_store(&state, |s| {
        s.create_card(CreateCard {
            title: input.title,
            body: input.body,
            status: input
                .status
                .as_deref()
                .map(Status::parse)
                .transpose()
                .map_err(|e| board_core::Error::msg(e.to_string()))?,
            labels: input.labels.unwrap_or_default(),
            assignee_kind: Some(AssigneeKind::Human),
            actor_kind: Some("human".into()),
            actor_name: Some("desktop".into()),
            ..Default::default()
        })
    })
}

#[derive(Deserialize)]
struct MoveInput {
    id: String,
    to: String,
    note: Option<String>,
}

#[tauri::command]
fn move_card(state: State<'_, AppState>, input: MoveInput) -> Result<Card, String> {
    with_store(&state, |s| {
        s.move_card(
            &input.id,
            Status::parse(&input.to).map_err(|e| board_core::Error::msg(e.to_string()))?,
            input.note.as_deref(),
            "human",
            Some("desktop"),
        )
    })
}

#[derive(Deserialize)]
struct UpdateInput {
    id: String,
    title: Option<String>,
    body: Option<String>,
}

#[tauri::command]
fn update_card(state: State<'_, AppState>, input: UpdateInput) -> Result<Card, String> {
    with_store(&state, |s| {
        s.update_card(
            &input.id,
            UpdateCard {
                title: input.title,
                body: input.body,
                actor_kind: Some("human".into()),
                actor_name: Some("desktop".into()),
                ..Default::default()
            },
        )
    })
}

#[tauri::command]
fn seed_demo(state: State<'_, AppState>) -> Result<Vec<Card>, String> {
    with_store(&state, |s| s.seed_demo())
}

#[tauri::command]
fn timeline(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<Event>, String> {
    with_store(&state, |s| s.list_events(None, limit.unwrap_or(40)))
}

#[tauri::command]
fn db_path() -> String {
    BoardStore::path_hint().display().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let store = BoardStore::open_default().expect("open board database");
    let _ = store.seed_demo();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            store: Mutex::new(store),
        })
        .invoke_handler(tauri::generate_handler![
            get_board,
            list_cards,
            get_card,
            create_card,
            move_card,
            update_card,
            seed_demo,
            timeline,
            db_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running board desktop");
}