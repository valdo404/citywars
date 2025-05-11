use futures::stream::BoxStream;
use std::sync::Arc;
use clickplanet_proto::clicks;
use serde::Deserialize;

#[cfg(not(target_arch = "wasm32"))]
use base64::{engine::general_purpose::STANDARD, Engine as _};

pub trait TileCount {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// Error type that's conditionally defined based on target
#[cfg(not(target_arch = "wasm32"))]
pub type DynError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type DynError = Box<dyn std::error::Error>;

// Common trait definition for both platforms with target-specific attributes
#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
pub trait ClickPlanetClient {
    async fn click_tile(&self, tile_id: u32, country_id: &str) -> Result<(), DynError>;
    async fn get_ownerships_by_batch(&self, start_tile_id: u32, end_tile_id: u32) -> Result<clicks::OwnershipState, DynError>;
    async fn get_ownerships(&self, index_coordinates: &Arc<dyn TileCount + Send + Sync>) -> Result<clicks::OwnershipState, DynError>;
    async fn listen_for_updates(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError>;
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
pub trait ClickPlanetClient {
    async fn click_tile(&self, tile_id: u32, country_id: &str) -> Result<(), DynError>;
    async fn get_ownerships_by_batch(&self, start_tile_id: u32, end_tile_id: u32) -> Result<clicks::OwnershipState, DynError>;
    async fn get_ownerships(&self, index_coordinates: &Arc<dyn TileCount + Send + Sync>) -> Result<clicks::OwnershipState, DynError>;
    async fn listen_for_updates(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError>;
}

pub const CLIENT_NAME: &'static str = "clickplanet client owned by valdo404";

#[derive(Deserialize)]
pub struct OwnershipResponse {
    pub data: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn generate_websocket_key() -> String {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let mut key = [0u8; 16];
    rng.fill(&mut key);
    STANDARD.encode(key)
}
