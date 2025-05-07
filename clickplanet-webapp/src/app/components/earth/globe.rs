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
    let cube_loaded = use_signal(|| false);
    let canvas_id = "simple-cube-canvas";

    rsx! {
        script {
            r#type: "module",
            {r#"
                import * as THREE from "three";
                import { OrbitControls } from "three/addons/controls/OrbitControls.js";
                import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";

                window.THREE = THREE;
                window.OrbitControls = OrbitControls;
                window.GLTFLoader = GLTFLoader;
                
                // Direct JS console logging
                console.log('THREE.js imports loaded. THREE version:', THREE.REVISION);
                console.log('OrbitControls available:', typeof OrbitControls);
                console.log('GLTFLoader available:', typeof GLTFLoader);
                
                // Make sure to expose environment variable to JavaScript
                window.CITYWARS_STATIC_SITE = "https://storage.googleapis.com/lv-project-313715-clickwars-static/static";
                // Add texture loading test with detailed diagnostics
                const textureUrl = window.CITYWARS_STATIC_SITE + '/earth/3_no_ice_clouds_16k.jpg';
                console.log('Testing texture URL:', textureUrl);
                
                // Check CORS environment
                console.log('CORS Origin:', window.location.origin);
                console.log('CORS Headers available in browser:', typeof Headers);
            "#}
        }

        div {
            height: "100%",
            width: "100%",
            canvas {
                id: "{canvas_id}",
                height: "100vh",
                width: "100%",
                style: "background-color: black;",
                onmounted: move |el| {
                    // Direct JS logging to check canvas element
                    let js_code = format!(r#"console.log('Canvas onmounted triggered for id: {}');"#, canvas_id);
                    let _ = js_sys::eval(&js_code);
                    
                    let mut cube_loaded_clone = cube_loaded.clone();
                    let web_element = el.as_web_event();
                    
                    if let Some(canvas) = web_element.dyn_into::<HtmlCanvasElement>().ok() {
                        // Log canvas dimensions
                        let js_code = format!(r#"console.log('Canvas dimensions:', document.getElementById('{}').clientWidth, 'x', document.getElementById('{}').clientHeight);"#, canvas_id, canvas_id);
                        let _ = js_sys::eval(&js_code);
                        
                        spawn_local(async move {            
                            // Wait for scripts to load - THREE is loaded asynchronously 
                            // so we need to wait before checking availability
                            gloo_console::log!("Waiting for THREE to load...");
                            TimeoutFuture::new(1000).await; // Wait 1 second for scripts to load
                            
                            // Now check THREE availability after waiting
                            gloo_console::log!("Checking THREE availability");
                            
                            if is_three_available() {
                                gloo_console::log!("THREE is available, initializing globe");
                                if let Ok(_) = init_globe(&canvas) {
                                    cube_loaded_clone.set(true);
                                    gloo_console::log!("Globe initialization successful");
                                } else {
                                    gloo_console::log!("Globe initialization FAILED");
                                }
                            } else {
                                gloo_console::log!("THREE is NOT available after waiting");
                            }
                        });
                    } else {
                        gloo_console::log!("Failed to get HtmlCanvasElement");
                    }
                }
            }
        }
}
}