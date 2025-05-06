use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use crate::app::components::earth::three_js::{init_globe, is_three_available};

/// The Globe component acts as a wrapper for our Three.js implementation
/// This uses the Three.js renderer instead of WebGPU directly for better compatibility
#[component]
pub fn Globe() -> Element {
    let mut cube_loaded = use_signal(|| false);
    let canvas_id = "simple-cube-canvas";

    // We'll skip most of the complex lifecycle management and script loading checks
    rsx! {
        div {
            div { class: "status-indicator",
                if cube_loaded() {
                    "Three.js Cube is loaded and running."
                } else {
                    "Loading Three.js cube..."
                }
            }
            
            // Load Three.js script
            script { 
                src: "https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js" 
            }
            script {
                src: "/orbit-controls-global.js",
                r#type: "module"
            }
            canvas {
                id: "{canvas_id}",
                height: "500",
                width: "100%",
                style: "background-color: black;",
                onmounted: move |_element| {
                        let web_element = _element.as_web_event();
                        
                        if let Some(canvas) = web_element.dyn_into::<HtmlCanvasElement>().ok() {
                        spawn_local(async move {
                            TimeoutFuture::new(500).await;
                                                        
                            if is_three_available() {
                                if let Ok(_) = init_globe(&canvas) {
                                    cube_loaded.set(true);
                                }
                            }
                        });
                    }
                }
            }
        }
    }
}