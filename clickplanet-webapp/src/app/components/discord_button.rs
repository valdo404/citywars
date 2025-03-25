use dioxus::prelude::*;

// Updated for Dioxus 0.6.x compatibility
#[derive(PartialEq, Clone, Props)]
pub struct DiscordButtonProps {
    #[props(optional)]
    pub message: Option<String>, // Optional message to display, defaults to "Join us on Discord"
}

// Updated for Dioxus 0.6.x compatibility
pub fn DiscordButton(props: DiscordButtonProps) -> Element {
    let message = props
        .message
        .clone()
        .unwrap_or_else(|| "Join us on Discord".to_string());

    rsx!(
        a {
            href: "https://discord.gg/Nwekj6ndbn",
            target: "_blank",
            rel: "noopener noreferrer",
            class: "button button-discord",
            aria_label: "{message}",
            "{message}"
        }
    )
}
