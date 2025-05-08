use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use gloo_storage::{LocalStorage, Storage};
use gloo_net::http::Request;

/// Country data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Country {
    pub name: String,
    pub code: String,
}

const STORAGE_KEY: &str = "clickplanet-country";

/// Country repository
pub struct CountryRepository;

impl CountryRepository {
    pub fn default_country() -> Country {
        Country {
            name: String::from("🇺🇸 United States"),
            code: String::from("us"),
        }
    }

    pub fn load_selected() -> Country {
        LocalStorage::get(STORAGE_KEY).unwrap_or_else(|_| Self::default_country())
    }

    pub fn save_selected(country: &Country) -> Result<(), String> {
        LocalStorage::set(STORAGE_KEY, country)
            .map_err(|e| e.to_string())
    }

    pub async fn load_all() -> Result<HashMap<String, Country>, String> {
        let static_site = env!("CITYWARS_STATIC_SITE");
        let url = format!("{}/countries/countries.json", static_site);
        
        let resp = Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
            
        let countries_data: HashMap<String, String> = resp.json()
            .await
            .map_err(|e| e.to_string())?;

        log::debug!("Loaded {} countries", countries_data.len());
        
        Ok(countries_data
            .into_iter()
            .map(|(code, name)| {
                (code.clone(), Country {
                    name,
                    code,
                })
            })
            .collect())
    }
}