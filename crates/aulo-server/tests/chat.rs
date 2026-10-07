//! ChatService over a real Unix socket and a real SQLite file: every RPC,
//! pagination, input validation and the generic error text.
#![cfg(unix)]
// Helpers outside #[test] fns are not covered by allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

mod common;

use std::sync::{Arc, Mutex};

use aulo_proto::aulo::v1::chat_service_client::ChatServiceClient;
use aulo_proto::aulo::v1::chat_service_server::ChatServiceServer;
use aulo_proto::aulo::v1::{
    Chat, CreateChatRequest, DeleteChatRequest, GetChatRequest, ListChatsRequest,
    ListMessagesRequest, MessageRole, ModelRef, RenameChatRequest, SearchMessagesRequest,
    ToolCallStatus,
};
use aulo_server::{ChatApi, Limits};
use aulo_store::Store;
use common::{Running, connect, start_with};
use tonic::transport::Channel;
use tonic::{Code, Status};

const MISSING: &str = "01J00000000000000000000000";

struct Fixture {
    _server: Running,
    // The database must outlive the server.
    _db: tempfile::TempDir,
    store: Arc<Mutex<Store>>,
    client: ChatServiceClient<Channel>,
}

async fn fixture_with(limits: Limits) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aulo.db");
    let store = Arc::new(Mutex::new(Store::open(path.to_str().unwrap()).unwrap()));
    let api = ChatApi::new(Arc::clone(&store));
    let server = start_with(limits, |s| s.add_service(ChatServiceServer::new(api))).await;
    let client = ChatServiceClient::new(connect(&server.socket).await);
    Fixture {
        _server: server,
        _db: dir,
        store,
        client,
    }
}

async fn fixture() -> Fixture {
    fixture_with(Limits::default()).await
}

