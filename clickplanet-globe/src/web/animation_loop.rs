//! Frame loop: `requestAnimationFrame` on the main thread and in dedicated workers that have it
//! (Chrome, Firefox, Safari 16.4+), `setTimeout` at ~60 Hz otherwise.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut(JsValue)>>>>;

/// Calls `frame(timestamp_ms)` once per display frame until it returns `false`.
pub fn start_animation(frame: impl FnMut(f64) -> bool + 'static) {
    let global = js_sys::global();
    let raf = Reflect::get(&global, &"requestAnimationFrame".into())
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok());
    let set_timeout = Reflect::get(&global, &"setTimeout".into())
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
        .expect("setTimeout is always available");
    let performance = Reflect::get(&global, &"performance".into())
        .ok()
        .and_then(|p| p.dyn_into::<web_sys::Performance>().ok());

    let frame = Rc::new(RefCell::new(frame));
    let callback: FrameCallback = Rc::new(RefCell::new(None));
    let callback_inner = callback.clone();
    let global_inner = global.clone();
    *callback.borrow_mut() = Some(Closure::new(move |timestamp: JsValue| {
        let now = timestamp
            .as_f64()
            .or_else(|| performance.as_ref().map(|p| p.now()))
            .unwrap_or(0.0);
        if !(frame.borrow_mut())(now) {
            callback_inner.borrow_mut().take();
            return;
        }
        if let Some(cb) = callback_inner.borrow().as_ref() {
            schedule(&global_inner, raf.as_ref(), &set_timeout, cb);
        }
    }));
    let raf = Reflect::get(&global, &"requestAnimationFrame".into())
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok());
    let set_timeout: Function = Reflect::get(&global, &"setTimeout".into()).unwrap().unchecked_into();
    if let Some(cb) = callback.borrow().as_ref() {
        schedule(&global, raf.as_ref(), &set_timeout, cb);
    };
}

fn schedule(global: &JsValue, raf: Option<&Function>, set_timeout: &Function, cb: &Closure<dyn FnMut(JsValue)>) {
    let f: &Function = cb.as_ref().unchecked_ref();
    let scheduled = match raf {
        Some(raf) => raf.call1(global, f),
        None => set_timeout.call2(global, f, &JsValue::from(16)),
    };
    if let Err(e) = scheduled {
        log::error!("cannot schedule the next frame: {e:?}");
    }
}
