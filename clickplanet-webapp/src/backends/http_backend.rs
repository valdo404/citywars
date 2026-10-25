//! Backend talking to a citywars (or Clickplanet) server, like the original `HTTPBackend`:
//!
//! - RPC calls are `POST`/`GET` with a JSON body `{"data": [protobuf bytes]}` and a JSON response
//!   `{"data": "<base64 protobuf>"}`, retried up to 5 times with a timeout;
//! - live updates come from Server-Sent Events on `/v2/events` (`EventSource`, base64
//!   `UpdateNotification`, resumed with `Last-Event-ID` by the browser) or, with
//!   [`UpdatesTransport::WebSocket`], from the `/v2/ws/listen` websocket like the original (needed
//!   for servers without SSE, such as clickplanet.lol). They are delivered in batches every
//!   `batch_update_duration_ms`. Unlike the original, the websocket reconnects when it closes.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use clickplanet_proto::clicks::{BatchRequest, ClickRequest, LeaderboardResponse, OwnershipState, UpdateNotification};
use gloo_net::http::Request;
use gloo_timers::callback::{Interval, Timeout};
use js_sys::{ArrayBuffer, Uint8Array};
use prost::Message;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{EventSource, MessageEvent, WebSocket};

use crate::backends::backend::{Update, UpdatesCallback};

/// How live updates reach the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatesTransport {
    Sse,
    WebSocket,
}

type ResyncCallbacks = Rc<RefCell<Vec<Rc<dyn Fn()>>>>;

pub struct HttpBackend {
    base_url: String,
    transport: UpdatesTransport,
    resync_callbacks: ResyncCallbacks,
    timeout_ms: u32,
    batch_update_duration_ms: u32,
    pending: Rc<RefCell<Vec<Update>>>,
    callbacks: Rc<RefCell<Vec<UpdatesCallback>>>,
    listening: Cell<bool>,
}

fn update_from_notification(n: UpdateNotification) -> Update {
    Update {
        tile: n.tile_id.max(0) as u32,
        previous_country: (!n.previous_country_id.is_empty()).then_some(n.previous_country_id),
        new_country: n.country_id,
    }
}

impl HttpBackend {
    pub fn new(base_url: String, transport: UpdatesTransport, timeout_ms: u32, batch_update_duration_ms: u32) -> Self {
        HttpBackend {
            base_url,
            transport,
            resync_callbacks: Rc::new(RefCell::new(Vec::new())),
            timeout_ms,
            batch_update_duration_ms,
            pending: Rc::new(RefCell::new(Vec::new())),
            callbacks: Rc::new(RefCell::new(Vec::new())),
            listening: Cell::new(false),
        }
    }

    async fn fetch_once(&self, verb: &str, url: &str, body: Option<&[u8]>) -> Result<Option<Vec<u8>>, String> {
        let signal = web_sys::AbortSignal::timeout_with_u32(self.timeout_ms);
        let builder = match verb {
            "POST" => Request::post(url),
            _ => Request::get(url),
        }
        .header("Content-Type", "application/json")
        .abort_signal(Some(&signal));
        let request = match body {
            Some(bytes) => builder
                .json(&serde_json::json!({ "data": bytes }))
                .map_err(|e| e.to_string())?,
            None => builder.build().map_err(|e| e.to_string())?,
        };
        let response = request.send().await.map_err(|e| e.to_string())?;
        if !response.ok() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("{verb} {url}: {} {text}", response.status()));
        }
        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        match json.get("data").and_then(|v| v.as_str()) {
            Some(data) if !data.is_empty() => STANDARD.decode(data).map(Some).map_err(|e| e.to_string()),
            _ => Ok(None),
        }
    }

    /// Same retry policy as the original client: up to 5 attempts.
    async fn fetch(&self, verb: &str, path: &str, body: Option<&[u8]>) -> Result<Option<Vec<u8>>, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut last_error = String::new();
        for attempt in 0..5 {
            match self.fetch_once(verb, &url, body).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    log::warn!("{attempt} failed to fetch {verb} {path}: {e}");
                    last_error = e;
                }
            }
        }
        Err(format!("Failed to fetch {verb} {path}: {last_error}"))
    }

    pub async fn click_tile(&self, tile_id: u32, country_id: &str) -> Result<(), String> {
        let request = ClickRequest {
            tile_id: tile_id as i32,
            country_id: country_id.to_string(),
        };
        self.fetch("POST", "/v2/rpc/click", Some(&request.encode_to_vec())).await?;
        Ok(())
    }

    pub async fn get_current_ownerships_by_batch(
        &self,
        batch_size: u32,
        max_index: u32,
        callback: &mut dyn FnMut(Vec<(u32, String)>),
    ) -> Result<(), String> {
        let mut start = 1;
        while start < max_index {
            let request = BatchRequest {
                start_tile_id: start as i32,
                end_tile_id: (start + batch_size) as i32,
            };
            let data = self
                .fetch("POST", "/v2/rpc/ownerships-by-batch", Some(&request.encode_to_vec()))
                .await?
                .unwrap_or_default();
            let state = OwnershipState::decode(data.as_slice()).map_err(|e| e.to_string())?;
            callback(
                state
                    .ownerships
                    .into_iter()
                    .map(|o| (o.tile_id, o.country_id))
                    .collect(),
            );
            start += batch_size;
        }
        Ok(())
    }

    pub async fn leaderboard(&self) -> Result<Vec<(String, u32)>, String> {
        let data = self.fetch("GET", "/v2/rpc/leaderboard", None).await?.unwrap_or_default();
        let response = LeaderboardResponse::decode(data.as_slice()).map_err(|e| e.to_string())?;
        Ok(response.entries.into_iter().map(|e| (e.country_id, e.score)).collect())
    }

    pub fn listen_for_updates_batch(&self, callback: UpdatesCallback) {
        self.callbacks.borrow_mut().push(callback);
        if self.listening.replace(true) {
            return;
        }

        let pending = self.pending.clone();
        let callbacks = self.callbacks.clone();
        Interval::new(self.batch_update_duration_ms, move || {
            let updates = std::mem::take(&mut *pending.borrow_mut());
            if !updates.is_empty() {
                for callback in callbacks.borrow().iter() {
                    callback(updates.clone());
                }
            }
        })
        .forget();

        match self.transport {
            UpdatesTransport::WebSocket => {
                let ws_url = format!("{}/v2/ws/listen", self.base_url.replacen("http", "ws", 1));
                connect_websocket(Rc::new(ws_url), self.pending.clone(), 1000);
            }
            UpdatesTransport::Sse => {
                let url = format!("{}/v2/events", self.base_url);
                connect_event_source(&url, self.pending.clone(), self.resync_callbacks.clone());
            }
        }
    }

    /// Called when the server cannot replay the missed SSE events: the ownerships must be
    /// reloaded.
    pub fn on_resync(&self, callback: Rc<dyn Fn()>) {
        self.resync_callbacks.borrow_mut().push(callback);
    }
}

