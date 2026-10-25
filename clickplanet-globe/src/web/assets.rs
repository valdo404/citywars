//! Static assets of the globe, fetched by the globe itself (it owns its rendering data): tile
//! positions, flag atlas regions, flag atlas and earth texture. Images are decoded and resized by
//! the browser (`createImageBitmap`), which works both on the main thread and in workers.

use std::collections::HashMap;

use js_sys::{Promise, Reflect, Uint8Array};
use serde::de::IgnoredAny;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, ImageBitmap, ImageBitmapOptions, PremultiplyAlpha, ResizeQuality, Response};

#[derive(Deserialize)]
struct Coordinates {
    positions: Vec<f32>,
    #[allow(dead_code)]
    uvs: IgnoredAny,
    length: usize,
}

#[derive(Deserialize, Clone, Copy)]
pub struct AtlasRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

fn js_error(context: &str, e: JsValue) -> String {
    format!("{context}: {}", e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

fn call_global(name: &str, args: &[&JsValue]) -> Result<JsValue, String> {
    let global = js_sys::global();
    let f: js_sys::Function = Reflect::get(&global, &name.into())
        .map_err(|e| js_error(name, e))?
        .dyn_into()
        .map_err(|_| format!("{name} is not available"))?;
    let array: js_sys::Array = args.iter().copied().collect();
    f.apply(&global, &array).map_err(|e| js_error(name, e))
}

async fn fetch(url: &str) -> Result<Response, String> {
    let promise: Promise = call_global("fetch", &[&url.into()])?.unchecked_into();
    let response: Response = JsFuture::from(promise).await.map_err(|e| js_error(url, e))?.unchecked_into();
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    Ok(response)
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = fetch(url).await?;
    let buffer = JsFuture::from(response.array_buffer().map_err(|e| js_error(url, e))?)
        .await
        .map_err(|e| js_error(url, e))?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

async fn fetch_blob(url: &str) -> Result<Blob, String> {
    let response = fetch(url).await?;
    let blob = JsFuture::from(response.blob().map_err(|e| js_error(url, e))?)
        .await
        .map_err(|e| js_error(url, e))?;
    Ok(blob.unchecked_into())
}

async fn create_bitmap(source: &JsValue, width: Option<u32>, height: Option<u32>) -> Result<ImageBitmap, String> {
    let options = ImageBitmapOptions::new();
    options.set_premultiply_alpha(PremultiplyAlpha::None);
    if let (Some(w), Some(h)) = (width, height) {
        options.set_resize_width(w);
        options.set_resize_height(h);
        options.set_resize_quality(ResizeQuality::High);
    }
    let promise: Promise = call_global("createImageBitmap", &[source, options.as_ref()])?.unchecked_into();
    Ok(JsFuture::from(promise)
        .await
        .map_err(|e| js_error("createImageBitmap", e))?
        .unchecked_into())
}

pub struct GlobeAssets {
    /// Flat `[x, y, z, ...]` tile centres; tile `id` is at `3 * (id - 1)`.
    pub positions: Vec<f32>,
    pub atlas_regions: HashMap<String, AtlasRegion>,
}

pub async fn load_data(base_url: &str) -> Result<GlobeAssets, String> {
    let coordinates_url = format!("{base_url}/coordinates.json");
    let atlas_url = format!("{base_url}/countries/atlas.json");
    let (coordinates, atlas) = futures::join!(fetch_bytes(&coordinates_url), fetch_bytes(&atlas_url));
    let coordinates: Coordinates =
        serde_json::from_slice(&coordinates?).map_err(|e| format!("coordinates.json: {e}"))?;
    let atlas_regions =
        serde_json::from_slice(&atlas?).map_err(|e| format!("atlas.json: {e}"))?;
    let mut positions = coordinates.positions;
    positions.truncate(coordinates.length * 3);
    Ok(GlobeAssets { positions, atlas_regions })
}

pub async fn load_atlas(base_url: &str) -> Result<ImageBitmap, String> {
    let blob = fetch_blob(&format!("{base_url}/countries/atlas.png")).await?;
    create_bitmap(&blob, None, None).await
}

/// Earth texture resized to `width` x `width / 2`, plus its mip chain (each level resized by the
/// browser from the previous one).
pub async fn load_earth(base_url: &str, width: u32) -> Result<Vec<ImageBitmap>, String> {
    let blob = fetch_blob(&format!("{base_url}/earth/3_no_ice_clouds_16k.jpg")).await?;
    let mut levels = vec![create_bitmap(&blob, Some(width), Some(width / 2)).await?];
    let (mut w, mut h) = (width, width / 2);
    while w > 1 && h > 1 {
        w /= 2;
        h /= 2;
        let previous: JsValue = levels.last().unwrap().clone().into();
        levels.push(create_bitmap(&previous, Some(w), Some(h)).await?);
    }
    Ok(levels)
}
