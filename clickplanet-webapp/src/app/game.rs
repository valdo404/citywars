//! The game screen: owns the network (backend), the leaderboard and the player's country, and
//! drives the globe bundle through [`GlobeClient`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use clickplanet_globe_protocol::{GlobeSettings, GlobeToUi, InitParams, RendererPreference, UiToGlobe};
use dioxus::prelude::*;
use futures::channel::mpsc::unbounded;
use futures::StreamExt;

use crate::app::components;
use crate::app::components::leaderboard::LeaderboardEntry;
use crate::app::config::{Config, RendererChoice, STATIC_SITE};
use crate::app::countries::{Country, CountryRepository};
use crate::app::globe_client::{offscreen_supported, GlobeClient, GlobeEvent, GlobeMode};
use crate::backends::backend::{Backend, Update};
use crate::core::leaderboard::LeaderboardModel;

/// `coordinates.json` length, used until the globe reports the real count.
const DEFAULT_TILE_COUNT: usize = 257_946;
const OWNERSHIP_BATCH_SIZE: u32 = 10_000;

/// Country list passed as a prop (compared by pointer).
#[derive(Clone)]
pub struct CountryList(pub Rc<Vec<Country>>);

impl PartialEq for CountryList {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Country table of the globe protocol: index `i + 1` is `codes[i]`.
#[derive(Default)]
struct CountryIndex {
    codes: Vec<String>,
    index: HashMap<String, u16>,
}

impl CountryIndex {
    fn new(countries: &[Country]) -> Self {
        let mut codes: Vec<String> = countries.iter().map(|c| c.code.clone()).collect();
        codes.sort();
        let index = codes.iter().enumerate().map(|(i, c)| (c.clone(), i as u16 + 1)).collect();
        CountryIndex { codes, index }
    }

    /// Index of a code, adding unknown codes (returns whether the table grew).
    fn index_or_insert(&mut self, code: &str) -> (u16, bool) {
        if let Some(&i) = self.index.get(code) {
            return (i, false);
        }
        self.codes.push(code.to_string());
        let i = self.codes.len() as u16;
        self.index.insert(code.to_string(), i);
        (i, true)
    }

    fn code(&self, index: u16) -> Option<&str> {
        index.checked_sub(1).and_then(|i| self.codes.get(i as usize)).map(String::as_str)
    }
}

/// Interface-side game state (not rendered directly).
#[derive(Default)]
struct UiState {
    countries: CountryIndex,
    /// Owner index of each tile id (index 0 unused).
    owners: Vec<u16>,
    model: LeaderboardModel,
    ownership_load_started: bool,
}

impl UiState {
    fn set_owner(&mut self, tile: u32, owner: u16) -> u16 {
        let i = tile as usize;
        if self.owners.len() <= i {
            self.owners.resize(i + 1, 0);
        }
        std::mem::replace(&mut self.owners[i], owner)
    }

    fn owned_tiles(&self) -> (Vec<u32>, Vec<u16>) {
        self.owners
            .iter()
            .enumerate()
            .filter(|(_, &o)| o != 0)
            .map(|(t, &o)| (t as u32, o))
            .unzip()
    }

    fn counts(&self) -> Vec<(String, u32)> {
        let mut counts: HashMap<u16, u32> = HashMap::new();
        for &o in self.owners.iter().filter(|&&o| o != 0) {
            *counts.entry(o).or_default() += 1;
        }
        counts
            .into_iter()
            .filter_map(|(o, n)| self.countries.code(o).map(|c| (c.to_string(), n)))
            .collect()
    }
}

fn performance_mark(name: &str) {
    if let Some(p) = web_sys::window().and_then(|w| w.performance()) {
        let _ = p.mark(name);
    }
}

/// Exposes measurements on `window.citywarsStats` (read by the end-to-end checks).
fn publish_stat(key: &str, value: wasm_bindgen::JsValue) {
    let Some(window) = web_sys::window() else { return };
    let stats = js_sys::Reflect::get(&window, &"citywarsStats".into())
        .ok()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| {
            let o: wasm_bindgen::JsValue = js_sys::Object::new().into();
            let _ = js_sys::Reflect::set(&window, &"citywarsStats".into(), &o);
            o
        });
    let _ = js_sys::Reflect::set(&stats, &key.into(), &value);
}