/// Subscribes to the SSE stream. `EventSource` reconnects by itself and sends `Last-Event-ID`,
/// so the server replays what was missed (or sends `resync`).
fn connect_event_source(url: &str, pending: Rc<RefCell<Vec<Update>>>, resync: ResyncCallbacks) {
    let source = match EventSource::new(url) {
        Ok(s) => s,
        Err(e) => return log::error!("EventSource {url}: {e:?}"),
    };
    let on_update = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let Some(data) = event.data().as_string() else { return };
        match STANDARD.decode(data.trim()).map_err(|e| e.to_string()).and_then(|bytes| {
            UpdateNotification::decode(bytes.as_slice()).map_err(|e| e.to_string())
        }) {
            Ok(n) => pending.borrow_mut().push(update_from_notification(n)),
            Err(e) => log::warn!("bad SSE update: {e}"),
        }
    });
    let _ = source.add_event_listener_with_callback("update", on_update.as_ref().unchecked_ref());
    on_update.forget();

    let on_resync = Closure::<dyn FnMut(MessageEvent)>::new(move |_event: MessageEvent| {
        log::warn!("SSE resync requested: reloading the ownerships");
        for callback in resync.borrow().iter() {
            callback();
        }
    });
    let _ = source.add_event_listener_with_callback("resync", on_resync.as_ref().unchecked_ref());
    on_resync.forget();

    let url = url.to_string();
    let on_open = Closure::<dyn FnMut()>::new(move || log::info!("SSE connected to {url}"));
    source.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();
    // keep the EventSource alive for the page's lifetime
    std::mem::forget(source);
}

/// Opens the update websocket and reconnects (with backoff up to 10 s) when it closes.
fn connect_websocket(url: Rc<String>, pending: Rc<RefCell<Vec<Update>>>, retry_ms: u32) {
    let websocket = match WebSocket::new(&url) {
        Ok(ws) => ws,
        Err(e) => {
            log::error!("websocket {url}: {e:?}");
            schedule_reconnect(url, pending, retry_ms);
            return;
        }
    };
    websocket.set_binary_type(web_sys::BinaryType::Arraybuffer);

    let on_message = {
        let pending = pending.clone();
        Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Ok(buffer) = event.data().dyn_into::<ArrayBuffer>() {
                let bytes = Uint8Array::new(&buffer).to_vec();
                match UpdateNotification::decode(bytes.as_slice()) {
                    Ok(n) => pending.borrow_mut().push(update_from_notification(n)),
                    Err(e) => log::warn!("bad update notification: {e}"),
                }
            }
        })
    };
    websocket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    let on_open = {
        let url = url.clone();
        Closure::<dyn FnMut()>::new(move || log::info!("Websocket connected to {url}"))
    };
    websocket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();

    let on_close = Closure::once_into_js(move || {
        log::warn!("Websocket closed, reconnecting in {retry_ms} ms");
        schedule_reconnect(url, pending, retry_ms);
    });
    websocket.set_onclose(Some(on_close.unchecked_ref()));
}

fn schedule_reconnect(url: Rc<String>, pending: Rc<RefCell<Vec<Update>>>, retry_ms: u32) {
    Timeout::new(retry_ms, move || connect_websocket(url, pending, (retry_ms * 2).min(10_000))).forget();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_previous_country_means_none() {
        let u = update_from_notification(UpdateNotification {
            tile_id: 3,
            country_id: "fr".into(),
            previous_country_id: String::new(),
        });
        assert_eq!(u.previous_country, None);
    }
}
