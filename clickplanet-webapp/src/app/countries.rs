use serde::{Deserialize, Serialize};
use gloo_storage::{LocalStorage, Storage};

/// Country data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Country {
    pub name: String,
    pub code: String,
}

/// Same key as the original front end, so a stored choice is kept.
const STORAGE_KEY: &str = "clickplanet-country";

/// Country repository
pub struct CountryRepository;

impl CountryRepository {
    /// Fallback used by the original front end when the visitor's country is unknown.
    pub fn default_country() -> Country {
        Country {
            name: String::from("🇫🇷 France"),
            code: String::from("fr"),
        }
    }

    /// The stored choice, else the visitor's country, else France.
    ///
    /// The original guessed the visitor's country from the time zone (moment-timezone tables);
    /// this uses the region of the browser languages instead (`fr-FR` → `fr`), which needs no
    /// extra data.
    pub fn load_selected(countries: &[Country]) -> Country {
        if let Ok(stored) = LocalStorage::get::<Country>(STORAGE_KEY) {
            if let Some(country) = countries.iter().find(|c| c.code == stored.code) {
                return country.clone();
            }
        }
        Self::visitor_country(countries).unwrap_or_else(Self::default_country)
    }

    fn visitor_country(countries: &[Country]) -> Option<Country> {
        let navigator = web_sys::window()?.navigator();
        let languages: Vec<String> = navigator
            .languages()
            .iter()
            .filter_map(|l| l.as_string())
            .chain(navigator.language())
            .collect();
        languages.iter().find_map(|tag| {
            let region = tag.split(['-', '_']).nth(1)?.to_lowercase();
            countries.iter().find(|c| c.code == region).cloned()
        })
    }

    pub fn save_selected(country: &Country) -> Result<(), String> {
        LocalStorage::set(STORAGE_KEY, country).map_err(|e| e.to_string())
    }
}

/// Loads `countries/countries.json` (code → name with flag emoji), sorted by name.
pub async fn load_all() -> Result<Vec<Country>, String> {
    let url = crate::app::config::static_url("countries/countries.json");
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let names: std::collections::HashMap<String, String> =
        response.json().await.map_err(|e| format!("{url}: {e}"))?;
    let mut countries: Vec<Country> = names.into_iter().map(|(code, name)| Country { name, code }).collect();
    countries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(countries)
}
