use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct BlockButtonProps {
    pub on_click: Callback<MouseEvent, ()>,
    pub text: String,                 // Text to display on the button
    pub image_url: Option<String>,    // Optional image URL (not used in the current implementation)
    pub class_name: Option<String>,   // Optional class name for styling
}

#[component]
pub fn BlockButton(props: BlockButtonProps) -> Element {
    let class_name = match props.class_name {
        Some(class) => format!("button {}", class),
        None => "button".to_string(),
    };

    rsx! {
        button {
            class: class_name,
            onclick: move |evt| props.on_click.call(evt),
            if props.image_url.is_some() {
                img {
                    src: props.image_url.expect("Image URL is required"),
                    alt: props.text.clone(),
                    class: "button-icon",
                    style: "margin-right: 8px; width: 20px; height: 20px;"
                }
            }
            span {{ props.text.clone() }}
        }
    }
}