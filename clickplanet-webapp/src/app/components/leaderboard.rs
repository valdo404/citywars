use dioxus::prelude::*;

// Structure to store leaderboard entry data
#[derive(Debug, Clone, PartialEq)]
struct LeaderboardEntry {
    country_code: String,
    country_name: String,
    points: u32,
}

// Component for displaying the leaderboard
#[component]
pub fn Leaderboard() -> Element {
    let mut is_expanded = use_signal(|| false);
    
    // In a full implementation, this would be fetched from an API
    let mut entries = use_signal(|| {
        let mut mock_data = vec![
            LeaderboardEntry {
                country_code: "US".to_string(),
                country_name: "United States".to_string(),
                points: 8754,
            },
            LeaderboardEntry {
                country_code: "FR".to_string(),
                country_name: "France".to_string(),
                points: 7234,
            },
            LeaderboardEntry {
                country_code: "DE".to_string(),
                country_name: "Germany".to_string(),
                points: 6145,
            },
            LeaderboardEntry {
                country_code: "JP".to_string(),
                country_name: "Japan".to_string(),
                points: 5467,
            },
            LeaderboardEntry {
                country_code: "GB".to_string(),
                country_name: "United Kingdom".to_string(),
                points: 4932,
            },
        ];
        
        // Sort by points, descending
        mock_data.sort_by(|a, b| b.points.cmp(&a.points));
        mock_data
    });

    let toggle_expanded = move |_| {
        is_expanded.set(!is_expanded());
    };

    rsx! {
        div { class: "leaderboard",
            button { 
                class: "leaderboard-toggle", 
                onclick: toggle_expanded,
                i { class: "fas fa-trophy" }
                " Leaderboard"
            }

            if is_expanded() {
                div { class: "leaderboard-content",
                    div { class: "leaderboard-header",
                        h3 { "Top Countries" }
                    }
                    ul { class: "leaderboard-list",
                        {entries().iter().enumerate().map(|(index, entry)| {
                            rsx! {
                                li { key: "{index}",
                                    span { class: "leaderboard-rank", "{index + 1}." }
                                    img { 
                                        class: "country-flag",
                                        src: "/static/flags/{entry.country_code.to_lowercase()}.svg",
                                        alt: "{entry.country_name} flag" 
                                    }
                                    span { class: "country-name", "{entry.country_name}" }
                                    span { class: "country-points", "{entry.points} pts" }
                                }
                            }
                        })}
                    }
                }
            }
        }
    }
}
