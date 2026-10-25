//! The globe runtime: handles protocol messages, owns the camera, the tile state and the renderer,
//! and runs the frame loop.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use clickplanet_globe_protocol::{
    decode, CameraCommand, GlobeSettings, GlobeToUi, InitParams, PointerInput, PointerKind, PointerType, UiToGlobe,
};
use glam::{Vec2, Vec3};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlCanvasElement, OffscreenCanvas};

use super::animation_loop::start_animation;
use super::assets::{self, AtlasRegion};
use super::endpoint::Endpoint;
use super::setup_webgpu::{earth_texture_width, CanvasTarget, GpuContext, Renderer, TILE_DIAMETER};
use crate::core::controls::OrbitControls;
use crate::core::picking::TileIndex;
use crate::core::tiles::{CountryTable, TileState};

/// Pointer movement (CSS px) under which a press/release is a click rather than a drag.
const CLICK_SLOP_MOUSE: f32 = 5.0;
const CLICK_SLOP_TOUCH: f32 = 10.0;
/// Extra picking tolerance around a tile disc (CSS px).
const PICK_SLOP_MOUSE: f32 = 2.0;
const PICK_SLOP_TOUCH: f32 = 8.0;
/// The original rendered at device pixel ratio 1; cap it to keep phones fast.
const MAX_DEVICE_PIXEL_RATIO: f32 = 2.0;

struct Pointer {
    id: i32,
    position: Vec2,
}

#[derive(Default)]
struct Gesture {
    pointers: Vec<Pointer>,
    start: Vec2,
    moved: bool,
    pinch_distance: f32,
}

struct Loaded {
    renderer: Renderer,
    index: TileIndex,
    positions: Vec<f32>,
    atlas: HashMap<String, AtlasRegion>,
}

pub struct Globe {
    endpoint: Endpoint,
    controls: OrbitControls,
    settings: GlobeSettings,
    gesture: Gesture,
    css_size: Vec2,
    device_pixel_ratio: f32,
    countries: CountryTable,
    tiles: TileState,
    hovered: Option<u32>,
    player_country: Option<String>,
    loaded: Option<Loaded>,
    /// Canvas given to `run` (main-thread mode), used when `Init` carries none.
    default_canvas: Option<wasm_bindgen::JsValue>,
    /// Messages that need the tiles, received before they were loaded.
    pending: Vec<UiToGlobe>,
    regions_dirty: bool,
    started: bool,
    ready_sent: bool,
    frames: u32,
    frame_time_ms: f64,
    last_frame: Option<f64>,
    last_stats: f64,
}

pub type SharedGlobe = Rc<RefCell<Globe>>;

impl Globe {
    pub fn new(endpoint: Endpoint, default_canvas: Option<wasm_bindgen::JsValue>) -> SharedGlobe {
        Rc::new(RefCell::new(Globe {
            endpoint,
            controls: OrbitControls::default(),
            settings: GlobeSettings::default(),
            gesture: Gesture::default(),
            css_size: Vec2::new(1.0, 1.0),
            device_pixel_ratio: 1.0,
            countries: CountryTable::default(),
            tiles: TileState::new(0),
            hovered: None,
            player_country: None,
            loaded: None,
            default_canvas,
            pending: Vec::new(),
            regions_dirty: false,
            started: false,
            ready_sent: false,
            frames: 0,
            frame_time_ms: 0.0,
            last_frame: None,
            last_stats: 0.0,
        }))
    }

    fn send(&self, message: GlobeToUi) {
        self.endpoint.send(&message);
    }

    fn fatal(&self, message: String) {
        log::error!("{message}");
        self.send(GlobeToUi::Error { message, fatal: true });
    }

    fn physical_size(&self) -> (u32, u32) {
        let dpr = self.device_pixel_ratio.clamp(1.0, MAX_DEVICE_PIXEL_RATIO);
        (
            (self.css_size.x * dpr).round().max(1.0) as u32,
            (self.css_size.y * dpr).round().max(1.0) as u32,
        )
    }

