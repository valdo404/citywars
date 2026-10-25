//! The transport end used by the globe: the `DedicatedWorkerGlobalScope` in a worker, or a
//! `MessagePort` on the main thread. Both have `postMessage(message, transfer)` and `onmessage`.

use clickplanet_globe_protocol::{encode, GlobeToUi};
use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[derive(Clone)]
    pub type Endpoint;

    #[wasm_bindgen(method, structural, js_name = postMessage, catch)]
    fn post_message_with_transfer(this: &Endpoint, message: &JsValue, transfer: &Array) -> Result<(), JsValue>;

    #[wasm_bindgen(method, structural, setter = onmessage)]
    pub fn set_onmessage(this: &Endpoint, handler: Option<&js_sys::Function>);
}

impl Endpoint {
    /// Sends a message, transferring its payload buffer.
    pub fn send(&self, message: &GlobeToUi) {
        let bytes = encode(message);
        let payload = Uint8Array::new_with_length(bytes.len() as u32);
        payload.copy_from(&bytes);
        let buffer = payload.buffer();
        let envelope = Object::new();
        let _ = Reflect::set(&envelope, &"payload".into(), &buffer);
        if let Err(e) = self.post_message_with_transfer(&envelope, &Array::of1(&buffer)) {
            log::error!("cannot post a globe message: {e:?}");
        }
    }
}

/// Extracts `(payload bytes, canvas)` from a received message object.
pub fn read_message(data: &JsValue) -> Option<(Vec<u8>, Option<JsValue>)> {
    let payload = Reflect::get(data, &"payload".into()).ok()?;
    if payload.is_undefined() {
        return None;
    }
    let bytes = Uint8Array::new(&payload).to_vec();
    let canvas = Reflect::get(data, &"canvas".into()).ok().filter(|c| !c.is_undefined() && !c.is_null());
    Some((bytes, canvas))
}
