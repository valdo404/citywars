use dioxus::prelude::*;

// Updated for Dioxus 0.6.x compatibility
pub fn GlobeMock() -> Element {
    rsx! {
        div { 
            class: "globe-container",
            style: "width: 100%; height: 80vh; display: flex; justify-content: center; align-items: center; background-color: #1a1a2e;",
            
            div {
                style: "text-align: center; color: white; padding: 20px; background-color: rgba(0,0,0,0.5); border-radius: 8px;",
                h2 { "Globe Component" }
                p { "This is a temporary placeholder while the interactive globe is being fixed." }
                p { "The full interactive globe will be restored soon." }
            }
        }
    }
}

