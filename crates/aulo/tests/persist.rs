//! `aulod` opens `aulo.db` and serves the store and one grant over the local socket.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use aulo_proto::aulo::v1::approval_service_client::ApprovalServiceClient;
use aulo_proto::aulo::v1::chat_service_client::ChatServiceClient;
use aulo_proto::aulo::v1::{
    CreateChatRequest, GrantDecision, ListChatsRequest, ListGrantsRequest, RevokeGrantRequest,
};
use aulo_server::local_socket_path;
use aulo_store::Store;
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

const AULOD: &str = env!("CARGO_BIN_EXE_aulod");

struct Daemon {
    child: Child,
    stderr: Option<JoinHandle<String>>,
}

impl Daemon {
    fn stop(&mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stderr
            .take()
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn spawn(home: &Path) -> Daemon {
    let mut child = Command::new(AULOD);
    child
        .arg("--home")
        .arg(home)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars() {
        if key.starts_with("AULO_") {
            child.env_remove(key);
        }
    }
    let mut child = child.spawn().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let stderr = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    });
    Daemon {
        child,
        stderr: Some(stderr),
    }
}

async fn connect(socket: &Path) -> Channel {
    let socket = socket.to_owned();
    Endpoint::from_static("http://[::]:50051")
        .connect_timeout(Duration::from_secs(2))
        .connect_with_connector(service_fn(move |_| {
            let socket = socket.clone();
            async move { Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(socket).await?)) }
        }))
        .await
        .unwrap()
}

#[tokio::test]
async fn aulod_serves_a_chat_and_one_grant() {
    let home = tempfile::tempdir().unwrap();
    let db = aulo_store::database_path(home.path());
    let grant_id = {
        let mut store = Store::open(db.to_str().unwrap()).unwrap();
        store
            .put_grant("app:Safari", "browse", "allow", None)
            .unwrap()
            .id
    };

    let mut daemon = spawn(home.path());
    let socket = local_socket_path(home.path());
    let started = Instant::now();
    while !socket.exists() {
        if started.elapsed() > Duration::from_secs(15) || daemon.child.try_wait().unwrap().is_some()
        {
            let err = daemon.stop();
            panic!("aulod did not bind {socket:?}: {err}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let channel = connect(&socket).await;
    let mut chats = ChatServiceClient::new(channel.clone());
    let mut grants = ApprovalServiceClient::new(channel);

    let listed = grants
        .list_grants(ListGrantsRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(listed.grants.len(), 1);
    assert_eq!(listed.grants[0].id, grant_id);
    assert_eq!(listed.grants[0].subject, "app:Safari");
    assert_eq!(listed.grants[0].scope, "browse");
    assert_eq!(listed.grants[0].decision(), GrantDecision::Allow);

    let chat = chats
        .create_chat(CreateChatRequest {
            title: Some("from aulod".into()),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner()
        .chat
        .unwrap();
    let chats = chats
        .list_chats(ListChatsRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(chats.chats.len(), 1);
    assert_eq!(chats.chats[0].id, chat.id);
    assert_eq!(chats.chats[0].title, "from aulod");

    grants
        .revoke_grant(RevokeGrantRequest { grant_id })
        .await
        .unwrap();
    let left = grants
        .list_grants(ListGrantsRequest::default())
        .await
        .unwrap()
        .into_inner();
    assert!(left.grants.is_empty());

    // The rows are in the daemon's database, not a side file.
    let mut store = Store::open(db.to_str().unwrap()).unwrap();
    assert_eq!(store.list_grants().unwrap().len(), 0);
    assert_eq!(store.list_chats(10, 0).unwrap().len(), 1);
}
