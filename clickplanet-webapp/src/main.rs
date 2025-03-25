use dioxus::prelude::*;
use dioxus_web::launch;
use std::cell::RefCell;

mod app {
    pub mod components {
        pub mod buy_me_a_coffee;
        pub mod block_button;
        pub mod close_button;
        pub mod discord_button;
        pub mod modal;
        pub mod modal_manager;
        pub mod on_load_modal;
        pub mod select_with_search;
        pub mod globe;
        pub mod settings;
        pub mod leaderboard;
        pub mod about;
    }
    pub mod countries;
    pub mod viewer;
}

mod backends;

fn main() {
    console_log::init_with_level(log::Level::Debug).expect("Unable to initialize console_log");
    
    // Launch the web application
    launch(App);

}

// Define app routes
#[derive(Clone, PartialEq)]
enum Route {
    Home {},
    
    About {},
}

// App component takes no props now
// Commented out for now to fix compilation issues
/*
// Define a custom Link component for navigation
#[component]
fn Link(to: Route, children: Element) -> Element {
    let onclick = move |_| {
        APP_STATE.with(|state| {
            if let Some(nav_fn) = state.navigate.borrow().as_ref() {
                nav_fn(to.clone())
            }
        });
    };
    
    rsx! {
        button {
            class: "link-button",
            onclick: onclick,
            {children}
        }
    }
}

// A simple state holder for app-wide state
thread_local! {
    static APP_STATE: AppState = AppState::new();
}

struct AppState {
    navigate: RefCell<Option<Box<dyn Fn(Route) + 'static>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            navigate: RefCell::new(None),
        }
    }
}
*/

// Updated for Dioxus 0.6.x compatibility
fn App() -> Element {
    let mut current_route = use_signal(|| Route::Home {});
    
    let mut navigate = move |route: Route| {
        current_route.set(route);
    };
    
    // Commented out for now to fix compilation issues
    /*
    // Store the navigation function in the app state for Link components
    use_effect(move || {
        APP_STATE.with(|state| {
            *state.navigate.borrow_mut() = Some(Box::new(navigate.clone()));
        });
        || {}
    });
    */
    
    rsx! {
        nav { class: "main-nav",
            button { 
                onclick: move |_| navigate(Route::Home {}),
                class: if matches!(current_route(), Route::Home {}) { "active" } else { "" },
                "Home" 
            }
            button { 
                onclick: move |_| navigate(Route::About {}),
                class: if matches!(current_route(), Route::About {}) { "active" } else { "" },
                "About" 
            }
        }
        
        div { class: "content",
            match current_route() {
                Route::Home {} => rsx! { HomeScreen {} },
                Route::About {} => rsx! { AboutScreen {} },
            }
        }
    }
}

// Updated for Dioxus 0.6.x compatibility
fn HomeScreen() -> Element {
    let mut show_welcome_modal = use_signal(|| true);
    
    rsx! {
        div { class: "container",
            // Conditionally render the welcome modal
            if show_welcome_modal() {
                app::components::on_load_modal::OnLoadModal {
                    title: "Dear earthlings".to_string(),
                    on_close: move |_| show_welcome_modal.set(false),
                    children: rsx! {
                        div { class: "center-align",
                            img {
                                alt: "ClickPlanet logo",
                                src: "/static/logo.svg",
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
            
            // Main globe container
            app::components::globe::GlobeMock {}
            
            // Menu with leaderboard and settings
            div { class: "menu",
                app::components::leaderboard::Leaderboard {}
                div { class: "menu-actions",
                    app::components::settings::Settings {}
                    app::components::about::About {}
                    app::components::discord_button::DiscordButton {
                        message: Some("Join our Discord server".to_string()),
                    }
                }
            }
        }
    }
}

// Updated for Dioxus 0.6.x compatibility
fn AboutScreen() -> Element {
    rsx! {
        div { class: "about-page",
            h1 { "About ClickPlanet" }
            p { "ClickPlanet is a real-time collaborative globe where players from around the world can claim hexagonal territories for their countries." }
            p { "This is a Rust/WebAssembly implementation of the original ClickPlanet game." }
            // Link component commented out until fixed
            // Link { to: Route::Home {}, "Return to the globe" }
            button { 
                onclick: move |_| {
                    // Manual navigation logic
                    // Would normally use Link component
                }, 
                "Return to the globe" 
            }
        }
    }
}
