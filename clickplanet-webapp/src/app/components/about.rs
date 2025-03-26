use dioxus::prelude::*;
use crate::app::components::modal_manager::ModalManager;
use crate::app::components::buy_me_a_coffee::BuyMeACoffee;
use crate::app::components::block_button::BlockButtonProps;

#[component]
pub fn About() -> Element {
    rsx! {
        ModalManager {
            open_by_default: false,
            modal_title: "ClickPlanet".to_string(),
            button_props: BlockButtonProps {
                on_click: Callback::new(|_| {}),
                text: "About".to_string(),
                image_url: "/static/favicon.png".to_string(),
                class_name: Some("button-about".to_string()),
            },
            close_button_text: Some("Back".to_string()),
            modal_children: rsx! {
                h3 { "The ultimate world war" }
                h4 { "It's like Pixel Wars but way more epic" }
                p {
                    "ClickPlanet is a virtual battleground where you "
                    br {}
                    "conquer territories for a country, click after click. "
                    br {}
                    "Out-click rival nations, and dominate the map. "
                    br {}
                    "Every territory is yours, until someone takes it back!"
                }
                
                div { class: "modal-about-project",
                    h3 { "About the project" }
                    p {
                        "ClickPlanet started as a TypeScript implementation and has evolved into "
                        "this Rust/WebAssembly version for improved performance and scalability."
                    }
                    p {
                        "This is an open-source collaborative project maintained by a team of "
                        "developers passionate about web technologies and interactive experiences."
                    }
                    h4 { "How to Play" }
                    ol {
                        li { "Select your country from the settings menu" }
                        li { "Click on any unclaimed hexagon to claim it for your country" }
                        li { "Each claim gives your country points" }
                        li { "Check the leaderboard to see how your country ranks" }
                    }
                }
                
                div { class: "about-links",
                    p { class: "center-align", "Want to contribute or learn more?" }
                    div { class: "center-align modal-about-social",
                        a { target: "_blank", href: "https://github.com/valdo404/clickplanet-client", b { "GitHub" } }
                    }
                }
                
                BuyMeACoffee {}
            },
        }
    }
}
