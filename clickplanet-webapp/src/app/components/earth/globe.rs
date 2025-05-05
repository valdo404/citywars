use dioxus::prelude::*;
use gloo_timers::callback::Timeout;

/// The Globe component acts as a wrapper for our Three.js implementation
/// This uses the Three.js renderer instead of WebGPU directly for better compatibility
#[component]
pub fn Globe() -> Element {
    let mut cube_loaded = use_signal(|| false);
    let mut scripts_loaded = use_signal(|| false);
    let canvas_id = "simple-cube-canvas";

    // Check if scripts are loaded and initialize Three.js
    use_effect(move || {
        if scripts_loaded() {
            use wasm_bindgen_futures::spawn_local;
            use crate::app::components::earth::three_js::{init_cube, is_three_available};
            
            spawn_local(async move {
                // Additional verification that Three.js is available
                if is_three_available() {
                    if let Err(_) = init_cube(canvas_id) {
                        // Silently handle error
                    } else {
                        cube_loaded.set(true);
                    }
                }
            });
        }

        // No cleanup needed
        ()
    });
    
    // Set a timer to check if scripts are loaded
    use_effect(move || {
        use wasm_bindgen::prelude::*;
        
        // Wait for scripts to load
        let timeout_ms = 1000; // Give scripts enough time to load
        
        let handle = Timeout::new(timeout_ms, move || {
            // Check if Three.js is available after timeout
            use crate::app::components::earth::three_js::is_three_available;
            
            let three_available = is_three_available();
            if three_available {
                scripts_loaded.set(true);
            }
        });
        
        // Cleanup function
        (|| { drop(handle); })()
    });

    // Simplified script loading - load Three.js directly in the RSX
    // Then use the hook to check if it's available and initialize the cube
    
    // Check if scripts are loaded and initialize Three.js - this is called immediately and after scripts load
    use_effect(move || {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen_futures::spawn_local;
        use crate::app::components::earth::three_js::{init_cube, is_three_available};
        
        // Check if Three.js is available
        spawn_local(async move {
            // Direct check - wait a little to ensure scripts have a chance to load
            gloo_timers::future::TimeoutFuture::new(500).await;
            
            let three_available = is_three_available();
            
            if three_available {
                scripts_loaded.set(true);
                
                // Now initialize the cube
                if let Err(_) = init_cube(canvas_id) {
                    // Silently handle error
                } else {
                    cube_loaded.set(true);
                }
            }
        });
        
        // No cleanup needed
        ()
    });

    rsx! {
        // Load Three.js as a regular script (not a module) to make it globally available
        document::Script {
            src: "https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.min.js"
        }
        
        // Load OrbitControls after Three.js
        document::Script {
            src: "https://cdn.jsdelivr.net/npm/three@0.160.0/examples/js/controls/OrbitControls.js"
        }
        
        div {
            class: "earth-container",
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