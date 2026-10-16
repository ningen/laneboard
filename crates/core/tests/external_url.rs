use board_core::{BoardStore, CreateCard, GitLabSettingsUpdate, Status};

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

#[test]
fn gitlab_settings_roundtrip_masks_token() {
    let dir = std::env::temp_dir().join(format!(
        "board-gl-settings-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = BoardStore::open(dir.join("board.db")).unwrap();

    let view = store
        .save_gitlab_settings(GitLabSettingsUpdate {
            base_url: Some("https://gitlab.example".into()),
            token: Some("glpat-secret".into()),
            project: Some("acme/app".into()),
            open_status: Some("ready".into()),
            label: Some("board".into()),
            state: Some("opened".into()),
        })
        .unwrap();
    assert!(view.token_set);
    assert_eq!(view.project, "acme/app");
    assert_eq!(view.base_url, "https://gitlab.example");

    // Empty token on update keeps previous.
    let view2 = store
        .save_gitlab_settings(GitLabSettingsUpdate {
            project: Some("acme/app2".into()),
            token: Some("".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(view2.project, "acme/app2");
    assert!(view2.token_set);
    assert_eq!(store.gitlab_settings().unwrap().token, "glpat-secret");

    let _ = std::fs::remove_dir_all(&dir);
}
