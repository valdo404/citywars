use dioxus::prelude::*;
use crate::app::components::modal_manager::ModalManager;
use crate::app::components::select_with_search::{SelectWithSearch, Value as CountryValue};
use crate::app::components::block_button::BlockButtonProps;
use crate::app::countries::{Country, CountryRepository};

#[derive(Props, PartialEq, Clone)]
pub struct SettingsProps {
    pub country: Country,
    pub set_country: Callback<Country>,
}

#[component]
pub fn Settings(props: SettingsProps) -> Element {
    let selected_country = CountryValue {
        code: props.country.code.clone(),
        name: props.country.name.clone(),
    };
    
    let on_change = move |selected_country: CountryValue| {
        let new_country = Country {
            name: selected_country.name,
            code: selected_country.code,
        };
        if let Ok(_) = CountryRepository::save_selected(&new_country) {
            gloo_console::info!("Saved country selection to localStorage", &serde_wasm_bindgen::to_value(&new_country).unwrap());
        }
        props.set_country.call(new_country);
    };
    
    let mut countries = use_signal(Vec::new);
    
    use_future(move || async move {
        if let Ok(countries_map) = CountryRepository::load_all().await {
            let loaded_countries: Vec<CountryValue> = countries_map
                .into_iter()
                .map(|(code, country)| CountryValue {
                    code,
                    name: country.name,
                })
                .collect();
            log::debug!("Setting {} countries", loaded_countries.len());
            countries.set(loaded_countries);
        }
    });
    
    rsx! {
        ModalManager {
            open_by_default: false,
            modal_title: "Country".to_string(),
            button_props: BlockButtonProps {
                on_click: Callback::new(|_| {}),
                text: props.country.name.clone(),
                image_url: None,
                class_name: Some("button-settings".to_string()),
            },
            close_button_text: None,
            modal_children: rsx! {
                div { class: "",
                    SelectWithSearch {
                        on_change: on_change,
                        selected: selected_country,
                        values: countries.read().to_vec(),
                    }
                }
            },
        }
    }
}
