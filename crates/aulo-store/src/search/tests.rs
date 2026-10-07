use tempfile::TempDir;

use super::*;
use crate::schema::messages;

fn open() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("aulo.db");
    let store = Store::open(path.to_str().unwrap()).unwrap();
    (dir, store)
}

fn ids(hits: &[SearchHit]) -> Vec<&str> {
    hits.iter().map(|h| h.message.id.as_str()).collect()
}

#[test]
fn finds_a_message_by_a_word_case_and_diacritics_folded() {
    let (_d, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    let hit = s
        .append_message(&chat.id, "user", "Plan the Café trip")
        .unwrap();
    s.append_message(&chat.id, "user", "unrelated").unwrap();

    let found = s.search_messages("cafe", None, None, 10).unwrap();
    assert_eq!(ids(&found), [hit.id.as_str()]);
    assert_eq!(found[0].message, hit);
    assert!(
        s.search_messages("absent", None, None, 10)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn all_words_must_match() {
    let (_d, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    let both = s.append_message(&chat.id, "user", "red apple pie").unwrap();
    s.append_message(&chat.id, "user", "red car").unwrap();
    let found = s.search_messages("red apple", None, None, 10).unwrap();
    assert_eq!(ids(&found), [both.id.as_str()]);
}

#[test]
fn updated_message_is_found_by_its_new_word_only() {
    let (_d, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    let msg = s.append_message(&chat.id, "user", "alpha").unwrap();
    diesel::update(messages::table.find(&msg.id))
        .set(messages::content.eq("beta"))
        .execute(&mut s.conn)
        .unwrap();
    assert!(
        s.search_messages("alpha", None, None, 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ids(&s.search_messages("beta", None, None, 10).unwrap()),
        [msg.id.as_str()]
    );
}

#[test]
fn deleted_messages_and_chats_vanish() {
    let (_d, mut s) = open();
    let keep = s.create_chat(None, "t", "m").unwrap();
    let gone = s.create_chat(None, "t", "m").unwrap();
    let kept = s.append_message(&keep.id, "user", "needle one").unwrap();
    let single = s.append_message(&keep.id, "user", "needle two").unwrap();
    s.append_message(&gone.id, "user", "needle three").unwrap();

    diesel::delete(messages::table.find(&single.id))
        .execute(&mut s.conn)
        .unwrap();
    assert_eq!(
        ids(&s.search_messages("needle", None, None, 10).unwrap()).len(),
        2
    );
    assert!(s.delete_chat(&gone.id).unwrap());
    let found = s.search_messages("needle", None, None, 10).unwrap();
    assert_eq!(ids(&found), [kept.id.as_str()]);
}

#[test]
fn chat_filter_limits_results() {
    let (_d, mut s) = open();
    let a = s.create_chat(None, "t", "m").unwrap();
    let b = s.create_chat(None, "t", "m").unwrap();
    let in_a = s.append_message(&a.id, "user", "shared word").unwrap();
    s.append_message(&b.id, "user", "shared word").unwrap();
    let found = s.search_messages("shared", Some(&a.id), None, 10).unwrap();
    assert_eq!(ids(&found), [in_a.id.as_str()]);
    assert_eq!(
        s.search_messages("shared", None, None, 10).unwrap().len(),
        2
    );
}

#[test]
fn migration_indexes_messages_that_already_exist() {
    use diesel_migrations::MigrationHarness;
    let (_d, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    let msg = s.append_message(&chat.id, "user", "legacy words").unwrap();
    s.conn.revert_last_migration(crate::MIGRATIONS).unwrap();
    // While the index is gone, new rows are not indexed by any trigger.
    let late = s.append_message(&chat.id, "user", "legacy late").unwrap();
    s.conn.run_pending_migrations(crate::MIGRATIONS).unwrap();
    let found = s.search_messages("legacy", None, None, 10).unwrap();
    let mut got = ids(&found);
    got.sort_unstable();
    let mut want = [msg.id.as_str(), late.id.as_str()];
    want.sort_unstable();
    assert_eq!(got, want);
}

#[test]
fn hostile_queries_are_plain_words_not_errors() {
    let (_d, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    s.append_message(&chat.id, "user", "this OR that and NEAR things content")
        .unwrap();
    for q in [
        "\"",
        "\"\"\"",
        "*",
        "NEAR(",
        "NEAR(a b)",
        "OR",
        "AND",
        "NOT",
        "-",
        "- x",
        "content:",
        "col:",
        "^x",
        "(",
        ")",
        "a OR",
        "{content}: x",
        "\"unterminated",
        "x*",
        "\0",
        "'; DROP TABLE messages; --",
        "\u{202e}",
        "",
    ] {
        s.search_messages(q, None, None, 10)
            .unwrap_or_else(|e| panic!("query {q:?} failed: {e}"));
    }
    // Operators are literal words: "OR" finds the message that contains the word.
    assert_eq!(s.search_messages("OR", None, None, 10).unwrap().len(), 1);
    assert_eq!(
        s.search_messages("content:", None, None, 10).unwrap().len(),
        1
    );
    assert!(s.search_messages("\"", None, None, 10).unwrap().is_empty());
    assert_eq!(chats_left(&mut s), 1);
}

fn chats_left(s: &mut Store) -> i64 {
    use crate::schema::chats;
    chats::table.count().get_result(&mut s.conn).unwrap()
}

#[test]
fn match_expression_quotes_and_caps() {
    assert_eq!(
        match_expression("a OR b").as_deref(),
        Some("\"a\" \"OR\" \"b\"")
    );
    assert_eq!(
        match_expression("say \"hi\"").as_deref(),
        Some("\"say\" \"\"\"hi\"\"\"")
    );
    assert_eq!(match_expression("\" - *"), None);
    assert_eq!(match_expression("   "), None);
    let many = "w ".repeat(500);
    assert_eq!(
        match_expression(&many).unwrap().split(' ').count(),
        MAX_TERMS
    );
    // A cut inside a multi-byte character must not panic.
    let long = "é".repeat(MAX_QUERY_BYTES);
    assert!(match_expression(&long).is_some());
}

#[test]
fn pages_cover_every_hit_once_best_first() {
    let (_d, mut s) = open();
    let chat = s.create_chat(None, "t", "m").unwrap();
    // Different lengths give different bm25 ranks; equal ones exercise the id tie-break.
    for i in 0..7 {
        let filler = "pad ".repeat(i % 4);
        s.append_message(&chat.id, "user", &format!("page {filler}"))
            .unwrap();
    }
    let all = s.search_messages("page", None, None, 100).unwrap();
    assert_eq!(all.len(), 7);
    assert!(
        all.windows(2)
            .all(|w| (w[0].rank, &w[0].message.id) < (w[1].rank, &w[1].message.id))
    );

    let mut seen = Vec::new();
    let mut after: Option<(f64, String)> = None;
    loop {
        let page = s
            .search_messages("page", None, after.as_ref().map(|a| (a.0, a.1.as_str())), 3)
            .unwrap();
        if page.is_empty() {
            break;
        }
        let last = page.last().unwrap();
        after = Some((last.rank, last.message.id.clone()));
        seen.extend(page.into_iter().map(|h| h.message.id));
    }
    let want: Vec<String> = all.into_iter().map(|h| h.message.id).collect();
    assert_eq!(seen, want);
}
