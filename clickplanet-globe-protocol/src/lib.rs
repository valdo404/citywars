//! Event contract between the citywars web interface (Dioxus) and the globe renderer.
//!
//! The two are separate WebAssembly bundles. The interface owns the network (server RPC and live
//! updates) and the DOM; the globe owns the canvas, the camera and the tile rendering. They only
//! talk through the messages below, so the globe can be replaced (for example by a tuile-based one)
//! without touching the interface, as long as it speaks this protocol.
//!
//! # Transport
//!
//! Every message is a JavaScript object `{ payload: ArrayBuffer, canvas?: OffscreenCanvas | HTMLCanvasElement }`
//! sent with `postMessage` on a `Worker`/`DedicatedWorkerGlobalScope` (globe in a worker) or on the
//! two ends of a `MessageChannel` (globe on the main thread). `payload` is [`encode`]d with
//! postcard and is transferred, not copied. `canvas` is only present in [`UiToGlobe::Init`].
//!
//! # Tiles and countries
//!
//! Tiles are identified by their 1-based id, in the order of `coordinates.json` (the server uses
//! the same ids). Countries travel as small indices into the table sent with
//! [`UiToGlobe::SetCountries`]: index `i + 1` is `codes[i]`, index 0 means "no owner".

use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Bumped on any incompatible change. Both sides refuse messages with another version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Messages from the interface to the globe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiToGlobe {
    /// First message; the canvas travels next to the payload.
    Init(InitParams),
    /// Country table used by [`UiToGlobe::SetOwners`].
    SetCountries { codes: Vec<String> },
    /// Owners of a set of tiles: `owners[i]` owns `tiles[i]`. Used for the initial batches and for
    /// live updates.
    SetOwners { tiles: Vec<u32>, owners: Vec<u16> },
    /// The player's country; a click paints the tile with it right away (optimistic update, like
    /// the original front end). `None` disables the optimistic update.
    PlayerCountry { code: Option<String> },
    Settings(GlobeSettings),
    Camera(CameraCommand),
    /// Pointer input on the canvas, in CSS pixels relative to its top-left corner.
    Pointer(PointerInput),
    /// Mouse wheel, in pixels (positive = zoom out).
    Wheel { delta_y: f32 },
    /// The canvas was resized (CSS pixels).
    Resize { width: f32, height: f32, device_pixel_ratio: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitParams {
    /// Base URL of the static assets (earth texture, flag atlas, `coordinates.json`).
    pub assets_base_url: String,
    pub renderer: RendererPreference,
    pub width: f32,
    pub height: f32,
    pub device_pixel_ratio: f32,
    pub settings: GlobeSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RendererPreference {
    /// WebGPU when available, else WebGL2.
    Auto,
    WebGpu,
    WebGl2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobeSettings {
    /// Rotate slowly while the globe is not zoomed in (original behaviour).
    pub auto_rotate: bool,
    /// Report frame statistics every second ([`GlobeToUi::Stats`]).
    pub report_stats: bool,
}

impl Default for GlobeSettings {
    fn default() -> Self {
        GlobeSettings { auto_rotate: true, report_stats: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CameraCommand {
    /// Centre the view on a point (degrees); `zoom` 1 shows the whole globe.
    LookAt { lon: f32, lat: f32, zoom: Option<f32> },
    /// Back to the initial view.
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerKind {
    Down,
    Move,
    Up,
    Cancel,
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerType {
    Mouse,
    Touch,
    Pen,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointerInput {
    pub kind: PointerKind,
    pub pointer_id: i32,
    pub pointer_type: PointerType,
    pub x: f32,
    pub y: f32,
}

/// Messages from the globe to the interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GlobeToUi {
    /// The globe is initialised and has drawn its first frame.
    Ready { renderer: String, tile_count: u32 },
    /// Tile under the pointer changed (`None` = no tile).
    Hovered { tile: Option<u32> },
    /// A tile was clicked or tapped.
    Clicked { tile: u32, lon: f32, lat: f32 },
    /// Frame statistics over the last second.
    Stats { fps: f32, frame_time_ms: f32 },
    /// `fatal` errors mean the globe could not start (the interface may retry another mode).
    Error { message: String, fatal: bool },
    /// Tile positions are loaded (sent before [`GlobeToUi::Ready`], so the interface can start
    /// loading ownerships while the textures load).
    TilesLoaded { tile_count: u32 },
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    version: u16,
    message: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Malformed(String),
    Version { expected: u16, found: u16 },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Malformed(e) => write!(f, "malformed globe message: {e}"),
            DecodeError::Version { expected, found } => {
                write!(f, "globe protocol version {found}, expected {expected}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Encodes a message with the protocol version.
pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    postcard::to_allocvec(&Envelope { version: PROTOCOL_VERSION, message })
        .expect("globe messages always serialize")
}

/// Decodes a message, rejecting other protocol versions.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    // The version is the first field, so it can be read even if the rest does not match.
    let (version, _) = postcard::take_from_bytes::<u16>(bytes).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    if version != PROTOCOL_VERSION {
        return Err(DecodeError::Version { expected: PROTOCOL_VERSION, found: version });
    }
    postcard::from_bytes::<Envelope<T>>(bytes)
        .map(|e| e.message)
        .map_err(|e| DecodeError::Malformed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let messages = vec![
            UiToGlobe::SetCountries { codes: vec!["fr".into(), "de".into()] },
            UiToGlobe::SetOwners { tiles: vec![1, 2, 257_946], owners: vec![1, 2, 0] },
            UiToGlobe::Pointer(PointerInput {
                kind: PointerKind::Down,
                pointer_id: 1,
                pointer_type: PointerType::Touch,
                x: 10.5,
                y: 20.0,
            }),
            UiToGlobe::Camera(CameraCommand::LookAt { lon: 2.35, lat: 48.85, zoom: Some(4.0) }),
        ];
        for m in messages {
            assert_eq!(decode::<UiToGlobe>(&encode(&m)).unwrap(), m);
        }
        let ready = GlobeToUi::Ready { renderer: "WebGPU".into(), tile_count: 257_946 };
        assert_eq!(decode::<GlobeToUi>(&encode(&ready)).unwrap(), ready);
    }

    #[test]
    fn rejects_other_versions() {
        let mut bytes = encode(&GlobeToUi::Hovered { tile: None });
        bytes[0] = 99;
        assert_eq!(
            decode::<GlobeToUi>(&bytes),
            Err(DecodeError::Version { expected: PROTOCOL_VERSION, found: 99 })
        );
    }

    #[test]
    fn ownership_batches_are_compact() {
        // a full initial load: 258k tiles with small country indices
        let tiles: Vec<u32> = (1..=257_946).collect();
        let owners: Vec<u16> = tiles.iter().map(|t| (t % 250) as u16).collect();
        let bytes = encode(&UiToGlobe::SetOwners { tiles, owners });
        // varints: ~3 bytes per id and 1-2 bytes per owner
        assert!(bytes.len() < 257_946 * 5, "{} bytes", bytes.len());
    }
}
