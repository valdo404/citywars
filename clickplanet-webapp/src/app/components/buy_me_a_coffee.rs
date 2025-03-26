use dioxus::prelude::*;

// In Dioxus 0.6.x, components are defined as regular functions
pub fn BuyMeACoffee() -> Element {
    rsx!(
        a {
            href: "https://buymeacoffee.com/raphoester",
            target: "_blank",
            rel: "noopener noreferrer",
            class: "button button-coffee",
            "Buy me a coffee"
        }
    )
}
