use dioxus::prelude::*;
use crate::app::components::modal_manager::ModalManager;
use crate::app::components::select_with_search::{SelectWithSearch, Value as CountryValue};
use crate::app::components::block_button::BlockButtonProps;
use crate::app::countries::{Country, CountryRepository};

#[derive(Props, PartialEq, Clone)]
pub struct SettingsProps {
    pub country: Country,
    pub countries: Vec<Country>,
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
        if let Err(e) = CountryRepository::save_selected(&new_country) {
            log::warn!("cannot save the country: {e}");
        }
        props.set_country.call(new_country);
    };

    let values: Vec<CountryValue> = props
        .countries
        .iter()
        .map(|c| CountryValue { code: c.code.clone(), name: c.name.clone() })
        .collect();

    rsx! {
        ModalManager {
            open_by_default: false,
            modal_title: "Country".to_string(),
            button_props: BlockButtonProps {
                on_click: Callback::new(|_| {}),
                text: props.country.name.clone(),
                image_url: Some(crate::app::config::static_url(&format!("countries/svg/{}.svg", props.country.code))),
                class_name: Some("button-settings".to_string()),
            },
            close_button_text: None,
            modal_children: rsx! {
                div { class: "",
                    SelectWithSearch {
                        on_change: on_change,
                        selected: selected_country,
                        values: values,
                    }
                }
            },
        }
    }
}
