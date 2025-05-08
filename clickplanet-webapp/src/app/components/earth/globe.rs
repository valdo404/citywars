use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use crate::app::components::earth::three_js::{init_globe, is_three_available};

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
                    let mut cube_loaded_clone = cube_loaded.clone();
                    let web_element = el.as_web_event();
                    
                    if let Some(canvas) = web_element.dyn_into::<HtmlCanvasElement>().ok() {
                        
                        spawn_local(async move {            
                            TimeoutFuture::new(1000).await; // Wait 1 second for scripts to load
                            
                            if is_three_available() {
                                if let Ok(_) = init_globe(&canvas) {
                                    cube_loaded_clone.set(true);
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