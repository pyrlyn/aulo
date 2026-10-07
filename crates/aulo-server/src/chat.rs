//! `aulo.v1.ChatService` over `aulo-store`.
//!
//! The store is one synchronous SQLite connection, so every call locks it and
//! runs on the blocking pool; a handler never holds the lock across an await.
//! Clients get generic error text: store errors may carry SQL, paths or row
//! values, so they go to the log only.

mod convert;
mod input;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex, PoisonError};

use aulo_proto::aulo::v1::chat_service_server::ChatService;
use aulo_proto::aulo::v1::{
    CreateChatRequest, CreateChatResponse, DeleteChatRequest, DeleteChatResponse, GetChatRequest,
    GetChatResponse, ListChatsRequest, ListChatsResponse, ListMessagesRequest,
    ListMessagesResponse, RenameChatRequest, RenameChatResponse, SearchMessagesRequest,
    SearchMessagesResponse,
};
use aulo_store::{Store, StoreError};
use tonic::{Request, Response, Status};

use input::{ChatCursor, MessageCursor, SearchCursor};

/// Serves chat CRUD and message history. Clone-cheap: clones share one store.
#[derive(Debug, Clone)]
pub struct ChatApi {
    store: Arc<Mutex<Store>>,
}

impl ChatApi {
    #[must_use]
    pub fn new(store: Arc<Mutex<Store>>) -> Self {
        Self { store }
    }

    async fn with_store<T, F>(&self, op: &'static str, f: F) -> Result<T, Status>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store) -> Result<T, Status> + Send + 'static,
    {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || {
            // A panic in another call must not take chat history offline: the
            // connection rolls back an unfinished transaction when dropped.
            let mut guard = store.lock().unwrap_or_else(PoisonError::into_inner);
            f(&mut guard)
        })
        .await
        .map_err(|e| {
            tracing::error!(op, error = %e, "chat storage task failed");
            internal()
        })?
    }
}

fn internal() -> Status {
    Status::internal("internal storage error")
}

fn storage(op: &'static str, e: &StoreError) -> Status {
    tracing::error!(op, error = %e, "chat storage call failed");
    internal()
}

fn no_such_chat() -> Status {
    Status::not_found("chat not found")
}

#[tonic::async_trait]
impl ChatService for ChatApi {
    async fn create_chat(
        &self,
        request: Request<CreateChatRequest>,
    ) -> Result<Response<CreateChatResponse>, Status> {
        let req = request.into_inner();
        let bot_id = req.bot_id.as_deref().map(input::bot_id).transpose()?;
        let title = input::title(req.title.as_deref().unwrap_or_default(), true)?;
        let model = req.model.as_ref().map(input::model).transpose()?;
        let chat = self
            .with_store("create_chat", move |s| {
                s.create_chat(
                    bot_id.as_deref(),
                    &title,
                    model.as_deref().unwrap_or_default(),
                )
                .map_err(|e| {
                    if e.is_missing_reference() {
                        Status::not_found("bot not found")
                    } else {
                        storage("create_chat", &e)
                    }
                })
            })
            .await?;
        Ok(Response::new(CreateChatResponse {
            chat: Some(convert::chat(chat)),
        }))
    }

    async fn list_chats(
        &self,
        request: Request<ListChatsRequest>,
    ) -> Result<Response<ListChatsResponse>, Status> {
        let req = request.into_inner();
        let bot_id = req.bot_id.as_deref().map(input::bot_id).transpose()?;
        let size = input::page_size(req.page_size);
        let after = ChatCursor::parse(&req.page_token)?;
        let mut rows = self
            .with_store("list_chats", move |s| {
                let after = after.as_ref().map(|c| (c.updated_at, c.id.as_str()));
                s.list_chats_page(bot_id.as_deref(), after, input::fetch_limit(size))
                    .map_err(|e| storage("list_chats", &e))
            })
            .await?;
        let next_page_token = input::trim_page(&mut rows, size)
            .map(|c| ChatCursor::encode(c.updated_at, &c.id))
            .unwrap_or_default();
        Ok(Response::new(ListChatsResponse {
            chats: rows.into_iter().map(convert::chat).collect(),
            next_page_token,
        }))
    }

