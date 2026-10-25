//! Interface side of the globe transport (see `clickplanet-globe-protocol`).
//!
//! The globe is a separate wasm bundle served from `globe/`. By default it runs in a Web Worker
//! and draws on an `OffscreenCanvas` transferred from the page's canvas, so rendering never blocks
//! the Dioxus interface. When that is not possible (no `transferControlToOffscreen`, worker or GPU
//! initialisation failure, or `?globe=main`) it runs on the main thread, connected through a
//! `MessageChannel`, with the same messages.

use std::cell::RefCell;
use std::rc::Rc;

use clickplanet_globe_protocol::{decode, encode, GlobeToUi, InitParams, UiToGlobe};
use futures::channel::mpsc::UnboundedSender;
use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{HtmlCanvasElement, MessageChannel, MessageEvent, MessagePort, Worker, WorkerOptions, WorkerType};

#[wasm_bindgen(inline_js = "export function import_module(url) { return import(url); }")]
extern "C" {
    fn import_module(url: &str) -> js_sys::Promise;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobeMode {
    Worker,
    MainThread,
}

impl GlobeMode {
    pub fn label(self) -> &'static str {
        match self {
            GlobeMode::Worker => "worker + OffscreenCanvas",
            GlobeMode::MainThread => "main thread",
        }
    }
}

/// What the interface receives from the globe.
#[derive(Debug, Clone)]
pub enum GlobeEvent {
    Message(GlobeToUi),
    /// The worker could not start; the interface should retry on the main thread.
    WorkerFailed(String),
}

enum Endpoint {
    Worker(Worker),
    Port(MessagePort),
}

impl Endpoint {
    fn post(&self, message: &JsValue, transfer: &Array) -> Result<(), JsValue> {
        match self {
            Endpoint::Worker(w) => w.post_message_with_transfer(message, transfer),
            Endpoint::Port(p) => p.post_message_with_transferable(message, transfer),
        }
    }
}

struct Inner {
    endpoint: Option<Endpoint>,
    mode: GlobeMode,
    events: UnboundedSender<GlobeEvent>,
    ready: bool,
    /// Bytes posted since start (for the transfer cost measurements).
    bytes_sent: u64,
    encode_post_ms: f64,
}

#[derive(Clone)]
pub struct GlobeClient {
    inner: Rc<RefCell<Inner>>,
}

impl PartialEq for GlobeClient {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Absolute URL of a file of the globe bundle, served in `globe/` next to the page.
fn globe_url(file: &str) -> String {
    let base = web_sys::window().and_then(|w| w.location().href().ok()).unwrap_or_default();
    web_sys::Url::new_with_base(&format!("globe/{file}"), &base)
        .map(|u| u.href())
        .unwrap_or_else(|_| format!("./globe/{file}"))
}

fn now_ms() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or(0.0)
}

fn decode_event(event: &MessageEvent) -> Option<GlobeToUi> {
    let payload = Reflect::get(&event.data(), &"payload".into()).ok()?;
    let bytes = Uint8Array::new(&payload).to_vec();
    match decode::<GlobeToUi>(&bytes) {
        Ok(m) => Some(m),
        Err(e) => {
            log::error!("{e}");
            None
        }
    }
}

/// Worker + OffscreenCanvas needs `transferControlToOffscreen` (Chrome 69+, Firefox 105+,
/// Safari 16.4+) and module workers.
pub fn offscreen_supported(canvas: &HtmlCanvasElement) -> bool {
    Reflect::has(canvas, &"transferControlToOffscreen".into()).unwrap_or(false)
}

impl GlobeClient {
    pub fn new(mode: GlobeMode, events: UnboundedSender<GlobeEvent>) -> Self {
        GlobeClient {
            inner: Rc::new(RefCell::new(Inner {
                endpoint: None,
                mode,
                events,
                ready: false,
                bytes_sent: 0,
                encode_post_ms: 0.0,
            })),
        }
    }

    pub fn mode(&self) -> GlobeMode {
        self.inner.borrow().mode
    }

    pub fn is_started(&self) -> bool {
        self.inner.borrow().endpoint.is_some()
    }

    pub fn transfer_stats(&self) -> (u64, f64) {
        let inner = self.inner.borrow();
        (inner.bytes_sent, inner.encode_post_ms)
    }

    pub fn set_ready(&self) {
        self.inner.borrow_mut().ready = true;
    }

    pub fn is_ready(&self) -> bool {
        self.inner.borrow().ready
    }

    /// Sends a message, transferring its payload. Messages sent before `start` are dropped: the
    /// interface sends its full state after starting the globe.
    pub fn send(&self, message: &UiToGlobe) {
        self.post(message, None);
    }

