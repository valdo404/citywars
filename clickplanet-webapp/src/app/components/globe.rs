use dioxus::prelude::*;
use log::{debug, error, info};
use web_sys::HtmlCanvasElement;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures;
use crate::app::components::setup_webgpu::{setup_webgpu, use_animation_state};

#[component]
pub fn globe() -> Element {
    let mut canvas_ref = use_signal(|| None::<HtmlCanvasElement>);
    
    // Create animation state signal that will be used for rotation
    let rotation = use_animation_state();
    
    use_effect(move || {
        to_owned![canvas_ref, rotation];
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(canvas) = canvas_ref.read().clone() {
                info!("Canvas found - initializing WebGPU Earth rendering");
                // Hand off to setup_webgpu for real initialization with animation signal
                setup_webgpu(canvas, rotation).await;
            } else {
                error!("No canvas found for globe rendering");
            }
        });
        
        ()
    });
    
    rsx! {
        div {
            style: "width: 100%; height: 100%; overflow: hidden;",
            canvas {
                id: "globe-canvas",
                style: "width: 100%; height: 100%; display: block; background-color: #001020;",
                onmounted: move |_| {
                    debug!("Canvas mounted");
                    let window = web_sys::window().expect("no global window");
                    let document = window.document().expect("no document on window");
                    if let Some(element) = document.get_element_by_id("globe-canvas") {
                        if let Some(canvas) = element.dyn_ref::<HtmlCanvasElement>() {
                            debug!("Canvas element captured");
                            canvas_ref.set(Some(canvas.clone()));
                        } else {
                            error!("Failed to convert to HtmlCanvasElement");
                        }
                    } else {
                        error!("Could not find canvas element with ID 'globe-canvas'");
                    }
                }
            }
        }
    }
}