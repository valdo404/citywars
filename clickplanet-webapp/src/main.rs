// The web app only targets wasm32 (build it with build-web.sh). On native the binary is a stub so
// that `cargo check --workspace` works; the platform-independent logic in `core` is tested there.
mod core;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod backends;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("clickplanet-webapp only runs in the browser: build it with `./build-web.sh` (target wasm32-unknown-unknown)");
    std::process::exit(1);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_log::init_with_level(log::Level::Info).expect("Unable to initialize console_log");
    dioxus::launch(web::App);
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::rc::Rc;

    use dioxus::document::Stylesheet;
    use dioxus::prelude::*;

    use crate::app;
    use crate::app::game::CountryList;

    #[derive(Routable, Clone, PartialEq)]
    enum Route {
        #[route("/")]
        Home {},

        #[route("/:..segments")]
        NotFound { segments: Vec<String> },
    }

    #[allow(non_snake_case)]
    pub fn App() -> Element {
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
                Router::<Route> {}
            }
        }
    }

    #[component]
    fn Home() -> Element {
        let countries = use_resource(app::countries::load_all);
        let countries = countries.read().clone();
        match countries {
            None => rsx! { div { class: "loading", "Loading…" } },
            Some(Err(e)) => rsx! { div { class: "globe-error", "Cannot load the game data: {e}" } },
            Some(Ok(list)) => rsx! {
                app::game::Game { countries: CountryList(Rc::new(list)) }
            },
        }
    }

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
                p { "Requested URL: {url_path}" }
                Link { to: Route::Home {},
                    button { "Return Home" }
                }
            }
        }
    }
}