impl Fixture {
    async fn create(&mut self, title: &str) -> Chat {
        self.client
            .create_chat(CreateChatRequest {
                title: Some(title.into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_inner()
            .chat
            .unwrap()
    }

    fn store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        f(&mut self.store.lock().unwrap())
    }
}

fn code<T: std::fmt::Debug>(r: Result<tonic::Response<T>, Status>) -> Code {
    r.unwrap_err().code()
}

#[tokio::test]
async fn create_get_rename_and_delete_a_chat() {
    let mut f = fixture().await;
    let bot = f.store(|s| s.create_bot("helper").unwrap());
    let model = ModelRef {
        provider: "anthropic".into(),
        model: "claude/sonnet".into(),
    };
    let chat = f
        .client
        .create_chat(CreateChatRequest {
            bot_id: Some(bot.id.clone()),
            title: Some("first".into()),
            model: Some(model.clone()),
        })
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    assert_eq!(chat.id.len(), 26);
    assert_eq!(chat.bot_id.as_deref(), Some(bot.id.as_str()));
    assert_eq!(chat.title, "first");
    assert_eq!(chat.model, Some(model));
    assert_eq!(chat.created_at, chat.updated_at);
    assert!(chat.created_at.unwrap().seconds > 1_700_000_000);

    let got = f
        .client
        .get_chat(GetChatRequest {
            chat_id: chat.id.clone(),
        })
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    assert_eq!(got, chat);

    // A lowercase id is the same ULID, so it finds the same chat.
    let lower = f
        .client
        .get_chat(GetChatRequest {
            chat_id: chat.id.to_lowercase(),
        })
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    assert_eq!(lower.id, chat.id);

    let renamed = f
        .client
        .rename_chat(RenameChatRequest {
            chat_id: chat.id.clone(),
            title: "второй".into(),
        })
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    assert_eq!(renamed.title, "второй");
    assert_eq!(renamed.model, chat.model);
    assert!(renamed.updated_at.unwrap().seconds >= chat.updated_at.unwrap().seconds);

    f.client
        .delete_chat(DeleteChatRequest {
            chat_id: chat.id.clone(),
        })
        .await
        .unwrap();
    let gone = f.client.get_chat(GetChatRequest { chat_id: chat.id }).await;
    assert_eq!(code(gone), Code::NotFound);
}

#[tokio::test]
async fn defaults_apply_when_title_and_model_are_omitted() {
    let mut f = fixture().await;
    let chat = f
        .client
        .create_chat(CreateChatRequest::default())
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    assert_eq!(chat.title, "");
    assert_eq!(chat.bot_id, None);
    assert_eq!(chat.model, Some(ModelRef::default()));
}

#[tokio::test]
async fn create_rejects_bad_input() {
    let mut f = fixture().await;
    let model = |provider: &str, model: &str| {
        Some(ModelRef {
            provider: provider.into(),
            model: model.into(),
        })
    };
    let requests = [
        CreateChatRequest {
            bot_id: Some("not-a-ulid".into()),
            ..Default::default()
        },
        CreateChatRequest {
            title: Some("x".repeat(201)),
            ..Default::default()
        },
        CreateChatRequest {
            title: Some("line\nbreak".into()),
            ..Default::default()
        },
        CreateChatRequest {
            title: Some("esc\u{1b}[31m".into()),
            ..Default::default()
        },
        CreateChatRequest {
            model: model("", "m"),
            ..Default::default()
        },
        CreateChatRequest {
            model: model("p", ""),
            ..Default::default()
        },
        CreateChatRequest {
            model: model("a/b", "m"),
            ..Default::default()
        },
        CreateChatRequest {
            model: model(&"p".repeat(65), "m"),
            ..Default::default()
        },
        CreateChatRequest {
            model: model("p", &"m".repeat(129)),
            ..Default::default()
        },
    ];
    for request in requests {
        let status = f.client.create_chat(request.clone()).await.unwrap_err();
        assert_eq!(status.code(), Code::InvalidArgument, "{request:?}");
    }
    // Exactly 200 bytes is allowed; the cap counts bytes, not characters.
    f.create(&"x".repeat(200)).await;
    assert_eq!(
        code(
            f.client
                .create_chat(CreateChatRequest {
                    title: Some("я".repeat(101)),
                    ..Default::default()
                })
                .await
        ),
        Code::InvalidArgument
    );
    // Nothing was stored by the rejected requests.
    assert_eq!(f.store(|s| s.list_chats(10, 0).unwrap().len()), 1);
}

#[tokio::test]
async fn create_with_an_unknown_bot_is_not_found() {
    let mut f = fixture().await;
    let result = f
        .client
        .create_chat(CreateChatRequest {
            bot_id: Some(MISSING.into()),
            ..Default::default()
        })
        .await;
    let status = result.unwrap_err();
    assert_eq!(status.code(), Code::NotFound);
    assert_eq!(status.message(), "bot not found");
}

#[tokio::test]
async fn ids_are_validated_on_every_chat_rpc() {
    let mut f = fixture().await;
    for bad in ["", "nope", "01J0000000000000000000000U", &"A".repeat(5000)] {
        let id = || bad.to_owned();
        let get = f.client.get_chat(GetChatRequest { chat_id: id() }).await;
        let rename = f
            .client
            .rename_chat(RenameChatRequest {
                chat_id: id(),
                title: "t".into(),
            })
            .await;
        let delete = f
            .client
            .delete_chat(DeleteChatRequest { chat_id: id() })
            .await;
        let messages = f
            .client
            .list_messages(ListMessagesRequest {
                chat_id: id(),
                ..Default::default()
            })
            .await;
        assert_eq!(code(get), Code::InvalidArgument, "{bad:.20}");
        assert_eq!(code(rename), Code::InvalidArgument, "{bad:.20}");
        assert_eq!(code(delete), Code::InvalidArgument, "{bad:.20}");
        assert_eq!(code(messages), Code::InvalidArgument, "{bad:.20}");
    }
    // The error never echoes what the client sent.
    let status = f
        .client
        .get_chat(GetChatRequest {
            chat_id: "secret-looking-value".into(),
        })
        .await
        .unwrap_err();
    assert!(!status.message().contains("secret"), "{status:?}");
}

#[tokio::test]
async fn unknown_chats_are_not_found() {
    let mut f = fixture().await;
    let chat_id = || MISSING.to_owned();
    let get = f
        .client
        .get_chat(GetChatRequest { chat_id: chat_id() })
        .await;
    let rename = f
        .client
        .rename_chat(RenameChatRequest {
            chat_id: chat_id(),
            title: "t".into(),
        })
        .await;
    let delete = f
        .client
        .delete_chat(DeleteChatRequest { chat_id: chat_id() })
        .await;
    let messages = f
        .client
        .list_messages(ListMessagesRequest {
            chat_id: chat_id(),
            ..Default::default()
        })
        .await;
    assert_eq!(code(get), Code::NotFound);
    assert_eq!(code(rename), Code::NotFound);
    assert_eq!(code(delete), Code::NotFound);
    assert_eq!(code(messages), Code::NotFound);
}

#[tokio::test]
async fn rename_validates_the_title() {
    let mut f = fixture().await;
    let chat = f.create("keep").await;
    for title in ["", "   ", "tab\there", &"x".repeat(201)] {
        let result = f
            .client
            .rename_chat(RenameChatRequest {
                chat_id: chat.id.clone(),
                title: title.into(),
            })
            .await;
        assert_eq!(code(result), Code::InvalidArgument, "{title:?}");
    }
    let got = f
        .client
        .get_chat(GetChatRequest { chat_id: chat.id })
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    assert_eq!(got.title, "keep");
}

#[tokio::test]
async fn chats_list_newest_activity_first_in_pages() {
    let mut f = fixture().await;
    let mut ids = Vec::new();
    for i in 0..5 {
        ids.push(f.create(&format!("c{i}")).await.id);
    }
    // Activity moves the oldest chat to the front; the pause makes its timestamp strictly newer.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    f.store(|s| s.append_message(&ids[0], "user", "bump").unwrap());
    let mut want = ids.clone();
    let oldest = want.remove(0);
    want.reverse();
    want.insert(0, oldest);

    let mut got = Vec::new();
    let mut token = String::new();
    let mut pages = 0;
    loop {
        let page = f
            .client
            .list_chats(ListChatsRequest {
                page_size: 2,
                page_token: token.clone(),
                bot_id: None,
            })
            .await
            .unwrap()
            .into_inner();
        pages += 1;
        assert!(page.chats.len() <= 2);
        got.extend(page.chats.into_iter().map(|c| c.id));
        token = page.next_page_token;
        if token.is_empty() {
            break;
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(got, want);

    // Page size 0 is the default, which holds everything here with no next page.
    let all = f
        .client
        .list_chats(ListChatsRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(all.chats.len(), 5);
    assert_eq!(all.next_page_token, "");
}

#[tokio::test]
async fn an_exact_fit_page_has_no_next_token() {
    let mut f = fixture().await;
    f.create("a").await;
    f.create("b").await;
    let page = f
        .client
        .list_chats(ListChatsRequest {
            page_size: 2,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(page.chats.len(), 2);
    assert_eq!(page.next_page_token, "");
}

#[tokio::test]
async fn page_size_is_clamped_to_200() {
    let mut f = fixture().await;
    f.store(|s| {
        for i in 0..201 {
            s.create_chat(None, &format!("c{i}"), "").unwrap();
        }
    });
    let page = f
        .client
        .list_chats(ListChatsRequest {
            page_size: u32::MAX,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(page.chats.len(), 200);
    assert!(!page.next_page_token.is_empty());
    let default = f
        .client
        .list_chats(ListChatsRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(default.chats.len(), 50);
}

#[tokio::test]
async fn chats_filter_by_bot() {
    let mut f = fixture().await;
    let bot = f.store(|s| s.create_bot("helper").unwrap());
    f.create("plain").await;
    let owned = f
        .client
        .create_chat(CreateChatRequest {
            bot_id: Some(bot.id.clone()),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    let page = f
        .client
        .list_chats(ListChatsRequest {
            bot_id: Some(bot.id),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(page.chats, [owned]);
    let bad = f
        .client
        .list_chats(ListChatsRequest {
            bot_id: Some("x".into()),
            ..Default::default()
        })
        .await;
    assert_eq!(code(bad), Code::InvalidArgument);
}

#[tokio::test]
async fn tampered_page_tokens_are_rejected() {
    let mut f = fixture().await;
    let chat = f.create("a").await;
    f.create("b").await;
    f.store(|s| s.append_message(&chat.id, "user", "x").unwrap());
    let chat_token = f
        .client
        .list_chats(ListChatsRequest {
            page_size: 1,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner()
        .next_page_token;
    assert!(!chat_token.is_empty());
    let message_token = {
        let id = f.store(|s| s.list_messages(&chat.id, None, 1).unwrap()[0].id.clone());
        format!("m1.{id}")
    };

    let lowercase = chat_token.to_lowercase();
    let bad = [
        "garbage".to_owned(),
        format!("{chat_token}x"),
        chat_token.replace("c1.", "c2."),
        lowercase,
        "c1.-5.01J00000000000000000000000".to_owned(),
        "c1.abc.01J00000000000000000000000".to_owned(),
        "c1.5".to_owned(),
        message_token.clone(),
        "x".repeat(10_000),
    ];
    for token in bad {
        let result = f
            .client
            .list_chats(ListChatsRequest {
                page_token: token.clone(),
                ..Default::default()
            })
            .await;
        assert_eq!(code(result), Code::InvalidArgument, "{token:.40}");
    }

    // The other direction: a chat token is no message token.
    let bad = [chat_token, "m1.".into(), "m1.short".into(), "m1.é".into()];
    for token in bad {
        let result = f
            .client
            .list_messages(ListMessagesRequest {
                chat_id: chat.id.clone(),
                page_token: token.clone(),
                ..Default::default()
            })
            .await;
        assert_eq!(code(result), Code::InvalidArgument, "{token:.40}");
    }
}

#[tokio::test]
async fn messages_page_oldest_first_with_tool_calls() {
    let mut f = fixture().await;
    let chat = f.create("t").await;
    let other = f.create("other").await;
    let (ids, big) = f.store(|s| {
        let mut ids = Vec::new();
        for i in 0..5 {
            let role = if i % 2 == 0 { "user" } else { "assistant" };
            ids.push(
                s.append_message(&chat.id, role, &format!("m{i}"))
                    .unwrap()
                    .id,
            );
        }
        s.append_message(&other.id, "user", "not mine").unwrap();
        s.record_tool_call(&ids[1], "first", "{\"a\":1}", Some("done"), "succeeded")
            .unwrap();
        s.record_tool_call(&ids[1], "second", "{}", None, "running")
            .unwrap();
        // 'é' is two bytes, so a cut at 64 KiB would land inside a character.
        let big = "é".repeat(40_000);
        s.record_tool_call(&ids[3], "big", "{}", Some(&big), "denied")
            .unwrap();
        s.record_tool_call(&ids[3], "odd", "{}", None, "something-new")
            .unwrap();
        (ids, big)
    });

    let mut got = Vec::new();
    let mut token = String::new();
    let mut pages = 0;
    loop {
        let page = f
            .client
            .list_messages(ListMessagesRequest {
                chat_id: chat.id.clone(),
                page_size: 2,
                page_token: token.clone(),
            })
            .await
            .unwrap()
            .into_inner();
        pages += 1;
        got.extend(page.messages);
        token = page.next_page_token;
        if token.is_empty() {
            break;
        }
    }
    assert_eq!(pages, 3);
    let got_ids: Vec<_> = got.iter().map(|m| m.id.clone()).collect();
    assert_eq!(got_ids, ids);
    assert!(got.iter().all(|m| m.chat_id == chat.id));
    assert_eq!(got[0].role(), MessageRole::User);
    assert_eq!(got[1].role(), MessageRole::Assistant);
    assert_eq!(got[0].content, "m0");
    assert!(got[0].tool_calls.is_empty());

    let calls = &got[1].tool_calls;
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "first");
    assert_eq!(calls[0].arguments_json, "{\"a\":1}");
    assert_eq!(calls[0].output.as_deref(), Some("done"));
    assert_eq!(calls[0].status(), ToolCallStatus::Succeeded);
    assert_eq!(calls[1].name, "second");
    assert_eq!(calls[1].output, None);
    assert_eq!(calls[1].status(), ToolCallStatus::Running);

    let calls = &got[3].tool_calls;
    assert_eq!(calls[0].status(), ToolCallStatus::Denied);
    let output = calls[0].output.as_deref().unwrap();
    assert_eq!(output.len(), 64 * 1024);
    assert!(big.starts_with(output));
    assert_eq!(calls[1].status(), ToolCallStatus::Unspecified);
}

#[tokio::test]
async fn deleting_a_chat_removes_its_messages() {
    let mut f = fixture().await;
    let chat = f.create("t").await;
    f.store(|s| s.append_message(&chat.id, "user", "x").unwrap());
    f.client
        .delete_chat(DeleteChatRequest {
            chat_id: chat.id.clone(),
        })
        .await
        .unwrap();
    let result = f
        .client
        .list_messages(ListMessagesRequest {
            chat_id: chat.id,
            ..Default::default()
        })
        .await;
    assert_eq!(code(result), Code::NotFound);
}

async fn search(
    f: &mut Fixture,
    query: &str,
    chat_id: Option<&str>,
    page_size: u32,
    page_token: &str,
) -> Result<aulo_proto::aulo::v1::SearchMessagesResponse, Status> {
    f.client
        .search_messages(SearchMessagesRequest {
            query: query.into(),
            page_size,
            page_token: page_token.into(),
            chat_id: chat_id.map(str::to_owned),
        })
        .await
        .map(tonic::Response::into_inner)
}

#[tokio::test]
async fn search_finds_messages_and_drops_deleted_ones() {
    let mut f = fixture().await;
    let a = f.create("a").await;
    let b = f.create("b").await;
    let (hit, tool_msg) = f.store(|s| {
        let hit = s
            .append_message(&a.id, "user", "Remember the Zebra")
            .unwrap();
        let tool_msg = s.append_message(&b.id, "assistant", "zebra facts").unwrap();
        s.record_tool_call(&tool_msg.id, "lookup", "{}", Some("ok"), "succeeded")
            .unwrap();
        s.append_message(&a.id, "user", "nothing here").unwrap();
        (hit, tool_msg)
    });

    let all = search(&mut f, "zebra", None, 0, "").await.unwrap();
    assert_eq!(all.messages.len(), 2);
    assert!(all.next_page_token.is_empty());
    let found = all.messages.iter().find(|m| m.id == tool_msg.id).unwrap();
    assert_eq!(found.chat_id, b.id);
    assert_eq!(found.role(), MessageRole::Assistant);
    assert_eq!(found.tool_calls.len(), 1);

    let only_a = search(&mut f, "zebra", Some(&a.id), 0, "").await.unwrap();
    assert_eq!(only_a.messages.len(), 1);
    assert_eq!(only_a.messages[0].id, hit.id);
    assert_eq!(only_a.messages[0].content, "Remember the Zebra");

    f.client
        .delete_chat(DeleteChatRequest { chat_id: b.id })
        .await
        .unwrap();
    let left = search(&mut f, "zebra", None, 0, "").await.unwrap();
    assert_eq!(left.messages.len(), 1);
    assert_eq!(left.messages[0].id, hit.id);
}

#[tokio::test]
async fn search_pages_cover_every_hit_once() {
    let mut f = fixture().await;
    let chat = f.create("t").await;
    let ids: Vec<String> = f.store(|s| {
        (0..5)
            .map(|i| {
                let pad = "pad ".repeat(i);
                s.append_message(&chat.id, "user", &format!("needle {pad}"))
                    .unwrap()
                    .id
            })
            .collect()
    });
    let mut got = Vec::new();
    let mut token = String::new();
    let mut pages = 0;
    loop {
        let page = search(&mut f, "needle", None, 2, &token).await.unwrap();
        pages += 1;
        got.extend(page.messages.into_iter().map(|m| m.id));
        token = page.next_page_token;
        if token.is_empty() {
            break;
        }
    }
    assert_eq!(pages, 3);
    got.sort();
    assert_eq!(got, ids);
}

#[tokio::test]
async fn hostile_search_text_is_a_plain_query() {
    let mut f = fixture().await;
    let chat = f.create("t").await;
    f.store(|s| s.append_message(&chat.id, "user", "plain words").unwrap());
    for q in [
        "\"",
        "*",
        "NEAR(",
        "OR",
        "-",
        "col:",
        "content: x",
        "a OR",
        "\0",
        "x\" OR \"y",
    ] {
        let found = search(&mut f, q, None, 0, "").await;
        assert!(found.is_ok(), "{q:?}");
    }
    assert_eq!(
        search(&mut f, "plain OR", None, 0, "")
            .await
            .unwrap()
            .messages
            .len(),
        0
    );
}

#[tokio::test]
async fn search_rejects_bad_input_without_echoing_it() {
    let mut f = fixture().await;
    let chat = f.create("t").await;
    f.store(|s| s.append_message(&chat.id, "user", "word").unwrap());
    let page = search(&mut f, "word", None, 1, "").await.unwrap();
    assert!(page.next_page_token.is_empty());
    let too_long = "w".repeat(1025);
    let cases = [
        ("", None, ""),
        ("   ", None, ""),
        (too_long.as_str(), None, ""),
        ("word", Some("not-a-ulid"), ""),
        ("word", None, "garbage"),
        ("word", None, "s1.nan.01J00000000000000000000000"),
        ("word", None, "s1.inf.01J00000000000000000000000"),
        ("word", None, "s1.1.5"),
        ("word", None, "s1.1"),
        ("word", None, "m1.01J00000000000000000000000"),
        ("word", None, "c1.5.01J00000000000000000000000"),
    ];
    for (q, chat_id, token) in cases {
        let err = search(&mut f, q, chat_id, 0, token).await.unwrap_err();
        assert_eq!(err.code(), Code::InvalidArgument, "{q:.20} {token}");
        assert!(!err.message().contains("garbage"));
    }
}

#[tokio::test]
async fn message_limits_apply_to_the_chat_service() {
    let mut f = fixture_with(Limits {
        max_decoding_message_size: 1024,
        ..Limits::default()
    })
    .await;
    let result = f
        .client
        .create_chat(CreateChatRequest {
            title: Some("x".repeat(4096)),
            ..Default::default()
        })
        .await;
    assert_eq!(code(result), Code::OutOfRange);
}
