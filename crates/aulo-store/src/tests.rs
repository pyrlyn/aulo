use diesel_migrations::MigrationHarness;
use tempfile::TempDir;

use super::*;

fn open() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("aulo.db");
    let store = Store::open(path.to_str().unwrap()).unwrap();
    (dir, store)
}

#[test]
fn migrations_apply_revert_and_reapply() {
    let (_dir, mut s) = open();
    assert_eq!(
        chats::table.count().get_result::<i64>(&mut s.conn).unwrap(),
        0
    );

    let reverted = s.conn.revert_all_migrations(MIGRATIONS).unwrap();
    assert_eq!(reverted.len(), 1);
    // The DSL cannot inspect sqlite_master; a failing query proves the table is gone.
    assert!(chats::table.count().get_result::<i64>(&mut s.conn).is_err());
    assert!(usage::table.count().get_result::<i64>(&mut s.conn).is_err());

    s.conn.run_pending_migrations(MIGRATIONS).unwrap();
    assert_eq!(
        bots::table.count().get_result::<i64>(&mut s.conn).unwrap(),
        0
    );
}

#[test]
fn reopening_keeps_data_and_does_not_remigrate() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("aulo.db");
    let path = path.to_str().unwrap();
    let id = Store::open(path)
        .unwrap()
        .create_chat(None, "t", "m")
        .unwrap()
        .id;
    assert!(Store::open(path).unwrap().get_chat(&id).unwrap().is_some());
}

#[test]
fn chat_crud() {
    let (_dir, mut s) = open();
    let bot = s.create_bot("helper").unwrap();
    let chat = s.create_chat(Some(&bot.id), "first", "model-a").unwrap();
    assert_eq!(s.get_chat(&chat.id).unwrap(), Some(chat.clone()));
    assert_eq!(s.get_chat("missing").unwrap(), None);

    assert!(s.rename_chat(&chat.id, "renamed").unwrap());
    assert!(!s.rename_chat("missing", "x").unwrap());
    assert_eq!(s.get_chat(&chat.id).unwrap().unwrap().title, "renamed");

    let other = s.create_chat(None, "second", "model-b").unwrap();
    s.append_message(&chat.id, "user", "bump").unwrap();
    let ids: Vec<_> = s
        .list_chats(10, 0)
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&other.id));
    assert_eq!(s.list_chats(1, 1).unwrap().len(), 1);

    assert!(s.delete_chat(&chat.id).unwrap());
    assert!(!s.delete_chat(&chat.id).unwrap());
    assert_eq!(s.get_chat(&chat.id).unwrap(), None);
}

#[test]
fn messages_paginate_in_insertion_order() {
    let (_dir, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    let want: Vec<_> = (0..5)
        .map(|i| {
            s.append_message(&chat.id, "user", &format!("m{i}"))
                .unwrap()
                .id
        })
        .collect();

    let page1 = s.list_messages(&chat.id, None, 2).unwrap();
    let page2 = s.list_messages(&chat.id, Some(&page1[1].id), 2).unwrap();
    let page3 = s.list_messages(&chat.id, Some(&page2[1].id), 2).unwrap();
    let got: Vec<_> = page1
        .iter()
        .chain(&page2)
        .chain(&page3)
        .map(|m| m.id.clone())
        .collect();
    assert_eq!(got, want);
    assert_eq!(page3.len(), 1);
    assert!(
        s.list_messages(&chat.id, Some(&want[4]), 2)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn append_message_rejects_unknown_chat() {
    let (_dir, mut s) = open();
    // Fails only if foreign_keys is on for this connection.
    assert!(s.append_message("missing", "user", "x").is_err());
}

#[test]
fn deleting_a_chat_cascades() {
    let (_dir, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    let msg = s.append_message(&chat.id, "assistant", "hi").unwrap();
    s.record_tool_call(&msg.id, "search", "{}", Some("ok"), "ok")
        .unwrap();
    s.record_usage(&chat.id, "p", "m", 10, 20).unwrap();
    assert_eq!(s.list_tool_calls(&msg.id).unwrap().len(), 1);
    assert_eq!(s.list_usage(&chat.id).unwrap()[0].output_tokens, 20);

    s.delete_chat(&chat.id).unwrap();
    for n in [
        messages::table
            .count()
            .get_result::<i64>(&mut s.conn)
            .unwrap(),
        tool_calls::table
            .count()
            .get_result::<i64>(&mut s.conn)
            .unwrap(),
        usage::table.count().get_result::<i64>(&mut s.conn).unwrap(),
    ] {
        assert_eq!(n, 0);
    }
}

#[test]
fn deleting_a_bot_keeps_its_chats() {
    let (_dir, mut s) = open();
    let bot = s.create_bot("b").unwrap();
    let chat = s.create_chat(Some(&bot.id), "t", "m").unwrap();
    diesel::delete(bots::table.find(&bot.id))
        .execute(&mut s.conn)
        .unwrap();
    assert_eq!(s.get_chat(&chat.id).unwrap().unwrap().bot_id, None);
    assert!(s.list_bots().unwrap().is_empty());
}

#[test]
fn database_runs_in_wal_mode() {
    let (dir, _store) = open();
    // The `-wal` sidecar only exists while a WAL-mode connection is open.
    assert!(dir.path().join("aulo.db-wal").exists());
}
