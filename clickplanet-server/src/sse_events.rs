//! Server-Sent Events: `GET /v2/events` streams what the server broadcasts (ownership updates and
//! clicks) as `text/event-stream`.
//!
//! - Each event has a monotonic `id:`, an `event:` type (`update` = `UpdateNotification`,
//!   `click` = `Click`) and the protobuf message in base64 in `data:`.
//! - A client reconnecting with `Last-Event-ID` (sent automatically by `EventSource`, or the
//!   `lastEventId` query parameter) gets the events it missed from an in-memory ring buffer. If they
//!   are no longer in the buffer, it gets a `resync` event and should reload the ownerships.
//! - A slow client that lags behind the broadcast channel is caught up the same way.
//! - A `:` comment is sent every `heartbeat` (`SSE_HEARTBEAT_SECS`, 20 s by default) so proxies and Cloud Run keep the
//!   connection open.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::stream::{self, Stream};
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseMessage {
    pub id: u64,
    pub kind: &'static str,
    /// base64 protobuf
    pub data: String,
}

struct Ring {
    next_id: u64,
    events: VecDeque<Arc<SseMessage>>,
    capacity: usize,
}

/// Assigns ids, keeps the recent events for replay and fans them out.
pub struct EventHub {
    ring: Mutex<Ring>,
    tx: broadcast::Sender<Arc<SseMessage>>,
}

/// What a (re)connecting client gets before the live events.
#[derive(Debug, PartialEq)]
pub enum Backlog {
    Replay(Vec<Arc<SseMessage>>),
    /// The missed events are gone: the client must reload its state.
    Resync,
}

impl EventHub {
    pub fn new(capacity: usize, channel_size: usize) -> Arc<Self> {
        let (tx, _) = broadcast::channel(channel_size.max(1));
        Arc::new(EventHub {
            ring: Mutex::new(Ring { next_id: 1, events: VecDeque::new(), capacity: capacity.max(1) }),
            tx,
        })
    }

    /// Publishes a protobuf message; returns its id.
    pub fn publish(&self, kind: &'static str, protobuf: &[u8]) -> u64 {
        let mut ring = self.ring.lock().unwrap();
        let message = Arc::new(SseMessage { id: ring.next_id, kind, data: STANDARD.encode(protobuf) });
        ring.next_id += 1;
        if ring.events.len() == ring.capacity {
            ring.events.pop_front();
        }
        ring.events.push_back(message.clone());
        // sent under the lock so that ids reach subscribers in order
        let _ = self.tx.send(message.clone());
        message.id
    }

    /// Id of the last published event (0 if none).
    pub fn last_id(&self) -> u64 {
        self.ring.lock().unwrap().next_id - 1
    }

    /// Events after `last_id` still in the buffer.
    fn backlog_locked(ring: &Ring, last_id: u64) -> Backlog {
        let latest = ring.next_id - 1;
        if last_id >= latest {
            return Backlog::Replay(Vec::new());
        }
        match ring.events.front() {
            Some(first) if first.id <= last_id + 1 => {
                Backlog::Replay(ring.events.iter().filter(|e| e.id > last_id).cloned().collect())
            }
            _ => Backlog::Resync,
        }
    }

    pub fn backlog(&self, last_id: u64) -> Backlog {
        Self::backlog_locked(&self.ring.lock().unwrap(), last_id)
    }

    /// Subscribes atomically: no event can fall between the backlog and the live receiver.
    pub fn subscribe(&self, last_id: Option<u64>) -> (Backlog, broadcast::Receiver<Arc<SseMessage>>) {
        let ring = self.ring.lock().unwrap();
        let rx = self.tx.subscribe();
        let backlog = match last_id {
            Some(id) => Self::backlog_locked(&ring, id),
            None => Backlog::Replay(Vec::new()),
        };
        (backlog, rx)
    }
}

fn to_event(message: &SseMessage) -> Event {
    Event::default().id(message.id.to_string()).event(message.kind).data(&message.data)
}

fn resync_event(last_id: u64) -> Event {
    Event::default().id(last_id.to_string()).event("resync").data("")
}

struct StreamState {
    hub: Arc<EventHub>,
    rx: broadcast::Receiver<Arc<SseMessage>>,
    pending: VecDeque<Event>,
    last_sent: u64,
}

/// The event stream of one client.
pub fn event_stream(hub: Arc<EventHub>, last_event_id: Option<u64>) -> impl Stream<Item = Result<Event, Infallible>> {
    let (backlog, rx) = hub.subscribe(last_event_id);
    let mut last_sent = last_event_id.unwrap_or_else(|| hub.last_id());
    let mut pending = VecDeque::new();
    match backlog {
        Backlog::Replay(events) => {
            for e in events {
                last_sent = e.id;
                pending.push_back(to_event(&e));
            }
        }
        Backlog::Resync => {
            last_sent = hub.last_id();
            pending.push_back(resync_event(last_sent));
        }
    }
    let state = StreamState { hub, rx, pending, last_sent };
    stream::unfold(state, |mut state| async move {
        loop {
            if let Some(event) = state.pending.pop_front() {
                return Some((Ok(event), state));
            }
            match state.rx.recv().await {
                Ok(message) => {
                    if message.id <= state.last_sent {
                        continue;
                    }
                    state.last_sent = message.id;
                    return Some((Ok(to_event(&message)), state));
                }
                Err(RecvError::Lagged(_)) => {
                    // slow client: catch up from the buffer, or ask for a resync
                    match state.hub.backlog(state.last_sent) {
                        Backlog::Replay(events) => {
                            for e in events {
                                state.last_sent = e.id;
                                state.pending.push_back(to_event(&e));
                            }
                        }
                        Backlog::Resync => {
                            state.last_sent = state.hub.last_id();
                            state.pending.push_back(resync_event(state.last_sent));
                        }
                    }
                }
                Err(RecvError::Closed) => return None,
            }
        }
    })
}

