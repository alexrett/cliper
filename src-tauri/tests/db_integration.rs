use cliper_lib::crypto::KeyManager;
use cliper_lib::db::{Database, NewItem};
use std::path::PathBuf;

#[test]
fn db_migration_and_insert() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().to_path_buf();
    let db = Database::new(app_dir).unwrap();
    db.migrate().unwrap();

    let km = KeyManager::new("test.bundle".into());
    km.unlock().unwrap();

    let text = b"hello db";
    let enc = km.encrypt(text).unwrap();
    let sha = Database::compute_sha256(text);
    let id = db
        .insert_item(NewItem {
            kind: "text".into(),
            size: text.len() as i64,
            sha256: sha,
            file_path: None,
            content_blob: Some(enc),
            preview_blob: None,
            rtf_blob: None,
        })
        .unwrap();
    assert!(id > 0);

    let list = db.list_recent(10).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, "text");
}

