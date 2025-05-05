use dioxus::prelude::*;
use log::info;

/// The Globe component acts as a wrapper for our Three.js implementation
/// This uses the Three.js renderer instead of WebGPU directly for better compatibility
#[component]
pub fn Globe() -> Element {
    info!("Rendering Earth Globe with Three.js");
    
    rsx! {
        div { class: "globe-container",
            crate::app::components::earth::three_js::ThreeJsGlobe {}
        }
    }
}