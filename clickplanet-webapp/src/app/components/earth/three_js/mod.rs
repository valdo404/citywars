mod bindings;
mod globe;
mod cube;

use dioxus::prelude::*;
use dioxus::document;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlCanvasElement;
use wasm_bindgen::JsCast;

pub use self::globe::init_globe;
pub use self::cube::init_cube;
pub use self::bindings::is_three_available;

#[component]
pub fn ThreeJsGlobe() -> Element {
    let canvas_id = "three-js-canvas";
    
    // We'll directly check scripts are loaded when the canvas is mounted
    // instead of relying on signals and effect hooks
    
    // Function to initialize Three.js with a canvas element directly
    fn init_three_js_with_canvas(canvas: &HtmlCanvasElement) -> Result<(), String> {
        // Ensure Three.js is available before trying to use it
        if !is_three_available() {
            return Err("Three.js is not available".to_string());
        }
        
        // Initialize cube with the canvas directly instead of looking it up by ID
        match init_cube(canvas) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to initialize Three.js: {:?}", e)),
        }
    }

    rsx! {
        // Load Three.js as a regular script (not a module) to make it globally available
        document::Script {
            src: "https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js"
        }
        
        // Load OrbitControls after Three.js
        // document::Script {
        //    src: "https://cdn.jsdelivr.net/npm/three@0.160.0/examples/js/controls/OrbitControls.js"
        //}
        
        div {
            id: "three-js-container",
            style: "width: 100%; height: 100vh; overflow: hidden;",
            
            // Canvas for Three.js rendering - using onmounted to get direct access
            canvas {
                id: "{canvas_id}",
                width: "100%",
                height: "100%",
                style: "display: block; background-color: #000;",
                // Use onmounted to get direct access to the canvas element
                onmounted: move |_element| {
                    // In Dioxus, we need to use the as_web_event method to get access to the web_sys::Element
                        use dioxus::web::WebEventExt;
                        let web_element = _element.as_web_event();
                        
                        // Now cast to HtmlCanvasElement
                        if let Some(canvas) = web_element.dyn_into::<HtmlCanvasElement>().ok() {
                        // Initialize Three.js with the canvas element directly
                        spawn_local(async move {
                            // Wait a brief moment to ensure Three.js is loaded
                            gloo_timers::future::TimeoutFuture::new(500).await;
                            
                            // Initialize cube with the canvas
                            if let Err(err) = init_three_js_with_canvas(&canvas) {
                                // Handle error silently
                                web_sys::console::log_1(&format!("Failed to initialize Three.js: {}", err).into());
                            }
                        });
                    }
                }
            }
        }
    }
}
