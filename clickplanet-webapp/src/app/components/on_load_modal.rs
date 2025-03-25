use dioxus::prelude::*;
use crate::app::components::modal::Modal;

// Updated for Dioxus 0.6.x compatibility
#[derive(PartialEq, Clone, Props)]
pub struct OnLoadModalProps {
    pub title: String,         // Title for the Modal
    pub children: Element,     // Children of the Modal
    pub on_close: EventHandler<()>,  // Event handler for closing the modal without event data
}

// Updated for Dioxus 0.6.x compatibility
pub fn OnLoadModal(props: OnLoadModalProps) -> Element {
    let mut is_open = use_signal(|| true); // Local state to track modal visibility

    if is_open() {
        rsx!(
            div { class: "modal-onload",
                Modal {
                    on_close: move || {
                        is_open.set(false);
                        // Forward the close event to the parent
                        // Call the handler with unit value since we don't need event data
                        props.on_close.call(());
                    },
                    title: props.title.to_string(),
                    children: props.children.clone(),
                }
            }
        )
    } else {
        rsx!()
    }

}