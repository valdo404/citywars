mod animation_loop;
mod assets;
mod create_sphere;
mod endpoint;
mod runtime;
mod setup_webgpu;

use js_sys::Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::MessageEvent;

use endpoint::{read_message, Endpoint};
use runtime::Globe;

/// Starts the globe on `endpoint` and replays the `queued` `MessageEvent`s. `canvas` is the
/// `HTMLCanvasElement` to draw on in main-thread mode (a DOM element cannot be posted); in a worker
/// it is `undefined` and the `OffscreenCanvas` comes with the `Init` message.
#[wasm_bindgen]
pub fn run(endpoint: JsValue, queued: Array, canvas: JsValue) {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);

    let endpoint: Endpoint = endpoint.unchecked_into();
    let canvas = (!canvas.is_undefined() && !canvas.is_null()).then_some(canvas);
    let globe = Globe::new(endpoint.clone(), canvas);

    let handler_globe = globe.clone();
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Some((bytes, canvas)) = read_message(&event.data()) {
            Globe::on_message(&handler_globe, &bytes, canvas);
        }
    });
    endpoint.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    for event in queued.iter() {
        let data = js_sys::Reflect::get(&event, &"data".into()).unwrap_or(event);
        if let Some((bytes, canvas)) = read_message(&data) {
            Globe::on_message(&globe, &bytes, canvas);
        }
    }
}
