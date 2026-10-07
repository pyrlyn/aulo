//! Unit checks of the pieces the gRPC tests cannot reach directly.

use aulo_proto::aulo::v1::ModelRef;
use tonic::Code;

use super::convert;
use super::input::{self, ChatCursor, MessageCursor};
use super::{StoreError, storage};

const ID: &str = "01J00000000000000000000000";

#[test]
fn page_size_defaults_and_clamps() {
    assert_eq!(input::page_size(0), 50);
    assert_eq!(input::page_size(1), 1);
    assert_eq!(input::page_size(200), 200);
    assert_eq!(input::page_size(201), 200);
    assert_eq!(input::page_size(u32::MAX), 200);
}

#[test]
fn trim_page_reports_the_cursor_row_only_when_more_remain() {
    let mut rows = vec![1, 2, 3];
    assert_eq!(input::trim_page(&mut rows, 2), Some(&2));
    assert_eq!(rows, [1, 2]);
    assert_eq!(input::trim_page(&mut rows, 2), None);
    assert_eq!(rows, [1, 2]);
}

#[test]
fn cursors_round_trip() {
    let token = ChatCursor::encode(1_700_000_000_123, ID);
    let cursor = ChatCursor::parse(&token).unwrap().unwrap();
    assert_eq!(
        (cursor.updated_at, cursor.id.as_str()),
        (1_700_000_000_123, ID)
    );
    let token = MessageCursor::encode(ID);
    assert_eq!(MessageCursor::parse(&token).unwrap().unwrap().0, ID);
    assert_eq!(ChatCursor::parse("").unwrap(), None);
    assert_eq!(MessageCursor::parse("").unwrap(), None);
}

#[test]
fn model_ref_survives_the_store_string() {
    let m = ModelRef {
        provider: "openai-compatible".into(),
        model: "org/model:7b".into(),
    };
    let stored = input::model(&m).unwrap();
    let chat = convert::chat(aulo_store::Chat {
        id: ID.into(),
        bot_id: None,
        title: String::new(),
        model: stored,
        created_at: 1_500,
        updated_at: 2_999,
    });
    assert_eq!(chat.model, Some(m));
    let (created, updated) = (chat.created_at.unwrap(), chat.updated_at.unwrap());
    assert_eq!((created.seconds, created.nanos), (1, 500_000_000));
    assert_eq!((updated.seconds, updated.nanos), (2, 999_000_000));
}

#[test]
fn a_model_string_without_a_provider_reads_back_as_the_model() {
    let chat = convert::chat(aulo_store::Chat {
        id: ID.into(),
        bot_id: None,
        title: String::new(),
        model: "legacy".into(),
        created_at: 0,
        updated_at: 0,
    });
    let model = chat.model.unwrap();
    assert_eq!(
        (model.provider.as_str(), model.model.as_str()),
        ("", "legacy")
    );
}

#[test]
fn storage_errors_hide_their_details_from_clients() {
    let status = storage(
        "test",
        &StoreError::Migrate("SELECT secret FROM /home/me/db".into()),
    );
    assert_eq!(status.code(), Code::Internal);
    assert_eq!(status.message(), "internal storage error");
}
