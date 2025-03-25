use dioxus::prelude::*;
use std::rc::Rc;
use dioxus::logger::tracing::event;

/// Convert a country code (e.g., "us") to a flag emoji (e.g., "🇺🇸")
fn get_flag_emoji(country_code: &str) -> String {
    // Regional Indicator Symbols start at Unicode codepoint U+1F1E6 for 'A'
    // We convert each ASCII letter to its regional indicator symbol
    let regional_indicator_a = 0x1F1E6; // Unicode codepoint for Regional Indicator Symbol Letter A
    
    // Convert country code to uppercase and then to flag emoji
    country_code
        .to_uppercase()
        .chars()
        .map(|c| {
            // Only process A-Z characters
            if c >= 'A' && c <= 'Z' {
                // Calculate the codepoint for the regional indicator symbol
                let offset = c as u32 - 'A' as u32;
                let regional_indicator = regional_indicator_a + offset;
                
                // Convert the codepoint to a character
                std::char::from_u32(regional_indicator).unwrap_or(' ')
            } else {
                // Leave non-alphabetic characters as-is
                c
            }
        })
        .collect()
}

#[derive(PartialEq, Clone)]
pub struct Value {
    pub code: String,
    pub name: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct SelectWithSearchProps {
    selected: Value,                      // Currently selected value
    values: Vec<Value>,                  // List of dropdown values
    on_change: Callback<Value>,             // Callback when selection changes
}

#[component]
pub fn SelectWithSearch(props: SelectWithSearchProps) -> Element {
    let mut search: Signal<String> = use_signal(|| "".to_string());      // Controlled state for search input
    let mut selected: Signal<Value> = use_signal(|| props.selected.clone()); // Controlled state for selected item

    let filtered_options: Vec<Value> = props
        .values
        .iter()
        .filter(|v| v.name.to_lowercase().contains(&search.read().to_lowercase())) // Access search as read-only
        .cloned() // Clone the filtered values to ensure they are owned
        .collect();

    let handle_search_change = move |event: Event<FormData>| {
        let input = event.data.value(); // Use the returned String
        search.set(input); // Update the state
    };

    let handle_select_change = {
        let mut selected = selected.clone(); // Clone the signal for capturing
        let on_change = props.on_change.clone(); // Clone callback
        let mut search = search.clone(); // Clone search signal

        move |event: Event<FormData>| {
            let value_code = event.data.value(); // Extract input value

            if let Some(value) = props.values.iter().find(|v| v.code == value_code) {
                selected.set(value.clone()); // Update selected state
                search.set("".to_string()); // Reset search
                on_change.call(value.clone()); // Call the on_change callback
            } else {
                log::error!("Value not found for code {}", value_code);
            }
        }
    };

    rsx! {
        div {
            select {
                class: "input-select",
                value: "{selected.read().code}", // Bind selected value
                oninput: handle_select_change, // Input change handler
                size: "5",
                {filtered_options.into_iter().map(|v| {
                    let value = v.clone();
                    // Format the display text with flag emoji outside the RSX macro
                    let display_text = format!("{}  {}", get_flag_emoji(&v.code), v.name);
                    
                    rsx!(
                        option {
                            class: "input-select-option",
                            onclick: move |_| {
                                selected.set(value.clone());
                                props.on_change.call(value.clone());
                            },
                            key: "{v.code}", // Unique key for each option
                            value: "{v.code}",
                            "{display_text}"
                        }
                    )
                })}
            }
            input {
                class: "input-search",
                r#type: "text",
                placeholder: "🔍 Search...",
                value: "{search}",
                oninput: handle_search_change, // Search input change
                autocomplete: "off",
            }
        }
    }
}