#[derive(Clone)]
pub struct SseState {
    pub hub: Arc<EventHub>,
    pub heartbeat: Duration,
}

#[derive(Deserialize)]
pub struct EventsQuery {
    #[serde(rename = "lastEventId")]
    last_event_id: Option<u64>,
}

pub async fn handle_events(
    State(state): State<SseState>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .or(query.last_event_id);
    Sse::new(event_stream(state.hub, last_event_id))
        .keep_alive(KeepAlive::new().interval(state.heartbeat).text(""))
}

/// Router with `GET /v2/events` only (merged into the main router).
pub fn router(hub: Arc<EventHub>, heartbeat: Duration) -> axum::Router {
    axum::Router::new()
        .route("/v2/events", axum::routing::get(handle_events))
        .with_state(SseState { hub, heartbeat })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[test]
    fn replays_from_last_event_id() {
        let hub = EventHub::new(10, 16);
        for i in 0..5u8 {
            hub.publish("update", &[i]);
        }
        match hub.backlog(3) {
            Backlog::Replay(events) => assert_eq!(events.iter().map(|e| e.id).collect::<Vec<_>>(), vec![4, 5]),
            other => panic!("{other:?}"),
        }
        assert_eq!(hub.backlog(5), Backlog::Replay(vec![]));
    }

    #[test]
    fn asks_for_a_resync_when_events_are_gone() {
        let hub = EventHub::new(3, 16);
        for i in 0..10u8 {
            hub.publish("update", &[i]);
        }
        assert_eq!(hub.backlog(2), Backlog::Resync);
        // ids 8, 9, 10 are still there
        assert!(matches!(hub.backlog(7), Backlog::Replay(ref e) if e.len() == 3));
    }

    async fn read_until(body: &mut Body, needle: &str, max_frames: usize) -> String {
        let mut text = String::new();
        for _ in 0..max_frames {
            let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
                .await
                .expect("timed out waiting for an SSE frame")
                .expect("stream ended")
                .unwrap();
            if let Ok(data) = frame.into_data() {
                text.push_str(std::str::from_utf8(&data).unwrap());
            }
            if text.contains(needle) {
                break;
            }
        }
        text
    }

    #[tokio::test]
    async fn streams_live_events() {
        let hub = EventHub::new(100, 16);
        let app = router(hub.clone(), Duration::from_secs(60));
        let response = app.oneshot(Request::get("/v2/events").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        let mut body = response.into_body();
        let publisher = hub.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            publisher.publish("update", b"hello");
        });
        let text = read_until(&mut body, "data:", 10).await;
        assert!(text.contains("event: update"), "{text}");
        assert!(text.contains("id: 1"), "{text}");
        assert!(text.contains(&format!("data: {}", STANDARD.encode(b"hello"))), "{text}");
    }

    #[tokio::test]
    async fn resumes_with_last_event_id() {
        let hub = EventHub::new(100, 16);
        for i in 0..4u8 {
            hub.publish("update", &[i]);
        }
        let app = router(hub.clone(), Duration::from_secs(60));
        let request = Request::get("/v2/events").header("Last-Event-ID", "2").body(Body::empty()).unwrap();
        let mut body = app.oneshot(request).await.unwrap().into_body();
        let text = read_until(&mut body, "id: 4", 10).await;
        assert!(!text.contains("id: 2\n"), "{text}");
        assert!(text.contains("id: 3") && text.contains("id: 4"), "{text}");
    }

    #[tokio::test]
    async fn sends_resync_when_too_far_behind() {
        let hub = EventHub::new(2, 16);
        for i in 0..6u8 {
            hub.publish("update", &[i]);
        }
        let app = router(hub.clone(), Duration::from_secs(60));
        let uri = "/v2/events?lastEventId=1";
        let mut body = app.oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap().into_body();
        let text = read_until(&mut body, "resync", 10).await;
        assert!(text.contains("event: resync"), "{text}");
    }

    #[tokio::test]
    async fn sends_heartbeats() {
        let hub = EventHub::new(10, 16);
        let app = router(hub, Duration::from_millis(50));
        let mut body = app.oneshot(Request::get("/v2/events").body(Body::empty()).unwrap()).await.unwrap().into_body();
        let text = read_until(&mut body, ":", 5).await;
        assert!(text.starts_with(':'), "{text:?}");
    }

    #[tokio::test]
    async fn slow_clients_catch_up_from_the_buffer() {
        // broadcast channel of 2 messages, buffer of 100: a client that does not read lags behind
        // the channel, then gets the missed events from the buffer
        let hub = EventHub::new(100, 2);
        let app = router(hub.clone(), Duration::from_secs(60));
        let mut body = app.oneshot(Request::get("/v2/events").body(Body::empty()).unwrap()).await.unwrap().into_body();
        for i in 0..10u8 {
            hub.publish("update", &[i]);
        }
        let text = read_until(&mut body, "id: 10\n", 20).await;
        let ids: Vec<&str> = text.lines().filter(|l| l.starts_with("id: ")).collect();
        let expected: Vec<String> = (1..=10).map(|i| format!("id: {i}")).collect();
        assert_eq!(ids, expected, "{text}");
    }
}
