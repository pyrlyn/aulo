use diesel_migrations::MigrationHarness;
use tempfile::TempDir;

use super::*;
use crate::schema::{audit, grants};

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
    assert_eq!(reverted.len(), 2);
    // The DSL cannot inspect sqlite_master; a failing query proves the table is gone.
    assert!(chats::table.count().get_result::<i64>(&mut s.conn).is_err());
    assert!(usage::table.count().get_result::<i64>(&mut s.conn).is_err());
    assert!(
        grants::table
            .count()
            .get_result::<i64>(&mut s.conn)
            .is_err()
    );
    assert!(audit::table.count().get_result::<i64>(&mut s.conn).is_err());

    s.conn.run_pending_migrations(MIGRATIONS).unwrap();
    assert_eq!(
        bots::table.count().get_result::<i64>(&mut s.conn).unwrap(),
        0
    );
    assert_eq!(
        audit::table.count().get_result::<i64>(&mut s.conn).unwrap(),
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

#[test]
fn grants_put_list_and_revoke() {
    let (_dir, mut s) = open();
    let a = s.put_grant("app:Safari", "browse", "allow", None).unwrap();
    let b = s.put_grant("app:Mail", "send", "deny", None).unwrap();
    assert_eq!(s.list_grants().unwrap(), vec![a.clone(), b.clone()]);

    assert!(s.revoke_grant(&a.id).unwrap());
    assert!(!s.revoke_grant(&a.id).unwrap());
    assert_eq!(s.list_grants().unwrap(), vec![b]);
}

#[test]
fn active_grants_filter_by_subject_and_expiry() {
    let (_dir, mut s) = open();
    let now = now_ms();
    let forever = s.put_grant("app:Safari", "browse", "allow", None).unwrap();
    let future = s
        .put_grant("app:Safari", "read", "allow", Some(now + 60_000))
        .unwrap();
    s.put_grant("app:Safari", "old", "allow", Some(now - 1))
        .unwrap();
    s.put_grant("app:Mail", "send", "allow", None).unwrap();

    assert_eq!(
        s.list_active_grants("app:Safari").unwrap(),
        vec![forever, future]
    );
    assert_eq!(s.list_grants().unwrap().len(), 4);
    assert!(s.list_active_grants("app:Unknown").unwrap().is_empty());
}

#[test]
fn audit_appends_in_seq_order_and_pages() {
    let (_dir, mut s) = open();
    assert_eq!(s.last_audit().unwrap(), None);

    let mut prev = String::from("genesis");
    let rows: Vec<_> = (0..5)
        .map(|i| {
            let hash = format!("h{i}");
            let row = s
                .append_audit(&prev, &hash, "tool_call", &format!("{{\"n\":{i}}}"))
                .unwrap();
            prev = hash;
            row
        })
        .collect();
    let seqs: Vec<_> = rows.iter().map(|r| r.seq).collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(s.last_audit().unwrap().as_ref(), rows.last());

    assert_eq!(s.list_audit_after(0, 10).unwrap(), rows);
    let page = s.list_audit_after(rows[1].seq, 2).unwrap();
    assert_eq!(page, rows[2..4]);
    assert!(s.list_audit_after(rows[4].seq, 10).unwrap().is_empty());
}

#[test]
fn audit_rejects_a_forked_chain() {
    let (_dir, mut s) = open();
    s.append_audit("genesis", "h0", "k", "{}").unwrap();
    // A second writer that read the same head must not be able to fork the chain.
    assert!(s.append_audit("genesis", "h0b", "k", "{}").is_err());
    assert_eq!(s.list_audit_after(0, 10).unwrap().len(), 1);
}

#[test]
fn chats_paginate_by_keyset_even_when_activity_moves() {
    let (_dir, mut s) = open();
    let bot = s.create_bot("b").unwrap();
    let ids: Vec<_> = (0..5)
        .map(|i| {
            let bot = (i % 2 == 0).then_some(bot.id.as_str());
            s.create_chat(bot, "t", "m").unwrap().id
        })
        .collect();
    let all: Vec<_> = s.list_chats(10, 0).unwrap();
    let cursor = |c: &Chat| (c.updated_at, c.id.clone());

    let page1 = s.list_chats_page(None, None, 2).unwrap();
    assert_eq!(page1, all[..2]);
    let (u, id) = cursor(&page1[1]);
    // A newer chat appearing mid-pagination must not shift the next page.
    s.create_chat(None, "late", "m").unwrap();
    let page2 = s.list_chats_page(None, Some((u, &id)), 2).unwrap();
    assert_eq!(page2, all[2..4]);
    let (u, id) = cursor(&page2[1]);
    let page3 = s.list_chats_page(None, Some((u, &id)), 2).unwrap();
    assert_eq!(page3, all[4..]);

    let of_bot = s.list_chats_page(Some(&bot.id), None, 10).unwrap();
    let want: Vec<_> = [&ids[0], &ids[2], &ids[4]].into_iter().collect();
    assert_eq!(of_bot.len(), 3);
    assert!(of_bot.iter().all(|c| want.contains(&&c.id)));
}

#[test]
fn tool_calls_load_for_many_messages_at_once() {
    let (_dir, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    let a = s.append_message(&chat.id, "assistant", "a").unwrap();
    let b = s.append_message(&chat.id, "assistant", "b").unwrap();
    let c = s.append_message(&chat.id, "assistant", "c").unwrap();
    s.record_tool_call(&a.id, "one", "{}", None, "running")
        .unwrap();
    s.record_tool_call(&b.id, "two", "{}", None, "running")
        .unwrap();
    s.record_tool_call(&c.id, "three", "{}", None, "running")
        .unwrap();
    let got = s
        .list_tool_calls_for(&[a.id.clone(), b.id.clone()])
        .unwrap();
    let names: Vec<_> = got.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["one", "two"]);
    assert!(s.list_tool_calls_for(&[]).unwrap().is_empty());
}

#[test]
fn an_unknown_bot_is_a_missing_reference() {
    let (_dir, mut s) = open();
    let err = s.create_chat(Some("missing"), "t", "m").unwrap_err();
    assert!(err.is_missing_reference(), "{err}");
}