    fn post(&self, message: &UiToGlobe, canvas: Option<&JsValue>) {
        let start = now_ms();
        let mut inner = self.inner.borrow_mut();
        let Some(endpoint) = inner.endpoint.as_ref() else {
            return;
        };
        let bytes = encode(message);
        let payload = Uint8Array::new_with_length(bytes.len() as u32);
        payload.copy_from(&bytes);
        let buffer = payload.buffer();
        let envelope = Object::new();
        let _ = Reflect::set(&envelope, &"payload".into(), &buffer);
        let transfer = Array::of1(&buffer);
        if let Some(canvas) = canvas {
            let _ = Reflect::set(&envelope, &"canvas".into(), canvas);
            transfer.push(canvas);
        }
        if let Err(e) = endpoint.post(&envelope, &transfer) {
            log::error!("cannot post to the globe: {e:?}");
        }
        inner.bytes_sent += bytes.len() as u64;
        inner.encode_post_ms += now_ms() - start;
    }

    /// Starts the globe on `canvas` (once per canvas element).
    pub fn start(&self, canvas: HtmlCanvasElement, init: InitParams) {
        let mode = self.mode();
        match mode {
            GlobeMode::Worker => {
                if let Err(e) = self.start_worker(&canvas, init) {
                    let _ = self.inner.borrow().events.unbounded_send(GlobeEvent::WorkerFailed(e));
                }
            }
            GlobeMode::MainThread => self.start_main_thread(canvas, init),
        }
    }

    fn start_worker(&self, canvas: &HtmlCanvasElement, init: InitParams) -> Result<(), String> {
        let offscreen = canvas
            .transfer_control_to_offscreen()
            .map_err(|e| format!("transferControlToOffscreen: {e:?}"))?;
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker = Worker::new_with_options(&globe_url("worker.js"), &options)
            .map_err(|e| format!("cannot start the globe worker: {e:?}"))?;

        let events = self.inner.borrow().events.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(m) = decode_event(&event) {
                let _ = events.unbounded_send(GlobeEvent::Message(m));
            }
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        let events = self.inner.borrow().events.clone();
        let on_error = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |event: web_sys::ErrorEvent| {
            let _ = events.unbounded_send(GlobeEvent::WorkerFailed(format!("globe worker error: {}", event.message())));
        });
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        self.inner.borrow_mut().endpoint = Some(Endpoint::Worker(worker));
        self.post(&UiToGlobe::Init(init), Some(offscreen.as_ref()));
        Ok(())
    }

    fn start_main_thread(&self, canvas: HtmlCanvasElement, init: InitParams) {
        let channel = match MessageChannel::new() {
            Ok(c) => c,
            Err(e) => return log::error!("MessageChannel: {e:?}"),
        };
        let ui_port = channel.port1();
        let globe_port = channel.port2();

        let events = self.inner.borrow().events.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(m) = decode_event(&event) {
                let _ = events.unbounded_send(GlobeEvent::Message(m));
            }
        });
        ui_port.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        self.inner.borrow_mut().endpoint = Some(Endpoint::Port(ui_port));
        // queued in the port until the globe module listens
        self.post(&UiToGlobe::Init(init), None);

        let events = self.inner.borrow().events.clone();
        spawn_local(async move {
            if let Err(e) = load_main_thread_globe(globe_port, canvas).await {
                let _ = events.unbounded_send(GlobeEvent::Message(GlobeToUi::Error {
                    message: format!("cannot load the globe: {e:?}"),
                    fatal: true,
                }));
            }
        });
    }

    /// Stops the current globe (worker) so that it can be restarted in another mode.
    pub fn restart_in(&self, mode: GlobeMode) {
        let mut inner = self.inner.borrow_mut();
        if let Some(Endpoint::Worker(w)) = inner.endpoint.take() {
            w.terminate();
        }
        inner.endpoint = None;
        inner.mode = mode;
        inner.ready = false;
    }
}

async fn load_main_thread_globe(port: MessagePort, canvas: HtmlCanvasElement) -> Result<(), JsValue> {
    let module = JsFuture::from(import_module(&globe_url("clickplanet_globe.js"))).await?;
    let init: js_sys::Function = Reflect::get(&module, &"default".into())?.dyn_into()?;
    JsFuture::from(js_sys::Promise::from(init.call0(&JsValue::NULL)?)).await?;
    let run: js_sys::Function = Reflect::get(&module, &"run".into())?.dyn_into()?;
    run.call3(&JsValue::NULL, &port, &Array::new(), &canvas)?;
    Ok(())
}
