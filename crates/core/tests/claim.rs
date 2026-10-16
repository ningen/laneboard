use board_core::{
    AssigneeKind, BoardStore, ClaimCard, CreateCard, Error, LinkType, ListFilter, Status,
};
use std::path::PathBuf;

fn tmp_db() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("board-claim-{}.db", uuid::Uuid::new_v4()));
    p
}

fn ready_card(store: &BoardStore, title: &str) -> board_core::Card {
    store
        .create_card(CreateCard {
            title: title.into(),
            status: Some(Status::Ready),
            ..Default::default()
        })
        .unwrap()
}

#[test]
fn claim_release_note_and_conflict() {
    let path = tmp_db();
    let store = BoardStore::open(&path).unwrap();
    let card = ready_card(&store, "Wire MCP");

    let claimed = store
        .claim_card(
            &card.key,
            ClaimCard {
                ttl_minutes: 30,
                note: Some("starting".into()),
                actor_kind: "agent".into(),
                actor_name: Some("claude".into()),
            },
        )
        .unwrap();
    assert_eq!(claimed.status, Status::Doing);
    assert_eq!(claimed.assignee_kind, AssigneeKind::Agent);
    assert_eq!(claimed.assignee_name.as_deref(), Some("claude"));
    assert!(BoardStore::lease_of(&claimed).is_some());

    let detail = store.get_card_detail(&card.key, 10).unwrap();
    assert!(detail.lease.is_some());
    assert!(detail.recent_events.iter().any(|e| e.kind == "claimed"));

    // Second actor cannot steal live lease (even though status is doing)
    let err = store
        .claim_card(
            &card.key,
            ClaimCard {
                ttl_minutes: 30,
                note: None,
                actor_kind: "agent".into(),
                actor_name: Some("cursor".into()),
            },
        )
        .unwrap_err();
    assert_eq!(err.code(), "already_claimed");
    assert!(matches!(err, Error::AlreadyClaimed(_)));

    store
        .add_note(&card.key, "halfway", "agent", Some("claude"))
        .unwrap();
    let events = store.list_events(Some(&card.key), 20).unwrap();
    assert!(events.iter().any(|e| e.kind == "note"));

    let released = store
        .release_card(&card.key, Status::Ready, Some("pause"), "agent", Some("claude"))
        .unwrap();
    assert_eq!(released.status, Status::Ready);
    assert!(BoardStore::lease_of(&released).is_none());

    let claimable = store.list_claimable(10).unwrap();
    assert!(claimable.iter().any(|c| c.key == card.key));

    let _ = std::fs::remove_file(path);
}

#[test]
fn blocked_by_prevents_done() {
    let path = tmp_db();
    let store = BoardStore::open(&path).unwrap();
    let blocker = store
        .create_card(CreateCard {
            title: "Auth first".into(),
            status: Some(Status::Doing),
            ..Default::default()
        })
        .unwrap();
    let dependent = store
        .create_card(CreateCard {
            title: "UI next".into(),
            status: Some(Status::Ready),
            ..Default::default()
        })
        .unwrap();
    store
        .link_cards(
            &blocker.key,
            &dependent.key,
            LinkType::Blocks,
            None,
            "human",
            Some("me"),
        )
        .unwrap();

    let err = store
        .move_card(&dependent.key, Status::Done, None, "agent", Some("claude"))
        .unwrap_err();
    assert_eq!(err.code(), "blocked_by_deps");

    store
        .move_card(&blocker.key, Status::Done, None, "agent", Some("claude"))
        .unwrap();
    store
        .move_card(&dependent.key, Status::Done, None, "agent", Some("claude"))
        .unwrap();

    let done = store
        .list_cards(&ListFilter {
            status: Some(Status::Done),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(done.len(), 2);

    let _ = std::fs::remove_file(path);
}

#[test]
fn not_claimable_when_not_ready() {
    let path = tmp_db();
    let store = BoardStore::open(&path).unwrap();
    let card = store
        .create_card(CreateCard {
            title: "Later".into(),
            status: Some(Status::Backlog),
            ..Default::default()
        })
        .unwrap();
    let err = store
        .claim_card(
            &card.key,
            ClaimCard {
                ttl_minutes: 60,
                note: None,
                actor_kind: "agent".into(),
                actor_name: Some("claude".into()),
            },
        )
        .unwrap_err();
    assert_eq!(err.code(), "not_claimable");
    let _ = std::fs::remove_file(path);
}
