
mod common;

#[cfg(not(target_arch = "wasm32"))]
mod native_client;
#[cfg(target_arch = "wasm32")]
mod wasm_client;

pub use common::{ClickPlanetClient, TileCount, DynError};

#[cfg(not(target_arch = "wasm32"))]
pub use native_client::NativeClickPlanetRestClient as ClickPlanetRestClient;
#[cfg(target_arch = "wasm32")]
pub use wasm_client::WasmClickPlanetRestClient as ClickPlanetRestClient;

pub mod prelude {
    pub use super::ClickPlanetClient;
    pub use super::ClickPlanetRestClient;
    pub use super::TileCount;
    pub use super::DynError;
}