//! citywars globe: a separate WebAssembly bundle that renders the Earth and the ~258k tiles with
//! wgpu (WebGPU, WebGL2 fallback) and talks to the Dioxus interface only through
//! [`clickplanet_globe_protocol`] messages.
//!
//! JavaScript entry point: [`run`]`(endpoint, queued)`, where `endpoint` is the worker global scope
//! (globe in a Web Worker, drawing on an `OffscreenCanvas`) or a `MessagePort` (globe on the main
//! thread, drawing on an `HTMLCanvasElement`), and `queued` the messages received before the module
//! was ready.

pub mod core;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::run;
