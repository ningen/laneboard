use board_core::{BoardStore, CreateCard, Status};

#[test]
fn find_and_list_by_external_url() {
    let dir = std::env::temp_dir().join(format!(
        "board-ext-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = BoardStore::open(dir.join("board.db")).unwrap();

    let url = "https://gitlab.com/acme/app/-/issues/9";
    let card = store
        .create_card(CreateCard {
            title: "From GitLab".into(),
            status: Some(Status::Ready),
            external_url: Some(url.into()),
            ..Default::default()
        })
        .unwrap();

    let found = store.find_by_external_url(url).unwrap().unwrap();
    assert_eq!(found.id, card.id);

    assert!(store.find_by_external_url("https://example/missing").unwrap().is_none());

    let linked = store.list_with_external_url().unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].key, card.key);

    let _ = std::fs::remove_dir_all(&dir);
}
