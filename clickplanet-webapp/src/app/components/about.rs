use dioxus::prelude::*;
use crate::Route;

#[component]
pub fn About() -> Element {
    let mut show_about = use_signal(|| false);
    
    let toggle_about = move |_| {
        show_about.set(!show_about());
    };
    
    rsx! {
        div { class: "about-container",
            button { 
                class: "about-toggle",
                onclick: toggle_about,
                i { class: "fas fa-info-circle" }
                " About"
            }
            
            if show_about() {
                div { class: "about-panel",
                    h3 { "About ClickPlanet" }
                    
                    div { class: "about-content",
                        p {
                            "ClickPlanet is a collaborative planet where players from around the world can claim hexagonal territories for their countries."
                        }
                        
                        p {
                            "This is a Rust/WebAssembly implementation of the ClickPlanet game, with performance and scalability as primary goals."
                        }
                        
                        h4 { "How to Play" }
                        ol {
                            li { "Select your country from the settings menu" }
                            li { "Click on any unclaimed (gray) hexagon on the globe to claim it" }
                            li { "Each claim gives your country points" }
                            li { "Check the leaderboard to see how your country ranks!" }
                        }
                        
                        div { class: "about-links",
                            // Link component commented out until fixed
                            // Link { 
                            //     to: Route::About {}, 
                            //     class: "learn-more",
                            //     "Learn More"
                            // }
                            button {
                                class: "learn-more",
                                "Learn More"
                            }
                        }
                    }
                    
                    button {
                        class: "close-about",
                        onclick: move |_| show_about.set(false),
                        "Close"
                    }
                }
            }
        }
    }
}
