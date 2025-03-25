use dioxus::prelude::*;
use crate::app::countries::Country;

// Structure to store leaderboard entry data matching the TypeScript implementation
#[derive(Debug, Clone, PartialEq)]
pub struct LeaderboardEntry {
    country: Country,
    tiles: u32,
}

// Props for the Leaderboard component
#[derive(Props, PartialEq, Clone)]
pub struct LeaderboardProps {
    #[props(default)]
    pub data: Option<Vec<LeaderboardEntry>>,
    #[props(default = 120000)]
    pub tiles_count: u32,
}

// Component for displaying the leaderboard
#[component]
pub fn Leaderboard() -> Element {
    let mut is_open = use_signal(|| true);
    
    // Total number of tiles in the globe - matches the default in the TypeScript implementation
    let total_tiles = 120000; // This would come from props.tiles_count in a real implementation
    
    // In a full implementation, this would be fetched from an API
    let entries = use_signal(|| {
        let mut mock_data = vec![
            LeaderboardEntry {
                country: Country { name: "United States".to_string(), code: "us".to_string() },
                tiles: 28754,
            },
            LeaderboardEntry {
                country: Country { name: "France".to_string(), code: "fr".to_string() },
                tiles: 22345,
            },
            LeaderboardEntry {
                country: Country { name: "Germany".to_string(), code: "de".to_string() },
                tiles: 18932,
            },
            LeaderboardEntry {
                country: Country { name: "Japan".to_string(), code: "jp".to_string() },
                tiles: 15467,
            },
            LeaderboardEntry {
                country: Country { name: "United Kingdom".to_string(), code: "gb".to_string() },
                tiles: 12543,
            },
            LeaderboardEntry {
                country: Country { name: "Brazil".to_string(), code: "br".to_string() },
                tiles: 9876,
            },
            LeaderboardEntry {
                country: Country { name: "Canada".to_string(), code: "ca".to_string() },
                tiles: 7654,
            },
            LeaderboardEntry {
                country: Country { name: "India".to_string(), code: "in".to_string() },
                tiles: 6543,
            },
            LeaderboardEntry {
                country: Country { name: "Australia".to_string(), code: "au".to_string() },
                tiles: 5432,
            },
            LeaderboardEntry {
                country: Country { name: "Spain".to_string(), code: "es".to_string() },
                tiles: 4321,
            },
        ];
        
        // Sort by tiles, descending
        mock_data.sort_by(|a, b| b.tiles.cmp(&a.tiles));
        mock_data
    });

    let toggle_leaderboard = move |_| {
        is_open.set(!is_open());
    };

    rsx! {
        div { class: "leaderboard",
            div { class: "leaderboard-header",
                img {
                    alt: "ClickPlanet logo",
                    src: "/public/static/favicon.png",
                    width: "56px",
                    height: "56px"
                }
                h1 { "ClickPlanet" }
            }
            div { class: "leaderboard-expand",
                button { 
                    class: "button button-leaderboard", 
                    onclick: toggle_leaderboard,
                    if is_open() { "Hide" } else { "Leaderboard" }
                }
            }

            if is_open() {
                div { class: "leaderboard-table-container",
                    table { class: "leaderboard-table",
                        thead {
                            tr {
                                th {}
                                th { colspan: "3", "🌍" }
                                th { class: "leaderboard-table-number leaderboard-table-tiles", "⚪️" }
                                th { class: "leaderboard-table-number", "%" }
                            }
                        }
                        tbody {
                            {entries().iter().enumerate().map(|(index, entry)| {
                                // Calculate percentage of total tiles
                                let percentage = (entry.tiles as f64 / total_tiles as f64 * 100.0).to_string();
                                let formatted_percentage = format!("{:.2}", percentage);
                                
                                // Truncate country name if too long
                                let max_length = 18;
                                let country_name = if entry.country.name.len() > max_length {
                                    format!("{}", entry.country.name[..max_length].trim())
                                } else {
                                    entry.country.name.clone()
                                };
                                
                                rsx! {
                                    tr { key: "{index}", class: "leaderboard-entry",
                                        td { class: "leaderboard-entry-index", "{index + 1}." }
                                        td { colspan: "3", 
                                            img { 
                                                class: "country-flag",
                                                src: "/public/static/countries/svg/{entry.country.code.to_lowercase()}.svg",
                                                alt: "{entry.country.name} flag",
                                                width: "20px",
                                                height: "auto",
                                                style: "margin-right: 8px;"
                                            }
                                            span { "{country_name}" }
                                        }
                                        td { class: "leaderboard-table-number leaderboard-table-tiles", "{entry.tiles}" }
                                        td { class: "leaderboard-table-number", "{formatted_percentage}" }
                                    }
                                }
                            })}
                        }
                    }
                }
            }
        }
    }
}
