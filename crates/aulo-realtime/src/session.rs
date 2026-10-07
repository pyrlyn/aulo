use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout, timeout_at};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_tls_with_config};
use tokio_util::task::AbortOnDropHandle;

use crate::config::bearer;
use crate::decode::{Decoder, MAX_TEXT_BYTES, ServerEvent};
use crate::{Dialect, RealtimeConfig, RealtimeError, wire};

/// A reply is a few seconds of audio as base64; a bigger message is a broken or hostile server.
const MAX_MESSAGE_BYTES: usize = 1 << 20;
/// Larger microphone buffers are split, so one command never holds a long recording.
const MAX_AUDIO_CHUNK_BYTES: usize = 32 * 1024;
/// Messages the server may send before the session starts.
const MAX_PRE_READY_MESSAGES: usize = 16;
const COMMAND_QUEUE: usize = 64;
const EVENT_QUEUE: usize = 256;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type Item = Result<ServerEvent, RealtimeError>;

enum Command {
    Text(String),
    Close,
}

/// One open voice session. Dropping it closes the socket without a goodbye.
///
/// Keep calling [`recv`](Session::recv) while sending: a consumer that stops
/// reading stops the socket (the server is slowed, not buffered), and with it
/// the sending side.
#[derive(Debug)]
pub struct Session {
    dialect: Dialect,
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<Item>,
    _task: AbortOnDropHandle<()>,
}

impl Session {
    /// Connects, configures the session and waits until it takes audio; the
    /// first [`recv`](Session::recv) then yields [`ServerEvent::Ready`].
    pub async fn connect(config: &RealtimeConfig) -> Result<Self, RealtimeError> {
        config.validate()?;
        let mut request = config
            .socket_url()?
            .as_str()
            .into_client_request()
            .map_err(|_| RealtimeError::invalid("base url", "not a WebSocket address"))?;
        request
            .headers_mut()
            .insert(AUTHORIZATION, bearer(&config.api_key)?);
        let limits = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (mut socket, _) = timeout(
            config.timeout,
            connect_async_tls_with_config(request, Some(limits), false, None),
        )
        .await
        .map_err(|_| RealtimeError::TimedOut)?
        .map_err(connect_error)?;

        let mut decoder = Decoder::new(config.dialect);
        let start = wire::start(config);
        timeout(config.timeout, async {
            // GPT-Live reads its configuration from the first message; Realtime
            // greets first and only then takes `session.update`.
            if config.dialect == Dialect::Live {
                send(&mut socket, start.clone()).await?;
            }
            await_ready(&mut socket, &mut decoder).await?;
            if config.dialect == Dialect::Realtime {
                send(&mut socket, start).await?;
            }
            Ok::<(), RealtimeError>(())
        })
        .await
        .map_err(|_| RealtimeError::TimedOut)??;

        let (commands, command_rx) = mpsc::channel(COMMAND_QUEUE);
        let (event_tx, events) = mpsc::channel(EVENT_QUEUE);
        // Room for it is guaranteed: the queue is empty.
        let _ = event_tx.try_send(Ok(ServerEvent::Ready));
        let task = tokio::spawn(run(
            socket,
            decoder,
            command_rx,
            event_tx,
            config.dialect,
            config.timeout,
        ));
        Ok(Self {
            dialect: config.dialect,
            commands,
            events,
            _task: AbortOnDropHandle::new(task),
        })
    }

    /// The next event; an `Err` is the last one, and `None` follows the end of the session.
    pub async fn recv(&mut self) -> Option<Item> {
        self.events.recv().await
    }

    /// Microphone audio in the session format: whole 16-bit samples for PCM,
    /// in order. There is no acknowledgment.
    pub async fn send_audio(&self, pcm: &[u8]) -> Result<(), RealtimeError> {
        if !pcm.len().is_multiple_of(2) {
            return Err(RealtimeError::invalid("audio", "whole 16-bit samples"));
        }
        for chunk in pcm.chunks(MAX_AUDIO_CHUNK_BYTES) {
            self.command(Command::Text(wire::audio(self.dialect, chunk)))
                .await?;
        }
        Ok(())
    }

    /// Stops the reply being generated. Realtime only.
    pub async fn cancel_response(&self) -> Result<(), RealtimeError> {
        self.realtime_only()?;
        self.command(Command::Text(wire::cancel())).await
    }

    /// Answers a [`ToolCall`](crate::ToolCall) and lets the model speak about the result. Realtime only.
    pub async fn tool_output(&self, call_id: &str, output: &str) -> Result<(), RealtimeError> {
        self.realtime_only()?;
        if call_id.is_empty() || call_id.len() > 128 || output.len() > MAX_TEXT_BYTES {
            return Err(RealtimeError::invalid(
                "tool output",
                "id or output too long",
            ));
        }
        for message in wire::tool_output(call_id, output) {
            self.command(Command::Text(message)).await?;
        }
        Ok(())
    }

