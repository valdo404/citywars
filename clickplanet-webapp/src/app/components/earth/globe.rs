use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use crate::app::components::earth::three_js::{init_cube, is_three_available};

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
            script { src: "https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js" }
            
            // Canvas for the 3D cube - using onmounted to get direct access
            canvas {
                id: "{canvas_id}",
                height: "500",
                width: "100%",
                style: "background-color: black;",
                onmounted: move |_element| {
                    // In Dioxus, we need to use the as_web_event method to get access to the web_sys::Element
                        // WebEventExt is imported at the top of the file
                        let web_element = _element.as_web_event();
                        
                        // Now cast to HtmlCanvasElement
                        if let Some(canvas) = web_element.dyn_into::<HtmlCanvasElement>().ok() {
                        // Initialize Three.js with the canvas element directly
                        spawn_local(async move {
                            // Use gloo_timers to wait for Three.js to load
                            TimeoutFuture::new(500).await;
                            
                            // Initialize the cube with canvas directly
                            // three_js functions are imported at the top of the file
                            
                            if is_three_available() {
                                if let Ok(_) = init_cube(&canvas) {
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