    async fn get_chat(
        &self,
        request: Request<GetChatRequest>,
    ) -> Result<Response<GetChatResponse>, Status> {
        let id = input::chat_id(&request.into_inner().chat_id)?;
        let chat = self
            .with_store("get_chat", move |s| {
                s.get_chat(&id)
                    .map_err(|e| storage("get_chat", &e))?
                    .ok_or_else(no_such_chat)
            })
            .await?;
        Ok(Response::new(GetChatResponse {
            chat: Some(convert::chat(chat)),
        }))
    }

    async fn rename_chat(
        &self,
        request: Request<RenameChatRequest>,
    ) -> Result<Response<RenameChatResponse>, Status> {
        let req = request.into_inner();
        let id = input::chat_id(&req.chat_id)?;
        let title = input::title(&req.title, false)?;
        let chat = self
            .with_store("rename_chat", move |s| {
                if !s
                    .rename_chat(&id, &title)
                    .map_err(|e| storage("rename_chat", &e))?
                {
                    return Err(no_such_chat());
                }
                // A concurrent delete between the two calls is reported as not found.
                s.get_chat(&id)
                    .map_err(|e| storage("rename_chat", &e))?
                    .ok_or_else(no_such_chat)
            })
            .await?;
        Ok(Response::new(RenameChatResponse {
            chat: Some(convert::chat(chat)),
        }))
    }

    async fn delete_chat(
        &self,
        request: Request<DeleteChatRequest>,
    ) -> Result<Response<DeleteChatResponse>, Status> {
        let id = input::chat_id(&request.into_inner().chat_id)?;
        self.with_store("delete_chat", move |s| match s.delete_chat(&id) {
            Ok(true) => Ok(()),
            Ok(false) => Err(no_such_chat()),
            Err(e) => Err(storage("delete_chat", &e)),
        })
        .await?;
        Ok(Response::new(DeleteChatResponse {}))
    }

    async fn list_messages(
        &self,
        request: Request<ListMessagesRequest>,
    ) -> Result<Response<ListMessagesResponse>, Status> {
        let req = request.into_inner();
        let id = input::chat_id(&req.chat_id)?;
        let size = input::page_size(req.page_size);
        let after = MessageCursor::parse(&req.page_token)?;
        let (mut rows, calls) = self
            .with_store("list_messages", move |s| {
                let fail = |e: StoreError| storage("list_messages", &e);
                s.get_chat(&id).map_err(fail)?.ok_or_else(no_such_chat)?;
                let rows = s
                    .list_messages(
                        &id,
                        after.as_ref().map(|c| c.0.as_str()),
                        input::fetch_limit(size),
                    )
                    .map_err(fail)?;
                // The extra row only proves a next page exists; its calls are not needed.
                let kept = &rows[..rows.len().min(size)];
                let ids: Vec<String> = kept.iter().map(|m| m.id.clone()).collect();
                let calls = s.list_tool_calls_for(&ids).map_err(fail)?;
                Ok((rows, calls))
            })
            .await?;
        let next_page_token = input::trim_page(&mut rows, size)
            .map(|m| MessageCursor::encode(&m.id))
            .unwrap_or_default();
        Ok(Response::new(ListMessagesResponse {
            messages: convert::messages(rows, calls),
            next_page_token,
        }))
    }

    async fn search_messages(
        &self,
        request: Request<SearchMessagesRequest>,
    ) -> Result<Response<SearchMessagesResponse>, Status> {
        let req = request.into_inner();
        let query = input::search_query(&req.query)?;
        let chat_id = req.chat_id.as_deref().map(input::chat_id).transpose()?;
        let size = input::page_size(req.page_size);
        let after = SearchCursor::parse(&req.page_token)?;
        let (mut hits, calls) = self
            .with_store("search_messages", move |s| {
                let fail = |e: StoreError| storage("search_messages", &e);
                let hits = s
                    .search_messages(
                        &query,
                        chat_id.as_deref(),
                        after.as_ref().map(|c| (c.rank, c.id.as_str())),
                        input::fetch_limit(size),
                    )
                    .map_err(fail)?;
                let ids: Vec<String> = hits
                    .iter()
                    .take(size)
                    .map(|h| h.message.id.clone())
                    .collect();
                let calls = s.list_tool_calls_for(&ids).map_err(fail)?;
                Ok((hits, calls))
            })
            .await?;
        let next_page_token = input::trim_page(&mut hits, size)
            .map(|h| SearchCursor::encode(h.rank, &h.message.id))
            .unwrap_or_default();
        Ok(Response::new(SearchMessagesResponse {
            messages: convert::messages(hits.into_iter().map(|h| h.message).collect(), calls),
            next_page_token,
        }))
    }
}
