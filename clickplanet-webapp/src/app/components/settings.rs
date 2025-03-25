use dioxus::prelude::*;
use crate::app::components::modal_manager::ModalManager;
use crate::app::components::select_with_search::{SelectWithSearch, Value as CountryValue};
use crate::app::components::block_button::BlockButtonProps;
use crate::app::countries::Country;

#[derive(Props, PartialEq, Clone)]
pub struct SettingsProps {
    pub country: Country,
    pub set_country: Callback<Country>,
}

/// Settings component for controlling country selection
#[component]
pub fn Settings(props: SettingsProps) -> Element {
    let country = props.country.clone();
    
    // Callback to handle country selection
    let on_change = move |selected_country: CountryValue| {
        let new_country = Country {
            name: selected_country.name,
            code: selected_country.code,
        };
        props.set_country.call(new_country);
    };
    
    // Convert country to CountryValue for SelectWithSearch
    let selected_country = CountryValue {
        code: country.code.clone(),
        name: country.name.clone(),
    };
    
    // Create a list of available countries
    // In a full implementation, this would be populated from the API
    let available_countries = vec![
        CountryValue { code: "us".to_string(), name: "United States".to_string() },
        CountryValue { code: "fr".to_string(), name: "France".to_string() },
        CountryValue { code: "de".to_string(), name: "Germany".to_string() },
        CountryValue { code: "jp".to_string(), name: "Japan".to_string() },
        CountryValue { code: "gb".to_string(), name: "United Kingdom".to_string() },
    ];
    
    rsx! {
        ModalManager {
            open_by_default: false,
            modal_title: "Country".to_string(),
            button_props: BlockButtonProps {
                on_click: Callback::new(|_| {}),
                text: country.name.clone(),
                image_url: format!("/static/countries/svg/{}.svg", country.code),
                class_name: Some("button-settings".to_string()),
            },
            close_button_text: None,
            modal_children: rsx! {
                div { class: "",
                    SelectWithSearch {
                        on_change: on_change,
                        selected: selected_country,
                        values: available_countries,
                    }
                }
            },
        }
    }
}
