use dioxus::prelude::*;

// Settings component for controlling global application settings
pub fn Settings() -> Element {
    let mut show_settings = use_signal(|| false);
    let mut selected_country = use_signal(String::new);
    let mut show_grid = use_signal(|| true);
    let mut user_name = use_signal(String::new);
    
    let toggle_settings = move |_| {
        show_settings.set(!show_settings());
    };
    
    let toggle_grid = move |_| {
        show_grid.set(!show_grid());
    };
    
    rsx! {
        div { class: "settings-container",
            button { 
                class: "settings-toggle",
                onclick: toggle_settings,
                i { class: "fas fa-cog" }
                " Settings"
            }
            
            if show_settings() {
                div { class: "settings-panel",
                    h3 { "Game Settings" }
                    
                    div { class: "settings-section",
                        label { "Your Username" }
                        input {
                            r#type: "text",
                            placeholder: "Enter your username",
                            value: "{user_name}",
                            oninput: move |evt| user_name.set(evt.value().clone())
                        }
                    }
                    
                    div { class: "settings-section",
                        label { "Your Country" }
                        div { class: "country-selector",
                            // In a full implementation, this would be populated from the API
                            select {
                                value: "{selected_country}",
                                onchange: move |evt| selected_country.set(evt.value().clone()),
                                option { value: "", "Select your country" }
                                option { value: "us", "United States" }
                                option { value: "fr", "France" }
                                option { value: "de", "Germany" }
                                option { value: "jp", "Japan" }
                                option { value: "gb", "United Kingdom" }
                            }
                        }
                    }
                    
                    div { class: "settings-section",
                        label { "Display Settings" }
                        div { class: "toggle-option",
                            input {
                                id: "show-grid",
                                r#type: "checkbox",
                                checked: "{show_grid}",
                                onclick: toggle_grid
                            }
                            label { r#for: "show-grid", "Show Grid" }
                        }
                    }
                    
                    div { class: "settings-actions",
                        button {
                            class: "primary-button",
                            onclick: move |_| {
                                // In a full implementation, this would save settings to an API
                                log::info!(
                                    "Saved settings: Username: {}, Country: {}, Show Grid: {}", 
                                    user_name(), 
                                    selected_country(), 
                                    show_grid()
                                );
                                show_settings.set(false);
                            },
                            "Save Settings"
                        }
                        button {
                            class: "secondary-button",
                            onclick: move |_| show_settings.set(false),
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}
