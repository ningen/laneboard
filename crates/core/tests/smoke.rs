use board_core::{
    AssigneeKind, BoardStore, CreateCard, LinkType, ListFilter, Status,
};
use std::path::PathBuf;

fn tmp_db() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("board-test-{}.db", uuid::Uuid::new_v4()));
    p
}

#[test]
fn create_link_search_timeline() {
    let path = tmp_db();
    let store = BoardStore::open(&path).unwrap();

    let a = store
        .create_card(CreateCard {
            title: "Auth middleware".into(),
            body: Some("wire JWT".into()),
            status: Some(Status::Doing),
            labels: vec!["api".into()],
            assignee_kind: Some(AssigneeKind::Agent),
            assignee_name: Some("bot".into()),
            ..Default::default()
        })
        .unwrap();

    let b = store
        .create_card(CreateCard {
            title: "Docs for agents".into(),
            status: Some(Status::Backlog),
            labels: vec!["docs".into()],
            ..Default::default()
        })
        .unwrap();

    store
        .link_cards(
            &a.key,
            &b.key,
            LinkType::Blocks,
            Some("docs after auth"),
            "agent",
            Some("bot"),
        )
        .unwrap();

    let hits = store
        .search(
            "Auth",
            &ListFilter {
                status: Some(Status::Doing),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].key, a.key);

    let detail = store.get_card_detail(&a.key, 10).unwrap();
    assert!(!detail.links.is_empty());
    assert!(!detail.recent_events.is_empty());

    store
        .move_card(&a.key, Status::Done, Some("shipped"), "human", Some("me"))
        .unwrap();
    let events = store.list_events(Some(&a.key), 20).unwrap();
    assert!(events.iter().any(|e| e.kind == "moved"));

    let before = store.latest_event_id().unwrap();
    assert!(before > 0);
    let activity = store.list_activity(10).unwrap();
    assert!(!activity.is_empty());
    let moved = activity.iter().find(|e| e.kind == "moved").unwrap();
    assert_eq!(moved.card_key.as_deref(), Some(a.key.as_str()));
    assert_eq!(moved.card_title.as_deref(), Some("Auth middleware"));
    let since = store.list_activity_since(before, 10).unwrap();
    assert!(since.is_empty());
    store
        .move_card(&b.key, Status::Ready, None, "agent", Some("bot"))
        .unwrap();
    let after = store.list_activity_since(before, 10).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].card_key.as_deref(), Some(b.key.as_str()));

    let _ = std::fs::remove_file(path);
}