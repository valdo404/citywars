use dioxus::prelude::*;

use crate::app::countries::Country;
use crate::core::leaderboard::{percentage, short_name};

/// One leaderboard line, like the original `{ country, tiles }`.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaderboardEntry {
    pub country: Country,
    pub tiles: u32,
}

#[derive(Props, PartialEq, Clone)]
pub struct LeaderboardProps {
    pub data: Vec<LeaderboardEntry>,
    pub tiles_count: usize,
}

#[component]
pub fn Leaderboard(props: LeaderboardProps) -> Element {
    let mut is_open = use_signal(|| true);

    rsx! {
        div { class: "leaderboard",
            div { class: "leaderboard-header",
                img {
                    alt: "ClickPlanet logo",
                    src: crate::app::config::static_url("logo.svg"),
                    width: "56px",
                    height: "56px"
                }
                h1 { "ClickPlanet" }
            }
            div { class: "leaderboard-expand",
                button {
                    class: "button button-leaderboard",
                    onclick: move |_| is_open.set(!is_open()),
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
                            for (index, entry) in props.data.iter().enumerate() {
                                tr { key: "{entry.country.code}", class: "leaderboard-entry",
                                    td { class: "leaderboard-entry-index", "{index + 1}." }
                                    td { colspan: "3", "{short_name(&entry.country.name)}" }
                                    td { class: "leaderboard-table-number leaderboard-table-tiles", "{entry.tiles}" }
                                    td { class: "leaderboard-table-number", "{percentage(entry.tiles, props.tiles_count)}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