    /// Ends the session. Keep calling [`recv`](Session::recv) until `None`:
    /// GPT-Live reports its final usage in [`ServerEvent::Closed`] first.
    pub async fn close(&self) {
        let _ = self.commands.send(Command::Close).await;
    }

    fn realtime_only(&self) -> Result<(), RealtimeError> {
        if self.dialect == Dialect::Realtime {
            Ok(())
        } else {
            Err(RealtimeError::Unsupported)
        }
    }

    async fn command(&self, command: Command) -> Result<(), RealtimeError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| RealtimeError::Closed)
    }
}

async fn send(socket: &mut Socket, text: String) -> Result<(), RealtimeError> {
    socket.send(Message::text(text)).await.map_err(stream_error)
}

async fn await_ready(socket: &mut Socket, decoder: &mut Decoder) -> Result<(), RealtimeError> {
    for _ in 0..MAX_PRE_READY_MESSAGES {
        let message = socket.next().await.ok_or(RealtimeError::Closed)?;
        let Some(text) = text_of(message.map_err(stream_error)?)? else {
            continue;
        };
        for event in decoder.decode(&text)? {
            match event {
                ServerEvent::Ready => return Ok(()),
                ServerEvent::Aulo(_) => {
                    return Err(RealtimeError::Protocol("the session was refused"));
                }
                _ => {}
            }
        }
    }
    Err(RealtimeError::Protocol("no session start"))
}

/// The JSON text of a message; `None` for control frames.
fn text_of(message: Message) -> Result<Option<String>, RealtimeError> {
    match message {
        Message::Text(text) => Ok(Some(text.as_str().to_owned())),
        Message::Close(_) => Err(RealtimeError::Closed),
        Message::Binary(_) => Err(RealtimeError::Protocol("unexpected binary message")),
        _ => Ok(None),
    }
}

async fn run(
    mut socket: Socket,
    mut decoder: Decoder,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Item>,
    dialect: Dialect,
    limit: Duration,
) {
    // Set once the caller asked to close: from then on only the server's goodbye is waited for.
    let mut deadline: Option<Instant> = None;
    loop {
        let incoming = tokio::select! {
            command = commands.recv(), if deadline.is_none() => {
                match command {
                    Some(Command::Text(text)) => {
                        if let Err(error) = send(&mut socket, text).await {
                            let _ = events.send(Err(error)).await;
                            return;
                        }
                    }
                    Some(Command::Close) | None => {
                        deadline = Some(Instant::now() + limit);
                        // GPT-Live finalizes usage on `session.close`; Realtime has no such message.
                        let goodbye = match dialect {
                            Dialect::Live => Message::text(wire::live_close()),
                            Dialect::Realtime => Message::Close(None),
                        };
                        let _ = socket.send(goodbye).await;
                    }
                }
                continue;
            }
            message = read(&mut socket, deadline) => message,
        };
        let closing = deadline.is_some();
        let text = match incoming {
            // Once the caller asked to close, however the socket ends is the end.
            None | Some(Err(_)) if closing => return,
            None => return fail(&events, RealtimeError::Closed).await,
            Some(Err(error)) => return fail(&events, stream_error(error)).await,
            Some(Ok(message)) => match text_of(message) {
                Ok(Some(text)) => text,
                Ok(None) => continue,
                Err(RealtimeError::Closed) if closing => return,
                Err(error) => return fail(&events, error).await,
            },
        };
        let decoded = match decoder.decode(&text) {
            Ok(decoded) => decoded,
            Err(error) => return fail(&events, error).await,
        };
        for event in decoded {
            let ended = matches!(event, ServerEvent::Closed { .. });
            if events.send(Ok(event)).await.is_err() {
                return;
            }
            if ended {
                let _ = socket.close(None).await;
                return;
            }
        }
    }
}

async fn read(socket: &mut Socket, deadline: Option<Instant>) -> Option<Result<Message, WsError>> {
    match deadline {
        // A server that never says goodbye is cut off.
        Some(deadline) => timeout_at(deadline, socket.next()).await.ok().flatten(),
        None => socket.next().await,
    }
}

async fn fail(events: &mpsc::Sender<Item>, error: RealtimeError) {
    let _ = events.send(Err(error)).await;
}

fn connect_error(error: WsError) -> RealtimeError {
    match error {
        WsError::Http(response) => RealtimeError::from_status(response.status().as_u16()),
        WsError::Io(_) | WsError::Tls(_) => RealtimeError::Unreachable,
        _ => RealtimeError::Protocol("the handshake failed"),
    }
}

fn stream_error(error: WsError) -> RealtimeError {
    match error {
        WsError::Capacity(_) => RealtimeError::Protocol("message too large"),
        WsError::ConnectionClosed | WsError::AlreadyClosed => RealtimeError::Closed,
        _ => RealtimeError::Protocol("the connection failed"),
    }
}
