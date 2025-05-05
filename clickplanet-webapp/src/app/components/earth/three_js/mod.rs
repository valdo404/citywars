mod bindings;
mod globe;

use dioxus::prelude::*;
use dioxus::document;
use wasm_bindgen_futures::spawn_local;
use log::error;

pub use self::globe::init_globe;

#[component]
pub fn ThreeJsGlobe() -> Element {
    let canvas_id = "three-js-canvas";
    let mut scripts_loaded = use_signal(|| false);
    
    // Initialize Three.js when scripts are loaded
    use_effect(move || {
        if scripts_loaded() {
            // Initialize Three.js with the canvas element
            spawn_local(async move {
                if let Err(e) = init_globe(canvas_id) {
                    error!("Failed to initialize Three.js: {:?}", e);
                }
            });
        }
        ()
    });
    
    // Set a timer to mark scripts as loaded after a delay
    // This is a workaround since onload is not directly available
    use_effect(move || {
        // A timeout ensures scripts are loaded before we attempt to use them
        let timeout_ms = 500;
        let handle = gloo_timers::callback::Timeout::new(timeout_ms, move || {
            scripts_loaded.set(true);
        });
        
        (|| { drop(handle); })()
    });

    rsx! {
        // Load Three.js and OrbitControls in head
        document::Script {
            src: asset!("public/js/three.module.js"),
            r#type: "module"
        }
        
        document::Script {
            src: asset!("public/js/OrbitControls.js"),
            r#type: "module"
        }
        
        div {
            id: "three-js-container",
            style: "width: 100%; height: 100vh; overflow: hidden;",
            
            // Canvas for Three.js rendering
            canvas {
                id: "{canvas_id}",
                width: "100%",
                height: "100%",
                style: "display: block; background-color: #000;"
            }
        }
    }
}