#[derive(Clone, PartialEq, Default)]
struct StatsView {
    renderer: String,
    fps: f32,
    frame_time_ms: f32,
}

fn initial_globe_mode() -> GlobeMode {
    let forced_main = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .is_some_and(|s| s.contains("globe=main"));
    let supported = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("canvas").ok())
        .and_then(|e| wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlCanvasElement>(e).ok())
        .is_some_and(|c| offscreen_supported(&c));
    if forced_main || !supported {
        GlobeMode::MainThread
    } else {
        GlobeMode::Worker
    }
}

/// Sends the whole interface state to a (re)started globe.
fn sync_globe(client: &GlobeClient, state: &UiState, country: &Country) {
    client.send(&UiToGlobe::SetCountries { codes: state.countries.codes.clone() });
    client.send(&UiToGlobe::PlayerCountry { code: Some(country.code.clone()) });
    let (tiles, owners) = state.owned_tiles();
    if !tiles.is_empty() {
        client.send(&UiToGlobe::SetOwners { tiles, owners });
    }
}

#[component]
pub fn Game(countries: CountryList) -> Element {
    let config = use_hook(Config::from_environment);
    let backend = use_hook({
        let codes: Vec<String> = countries.0.iter().map(|c| c.code.clone()).collect();
        let config = config.clone();
        move || Backend::from_config(&config, codes)
    });
    let state = use_hook({
        let countries = countries.clone();
        move || {
            Rc::new(RefCell::new(UiState {
                countries: CountryIndex::new(&countries.0),
                ..Default::default()
            }))
        }
    });
    let names: Rc<HashMap<String, Country>> = use_hook({
        let countries = countries.clone();
        move || Rc::new(countries.0.iter().map(|c| (c.code.clone(), c.clone())).collect())
    });

    let mut country = use_signal({
        let countries = countries.clone();
        move || CountryRepository::load_selected(&countries.0)
    });
    let leaderboard = use_signal(Vec::<LeaderboardEntry>::new);
    let mut tile_count = use_signal(|| DEFAULT_TILE_COUNT);
    let mut attempt = use_signal(|| 0u32);
    let mut stats = use_signal(StatsView::default);
    let mut hovered = use_signal(|| None::<u32>);
    let mut globe_error = use_signal(|| None::<String>);
    let mut show_welcome_modal = use_signal(|| js_sys::Math::random() > 0.5);

    let (client, events) = use_hook(|| {
        let (tx, rx) = unbounded::<GlobeEvent>();
        (GlobeClient::new(initial_globe_mode(), tx), Rc::new(RefCell::new(Some(rx))))
    });

    let init = use_hook(|| InitParams {
        assets_base_url: STATIC_SITE.to_string(),
        renderer: match config.renderer {
            RendererChoice::Auto => RendererPreference::Auto,
            RendererChoice::WebGpu => RendererPreference::WebGpu,
            RendererChoice::WebGl => RendererPreference::WebGl2,
        },
        width: 1.0,
        height: 1.0,
        device_pixel_ratio: 1.0,
        settings: GlobeSettings { auto_rotate: true, report_stats: true },
    });

    // recompute the visible leaderboard from the model
    let commit_leaderboard = {
        let state = state.clone();
        let names = names.clone();
        move || {
            let rows = state
                .borrow()
                .model
                .entries()
                .into_iter()
                .map(|(code, tiles)| LeaderboardEntry {
                    country: names
                        .get(&code)
                        .cloned()
                        .unwrap_or(Country { name: code.clone(), code }),
                    tiles,
                })
                .collect();
            let mut leaderboard = leaderboard;
            leaderboard.set(rows);
        }
    };

    // applies server updates to the interface state and relays them to the globe
    let apply_updates = {
        let state = state.clone();
        let client = client.clone();
        let commit = commit_leaderboard.clone();
        move |updates: Vec<Update>| {
            let mut s = state.borrow_mut();
            let mut grew = false;
            let mut tiles = Vec::with_capacity(updates.len());
            let mut owners = Vec::with_capacity(updates.len());
            for u in updates {
                let (index, inserted) = s.countries.index_or_insert(&u.new_country);
                grew |= inserted;
                let previous_index = s.set_owner(u.tile, index);
                let previous = s
                    .countries
                    .code(previous_index)
                    .map(str::to_string)
                    .or(u.previous_country);
                s.model.register_click(previous.as_deref(), &u.new_country);
                tiles.push(u.tile);
                owners.push(index);
            }
            if grew {
                client.send(&UiToGlobe::SetCountries { codes: s.countries.codes.clone() });
            }
            client.send(&UiToGlobe::SetOwners { tiles, owners });
            drop(s);
            commit();
        }
    };

    // network: leaderboard snapshot and live updates, started once
    use_hook({
        let backend = backend.clone();
        let state = state.clone();
        let commit = commit_leaderboard.clone();
        let apply_updates = apply_updates.clone();
        move || {
            performance_mark("citywars-ui-ready");
            publish_stat("backend", backend.name().into());
            let snapshot_backend = backend.clone();
            let snapshot_state = state.clone();
            let commit = commit.clone();
            spawn(async move {
                match snapshot_backend.leaderboard().await {
                    Ok(entries) => {
                        let mut s = snapshot_state.borrow_mut();
                        if !s.ownership_load_started || s.owners.is_empty() {
                            s.model.set_snapshot(entries);
                        }
                        drop(s);
                        commit();
                    }
                    Err(e) => log::error!("Failed to fetch the leaderboard: {e}"),
                }
            });

            let (tx, mut rx) = unbounded::<Vec<Update>>();
            backend.listen_for_updates_batch(Rc::new(move |updates| {
                let _ = tx.unbounded_send(updates);
            }));
            spawn(async move {
                while let Some(updates) = rx.next().await {
                    apply_updates(updates);
                }
            });
        }
    });

    // globe events
    use_hook({
        let client = client.clone();
        let backend = backend.clone();
        let state = state.clone();
        let commit = commit_leaderboard.clone();
        move || {
            let mut rx = events.borrow_mut().take().expect("globe events are consumed once");
            spawn(async move {
                while let Some(event) = rx.next().await {
                    match event {
                        GlobeEvent::Message(GlobeToUi::TilesLoaded { tile_count: n }) => {
                            performance_mark("citywars-tiles-loaded");
                            tile_count.set(n as usize);
                            publish_stat("tileCount", n.into());
                            let started = std::mem::replace(&mut state.borrow_mut().ownership_load_started, true);
                            if started {
                                continue;
                            }
                            let backend = backend.clone();
                            let state = state.clone();
                            let client = client.clone();
                            let commit = commit.clone();
                            spawn(async move {
                                let started_at = js_sys::Date::now();
                                let mut on_batch = |batch: Vec<(u32, String)>| {
                                    let mut s = state.borrow_mut();
                                    let mut grew = false;
                                    let mut tiles = Vec::with_capacity(batch.len());
                                    let mut owners = Vec::with_capacity(batch.len());
                                    for (tile, code) in batch {
                                        let (index, inserted) = s.countries.index_or_insert(&code);
                                        grew |= inserted;
                                        s.set_owner(tile, index);
                                        tiles.push(tile);
                                        owners.push(index);
                                    }
                                    if grew {
                                        client.send(&UiToGlobe::SetCountries { codes: s.countries.codes.clone() });
                                    }
                                    client.send(&UiToGlobe::SetOwners { tiles, owners });
                                };
                                let result = backend
                                    .get_current_ownerships_by_batch(OWNERSHIP_BATCH_SIZE, n, &mut on_batch)
                                    .await;
                                if let Err(e) = result {
                                    log::error!("Failed to fetch initial ownerships: {e}");
                                    return;
                                }
                                // exact counts once every tile is known
                                {
                                    let mut s = state.borrow_mut();
                                    let counts = s.counts();
                                    s.model.set_snapshot(counts);
                                }
                                commit();
                                performance_mark("citywars-ownerships-loaded");
                                let (bytes, relay_ms) = client.transfer_stats();
                                publish_stat("ownershipsLoadMs", (js_sys::Date::now() - started_at).into());
                                publish_stat("relayBytes", (bytes as f64).into());
                                publish_stat("relayEncodePostMs", relay_ms.into());
                            });
                        }
                        GlobeEvent::Message(GlobeToUi::Ready { renderer, tile_count: n }) => {
                            performance_mark("citywars-globe-ready");
                            client.set_ready();
                            tile_count.set(n as usize);
                            publish_stat("renderer", renderer.clone().into());
                            publish_stat("globeMode", client.mode().label().into());
                            stats.with_mut(|s| s.renderer = renderer);
                            globe_error.set(None);
                        }
                        GlobeEvent::Message(GlobeToUi::Hovered { tile }) => {
                            hovered.set(tile);
                            publish_stat("hoveredTile", tile.map(|t| t as f64).into());
                        }
                        GlobeEvent::Message(GlobeToUi::Clicked { tile, lon, lat }) => {
                            let code = country.peek().code.clone();
                            log::info!("click on tile {tile} ({lat:.2}, {lon:.2}) for {code}");
                            publish_stat("lastClickedTile", (tile as f64).into());
                            let backend = backend.clone();
                            spawn(async move {
                                if let Err(e) = backend.click_tile(tile, &code).await {
                                    log::error!("click failed: {e}");
                                }
                            });
                        }
                        GlobeEvent::Message(GlobeToUi::Stats { fps, frame_time_ms }) => {
                            publish_stat("fps", fps.into());
                            publish_stat("frameTimeMs", frame_time_ms.into());
                            stats.with_mut(|s| {
                                s.fps = fps;
                                s.frame_time_ms = frame_time_ms;
                            });
                        }
                        GlobeEvent::Message(GlobeToUi::Error { message, fatal }) => {
                            log::error!("globe: {message}");
                            if fatal && client.mode() == GlobeMode::Worker && !client.is_ready() {
                                log::warn!("retrying the globe on the main thread");
                                client.restart_in(GlobeMode::MainThread);
                                attempt += 1;
                            } else if fatal {
                                globe_error.set(Some(message));
                            }
                        }
                        GlobeEvent::WorkerFailed(message) => {
                            log::warn!("{message}; retrying the globe on the main thread");
                            client.restart_in(GlobeMode::MainThread);
                            attempt += 1;
                        }
                    }
                }
            });
        }
    });

    let on_globe_started = {
        let client = client.clone();
        let state = state.clone();
        move |_| sync_globe(&client, &state.borrow(), &country.peek())
    };

    let set_country = {
        let client = client.clone();
        move |new_country: Country| {
            client.send(&UiToGlobe::PlayerCountry { code: Some(new_country.code.clone()) });
            country.set(new_country);
        }
    };

    rsx! {
        div { class: "container",
            if show_welcome_modal() {
                components::on_load_modal::OnLoadModal {
                    title: "Dear earthlings".to_string(),
                    on_close: move |_| show_welcome_modal.set(false),
                    children: rsx! {
                        div { class: "center-align",
                            img {
                                alt: "ClickPlanet logo",
                                src: crate::app::config::static_url("logo.svg"),
                                width: "64px",
                                height: "auto"
                            }
                        }
                        div { class: "modal-onload-text",
                            h3 { "Do you like ClickPlanet ?" }
                            p { "It's free and open-source 🤗" }
                            p { "Sadly, the servers are expensive to run 😭" }
                            p { "Every contribution helps us keep this awesome platform running!" }
                        }
                        components::buy_me_a_coffee::BuyMeACoffee {}
                    }
                }
            }

            components::earth::globe::Globe {
                client: client.clone(),
                init: init.clone(),
                attempt: attempt(),
                on_started: on_globe_started,
            }

            if let Some(message) = globe_error() {
                div { class: "globe-error", "The globe could not start: {message}" }
            }

            if config.show_stats {
                div { class: "stats-hud",
                    "{stats().renderer} · {client.mode().label()} · {stats().fps:.0} fps · {stats().frame_time_ms:.1} ms"
                    if let Some(tile) = hovered() { " · tile {tile}" }
                    " · {backend.name()}"
                }
            }

            div { class: "menu",
                components::leaderboard::Leaderboard {
                    data: leaderboard(),
                    tiles_count: tile_count(),
                }
                div { class: "menu-actions",
                    components::settings::Settings {
                        country: country(),
                        countries: countries.0.as_ref().clone(),
                        set_country: Callback::new(set_country),
                    }
                    components::about::About {}
                    components::discord_button::DiscordButton {
                        message: Some("Join us on Discord".to_string()),
                    }
                }
            }
        }
    }
}