    fn update_regions(&mut self) {
        if let Some(loaded) = &self.loaded {
            let mut regions = vec![[0.0f32; 4]; self.countries.len()];
            for (index, region) in regions.iter_mut().enumerate() {
                if let Some(code) = self.countries.code_of(index as u32) {
                    if let Some(r) = loaded.atlas.get(code) {
                        *region = [r.x, r.y, r.width, r.height];
                    }
                }
            }
            loaded.renderer.set_regions(&regions);
            self.regions_dirty = false;
        } else {
            self.regions_dirty = true;
        }
    }

    fn pick(&self, position: Vec2, pointer_type: PointerType) -> Option<(u32, Vec3)> {
        let loaded = self.loaded.as_ref()?;
        let size = self.css_size;
        let ndc = Vec2::new(2.0 * position.x / size.x - 1.0, 1.0 - 2.0 * position.y / size.y);
        let camera = self.controls.frame(size.x / size.y);
        let hit = camera.pick_sphere(ndc)?;
        let slop = if pointer_type == PointerType::Mouse { PICK_SLOP_MOUSE } else { PICK_SLOP_TOUCH };
        let tolerance = TILE_DIAMETER / 2.0 + slop * camera.world_per_pixel(size.y);
        let tile = loaded.index.nearest(hit, tolerance)?;
        let i = (tile - 1) as usize * 3;
        let p = &loaded.positions[i..i + 3];
        Some((tile, Vec3::new(p[0], p[1], p[2])))
    }

    fn set_hovered(&mut self, tile: Option<u32>) {
        if tile != self.hovered {
            self.hovered = tile;
            self.send(GlobeToUi::Hovered { tile });
        }
    }

    fn click(&mut self, position: Vec2, pointer_type: PointerType) {
        let Some((tile, p)) = self.pick(position, pointer_type) else {
            return;
        };
        if let Some(index) = self.player_country.as_deref().and_then(|c| self.countries.index_of(c)) {
            // optimistic update, like the original front end
            self.tiles.set_owner(tile, index);
        }
        // three.js IcosahedronGeometry UV convention (u = atan2(z, -x) / 2pi + 0.5)
        let lat = p.y.clamp(-1.0, 1.0).asin().to_degrees();
        let lon = p.z.atan2(-p.x).to_degrees();
        self.send(GlobeToUi::Clicked { tile, lon, lat });
    }

    fn pointer(&mut self, input: PointerInput) {
        let position = Vec2::new(input.x, input.y);
        let slop = if input.pointer_type == PointerType::Mouse { CLICK_SLOP_MOUSE } else { CLICK_SLOP_TOUCH };
        let tracked = self.gesture.pointers.iter().position(|p| p.id == input.pointer_id);
        match input.kind {
            PointerKind::Down => {
                if tracked.is_none() {
                    self.gesture.pointers.push(Pointer { id: input.pointer_id, position });
                }
                match self.gesture.pointers.len() {
                    1 => {
                        self.gesture.start = position;
                        self.gesture.moved = false;
                    }
                    2 => {
                        self.gesture.pinch_distance =
                            self.gesture.pointers[0].position.distance(self.gesture.pointers[1].position);
                        self.gesture.moved = true;
                    }
                    _ => self.gesture.moved = true,
                }
            }
            PointerKind::Move => {
                let Some(i) = tracked else {
                    if input.pointer_type == PointerType::Mouse {
                        let tile = self.pick(position, input.pointer_type).map(|(t, _)| t);
                        self.set_hovered(tile);
                    }
                    return;
                };
                let previous = self.gesture.pointers[i].position;
                self.gesture.pointers[i].position = position;
                match self.gesture.pointers.len() {
                    1 => {
                        self.controls.rotate_by_pixels(position - previous, self.css_size.y);
                        if position.distance(self.gesture.start) > slop {
                            self.gesture.moved = true;
                        }
                    }
                    2 => {
                        let distance =
                            self.gesture.pointers[0].position.distance(self.gesture.pointers[1].position);
                        self.controls.pinch(self.gesture.pinch_distance, distance);
                        self.gesture.pinch_distance = distance;
                    }
                    _ => {}
                }
                if input.pointer_type == PointerType::Mouse {
                    let tile = self.pick(position, input.pointer_type).map(|(t, _)| t);
                    self.set_hovered(tile);
                }
            }
            PointerKind::Up | PointerKind::Cancel => {
                let Some(i) = tracked else { return };
                self.gesture.pointers.remove(i);
                let is_click = input.kind == PointerKind::Up && self.gesture.pointers.is_empty() && !self.gesture.moved;
                if is_click {
                    self.click(position, input.pointer_type);
                }
            }
            PointerKind::Leave => {
                if input.pointer_type == PointerType::Mouse && tracked.is_none() {
                    self.set_hovered(None);
                }
            }
        }
    }

