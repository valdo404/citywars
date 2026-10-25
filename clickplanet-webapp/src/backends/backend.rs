//! Backend interface, mirroring the original front end's `TileClicker`, `OwnershipsGetter` and
//! `UpdatesListener`.

use std::rc::Rc;

use crate::app::config::{BackendKind, Config};
use crate::backends::fake_backend::FakeBackend;
use crate::backends::http_backend::{HttpBackend, UpdatesTransport};

/// A tile changed owner.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub tile: u32,
    pub previous_country: Option<String>,
    pub new_country: String,
}

/// Callback receiving the updates of one batching window.
pub type UpdatesCallback = Rc<dyn Fn(Vec<Update>)>;

#[derive(Clone)]
pub enum Backend {
    Http(Rc<HttpBackend>),
    Fake(Rc<FakeBackend>),
}

impl Backend {
    /// `countries`: the country codes the fake backend's robots play for.
    pub fn from_config(config: &Config, countries: Vec<String>) -> Self {
        match config.backend {
            BackendKind::Http => {
                let transport = if config.websocket_updates { UpdatesTransport::WebSocket } else { UpdatesTransport::Sse };
                Backend::Http(Rc::new(HttpBackend::new(config.api_url.clone(), transport, 2000, 100)))
            }
            BackendKind::Fake => Backend::Fake(FakeBackend::new(500, countries)),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Backend::Http(_) => "http",
            Backend::Fake(_) => "fake",
        }
    }

    pub async fn click_tile(&self, tile_id: u32, country_id: &str) -> Result<(), String> {
        match self {
            Backend::Http(b) => b.click_tile(tile_id, country_id).await,
            Backend::Fake(b) => {
                b.click_tile(tile_id, country_id);
                Ok(())
            }
        }
    }

    /// Loads the current ownerships in batches of `batch_size` tiles, from tile 1 to `max_index`,
    /// calling `callback` after each batch.
    pub async fn get_current_ownerships_by_batch(
        &self,
        batch_size: u32,
        max_index: u32,
        callback: &mut dyn FnMut(Vec<(u32, String)>),
    ) -> Result<(), String> {
        match self {
            Backend::Http(b) => b.get_current_ownerships_by_batch(batch_size, max_index, callback).await,
            Backend::Fake(b) => {
                b.get_current_ownerships_by_batch(batch_size, max_index, callback);
                Ok(())
            }
        }
    }

    /// Current leaderboard (country code, tiles), as served by `/v2/rpc/leaderboard`.
    pub async fn leaderboard(&self) -> Result<Vec<(String, u32)>, String> {
        match self {
            Backend::Http(b) => b.leaderboard().await,
            Backend::Fake(b) => Ok(b.leaderboard()),
        }
    }

    /// Registers a callback for batched live updates (every 100 ms for HTTP, 500 ms for the fake
    /// backend, like the original).
    /// Called when live updates were lost and the ownerships must be reloaded (SSE `resync`).
    pub fn on_resync(&self, callback: Rc<dyn Fn()>) {
        if let Backend::Http(b) = self {
            b.on_resync(callback);
        }
    }

    pub fn listen_for_updates_batch(&self, callback: UpdatesCallback) {
        match self {
            Backend::Http(b) => b.listen_for_updates_batch(callback),
            Backend::Fake(b) => b.listen_for_updates_batch(callback),
        }
    }
}
