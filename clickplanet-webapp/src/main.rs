use dioxus::prelude::*;
use dioxus::document::Stylesheet;
use crate::app::countries::Country;

mod app;
mod backends;

fn main() {
    console_log::init_with_level(log::Level::Info).expect("Unable to initialize console_log");
    
    launch(App);
}

// Define app routes with Routable trait
#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/")]
    Home {},
    
    
    // Fallback route for when no other routes match
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

// Root app component that sets up the router
#[allow(non_snake_case)]
fn App() -> Element {
    
    rsx! {
        document::Link { rel: "icon", href: asset!("public/static/favicon.png") }
        Stylesheet { href: asset!("public/styles/base.css") }
        Stylesheet { href: asset!("public/styles/DiscordButton.css") }
        Stylesheet { href: asset!("public/styles/BuyMeACoffee.css") }
        Stylesheet { href: asset!("public/styles/Modal.css") }
        Stylesheet { href: asset!("public/styles/CloseButton.css") }
        Stylesheet { href: asset!("public/styles/SelectWithSearch.css") }
        Stylesheet { href: asset!("public/styles/About.css") }
        Stylesheet { href: asset!("public/styles/Leaderboard.css") }
        Stylesheet { href: asset!("public/styles/Menu.css") }

        Stylesheet { href: asset!("public/styles/rust-specific.css") }
        div { class: "content",
            // Router uses the Route enum to handle URL-based routing
            Router::<Route> {}
        }
    }
}

// Home component with the globe and main UI
#[component]
fn Home() -> Element {
    let mut show_welcome_modal = use_signal(|| true);
    
    // Manage country state here, similar to the original TypeScript implementation
    let mut country = use_signal(|| Country {
        name: "United States".to_string(),
        code: "us".to_string(),
    });
    
    // Callback to update country
    let set_country = move |new_country: Country| {
        country.set(new_country);
    };
    
    rsx! {
        div { class: "container",
            if show_welcome_modal() {
                app::components::on_load_modal::OnLoadModal {
                    title: "Dear earthlings".to_string(),
                    on_close: move |_| show_welcome_modal.set(false),
                    children: rsx! {
                        div { class: "center-align",
                            img {
                                alt: "ClickPlanet logo",
                                src: format!("{}/favicon.png", env!("CITYWARS_STATIC_SITE")),
                                width: "64px",
                                height: "auto"
                            }
                        }
                        div { class: "modal-onload-text",
                            h3 { "Do you like ClickPlanet ?" }
                            p { "It's free and open-source 🤗" }
                            p { "Sadly, the servers are expensive to run 😭" }
                            p { "Every contribution helps us keep this awesome platform running!" }
                        }
                        app::components::buy_me_a_coffee::BuyMeACoffee {}
                    }
                }
            }
            
            // app::components::earth::globe::globe {}
            
            div { class: "menu",
                app::components::leaderboard::Leaderboard {}
                div { class: "menu-actions",
                    app::components::settings::Settings {
                        country: country(),
                        set_country: Callback::new(set_country),
                    }
                    app::components::about::About {}
                    app::components::discord_button::DiscordButton {
                        message: Some("Join us on Discord".to_string()),
                    }
                }
            }
        }
    }
}

#[component]
fn About() -> Element {
    rsx! {
        div { class: "about-page",
            h1 { "About ClickPlanet" }
            p { "ClickPlanet is a real-time collaborative globe where players from around the world can claim hexagonal territories for their countries." }
            p { "This is a Rust/WebAssembly implementation of the original ClickPlanet game." }
            button { 
                onclick: move |_| { router().push(Route::Home {}); }, 
                "Return to the globe" 
            }


        }
    }
}


// 404 Not Found component
#[component]
fn NotFound(segments: Vec<String>) -> Element {
    let url_path = if segments.is_empty() { 
        "/".to_string() 
    } else { 
        format!("/{}", segments.join("/")) 
    };
    
    rsx! {
        div { class: "not-found", 
            h1 { "Page Not Found" }
            p { "The page you're looking for doesn't exist." }
            p { 
                "Requested URL: {url_path}"
            }
            Link { to: Route::Home {}, 
                button { "Return Home" }
            }
        }
    }
}