    fn camera(&mut self, command: CameraCommand) {
        match command {
            CameraCommand::Reset => self.controls = OrbitControls::default(),
            CameraCommand::LookAt { lon, lat, zoom } => {
                let (lon, lat) = (lon.to_radians(), lat.to_radians());
                let p = Vec3::new(-lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
                self.controls.phi = p.y.clamp(-1.0, 1.0).acos();
                self.controls.theta = p.x.atan2(p.z);
                if let Some(zoom) = zoom {
                    self.controls.set_zoom(zoom);
                }
            }
        }
    }

    /// Messages that need the tiles to be loaded.
    fn apply_tiles_message(&mut self, message: UiToGlobe) {
        if let UiToGlobe::SetOwners { tiles, owners } = message {
            for (tile, owner) in tiles.into_iter().zip(owners) {
                self.tiles.set_owner(tile, owner as u32);
            }
        }
    }

    fn handle(globe: &SharedGlobe, message: UiToGlobe, canvas: Option<wasm_bindgen::JsValue>) {
        let mut g = globe.borrow_mut();
        match message {
            UiToGlobe::Init(params) => {
                if g.started {
                    return;
                }
                g.started = true;
                let canvas = canvas.or_else(|| g.default_canvas.clone());
                drop(g);
                match canvas.and_then(canvas_target) {
                    Some(target) => spawn_local(Globe::init(globe.clone(), params, target)),
                    None => globe.borrow().fatal("init without a canvas".into()),
                }
            }
            UiToGlobe::SetCountries { codes } => {
                g.countries = CountryTable::new(codes);
                g.update_regions();
            }
            m @ UiToGlobe::SetOwners { .. } => {
                if g.loaded.is_some() {
                    g.apply_tiles_message(m);
                } else {
                    g.pending.push(m);
                }
            }
            UiToGlobe::PlayerCountry { code } => g.player_country = code,
            UiToGlobe::Settings(settings) => g.settings = settings,
            UiToGlobe::Camera(command) => g.camera(command),
            UiToGlobe::Pointer(input) => g.pointer(input),
            UiToGlobe::Wheel { delta_y } => g.controls.wheel(delta_y),
            UiToGlobe::Resize { width, height, device_pixel_ratio } => {
                g.css_size = Vec2::new(width.max(1.0), height.max(1.0));
                g.device_pixel_ratio = device_pixel_ratio;
                let (w, h) = g.physical_size();
                if let Some(loaded) = g.loaded.as_mut() {
                    loaded.renderer.resize(w, h);
                }
            }
        }
    }

    pub fn on_message(globe: &SharedGlobe, bytes: &[u8], canvas: Option<wasm_bindgen::JsValue>) {
        match decode::<UiToGlobe>(bytes) {
            Ok(message) => Globe::handle(globe, message, canvas),
            Err(e) => {
                log::error!("{e}");
                globe.borrow().send(GlobeToUi::Error { message: e.to_string(), fatal: false });
            }
        }
    }

    async fn init(globe: SharedGlobe, params: InitParams, canvas: CanvasTarget) {
        let base = params.assets_base_url.trim_end_matches('/').to_string();
        let (width, height) = {
            let mut g = globe.borrow_mut();
            g.css_size = Vec2::new(params.width.max(1.0), params.height.max(1.0));
            g.device_pixel_ratio = params.device_pixel_ratio;
            g.settings = params.settings.clone();
            g.physical_size()
        };
        let gpu = match GpuContext::new(canvas, params.renderer, width, height).await {
            Ok(gpu) => gpu,
            Err(e) => return globe.borrow().fatal(e),
        };
        log::info!("globe renderer: {}", gpu.backend_name);
        let earth_width = earth_texture_width(gpu.max_texture_dimension, params.width.min(params.height));
        let data = async {
            let data = assets::load_data(&base).await;
            if let Ok(d) = &data {
                globe.borrow().send(GlobeToUi::TilesLoaded { tile_count: (d.positions.len() / 3) as u32 });
            }
            data
        };
        let (data, atlas, earth) = futures::join!(
            data,
            assets::load_atlas(&base),
            assets::load_earth(&base, earth_width),
        );
        let (data, atlas, earth) = match (data, atlas, earth) {
            (Ok(d), Ok(a), Ok(e)) => (d, a, e),
            (d, a, e) => {
                let error = [d.err(), a.err(), e.err()].into_iter().flatten().collect::<Vec<_>>().join("; ");
                return globe.borrow().fatal(error);
            }
        };

        let index = TileIndex::new(&data.positions);
        let renderer = Renderer::new(gpu, &data.positions, &earth, &atlas);
        {
            let mut g = globe.borrow_mut();
            g.tiles = TileState::new(data.positions.len() / 3);
            g.loaded = Some(Loaded {
                renderer,
                index,
                positions: data.positions,
                atlas: data.atlas_regions,
            });
            g.update_regions();
            for message in std::mem::take(&mut g.pending) {
                g.apply_tiles_message(message);
            }
        }

        let frame_globe = globe.clone();
        start_animation(move |now| {
            frame_globe.borrow_mut().frame(now);
            true
        });
    }

    fn frame(&mut self, now: f64) {
        let dt = self.last_frame.map(|t| ((now - t) / 1000.0) as f32).unwrap_or(0.0).clamp(0.0, 0.1);
        self.last_frame = Some(now);
        let start = js_sys::Date::now();

        self.controls.auto_rotate_enabled = self.settings.auto_rotate;
        self.controls.update(dt);
        if self.regions_dirty {
            self.update_regions();
        }
        let aspect = self.css_size.x / self.css_size.y;
        let camera = self.controls.frame(aspect);
        let dirty = self.tiles.take_dirty();
        let hovered = self.hovered;
        let tile_count = self.tiles.len() as u32;
        let Some(loaded) = self.loaded.as_mut() else { return };
        loaded.renderer.upload_owners(self.tiles.owners(), &dirty);
        loaded.renderer.render(&camera, hovered);
        let renderer_name = loaded.renderer.backend_name().to_string();

        if !self.ready_sent {
            self.ready_sent = true;
            self.last_stats = now;
            self.send(GlobeToUi::Ready { renderer: renderer_name, tile_count });
        }
        self.frames += 1;
        self.frame_time_ms += js_sys::Date::now() - start;
        if now - self.last_stats >= 1000.0 {
            let elapsed = (now - self.last_stats) / 1000.0;
            if self.settings.report_stats {
                self.send(GlobeToUi::Stats {
                    fps: (self.frames as f64 / elapsed) as f32,
                    frame_time_ms: (self.frame_time_ms / self.frames.max(1) as f64) as f32,
                });
            }
            self.frames = 0;
            self.frame_time_ms = 0.0;
            self.last_stats = now;
        }
    }
}

fn canvas_target(canvas: wasm_bindgen::JsValue) -> Option<CanvasTarget> {
    if canvas.is_instance_of::<HtmlCanvasElement>() {
        return Some(CanvasTarget::Html(canvas.unchecked_into()));
    }
    // OffscreenCanvas is not exposed on every global, so check structurally.
    if js_sys::Reflect::has(&canvas, &"transferToImageBitmap".into()).unwrap_or(false) {
        return Some(CanvasTarget::Offscreen(canvas.unchecked_into::<OffscreenCanvas>()));
    }
    None
}
