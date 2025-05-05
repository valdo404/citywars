mod bindings;
mod globe;
mod cube;

use dioxus::prelude::*;
use dioxus::document;
use wasm_bindgen_futures::spawn_local;

pub use self::globe::init_globe;
pub use self::cube::init_cube;
pub use self::bindings::is_three_available;

#[component]
pub fn ThreeJsGlobe() -> Element {
    let canvas_id = "three-js-canvas";
    let mut scripts_loaded = use_signal(|| false);
    
    // Initialize Three.js when scripts are loaded
    use_effect(move || {
        if scripts_loaded() {
            // Initialize Three.js with the canvas element
            spawn_local(async move {
                if let Err(_) = init_globe(canvas_id) {
                    // Silently handle error
                }
            });
        }
        // No cleanup needed
        ()
    });
    
    // Set a timer to mark scripts as loaded after a delay and check THREE availability
    // This is a workaround since onload is not directly available
    use_effect(move || {
        use wasm_bindgen::prelude::*;
        
        // A timeout ensures scripts are loaded before we attempt to use them
        let timeout_ms = 500;
        
        let handle = gloo_timers::callback::Timeout::new(timeout_ms, move || {
            // Check if Three.js is available
            let three_available = is_three_available();
            
            if three_available {
                scripts_loaded.set(true);
            }
        });
        
        (|| { drop(handle); })()
    });

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
