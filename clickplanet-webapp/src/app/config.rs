//! Runtime configuration: compile-time defaults that can be overridden with URL query parameters.
//!
//! - `CITYWARS_STATIC_SITE` (compile time): base URL of the static assets (textures, flags,
//!   `coordinates.json`).
//! - `CITYWARS_API_URL` (compile time, default `http://localhost:3000`) or `?api=<url>`: the
//!   citywars server.
//! - `CITYWARS_BACKEND` (compile time, default `http`) or `?backend=http|fake`: `fake` runs the
//!   offline backend with robot clicks, for development without a server.
//! - `CITYWARS_UPDATES` (compile time, default `sse`) or `?updates=sse|ws`: live updates over
//!   Server-Sent Events (`/v2/events`) or the websocket (`/v2/ws/listen`, for servers without SSE).
//! - `?renderer=auto|webgpu|webgl`: graphics backend (default `auto`: WebGPU when available, else
//!   WebGL2).
//! - `?stats`: shows the frame rate and renderer in a small HUD.

pub const STATIC_SITE: &str = env!("CITYWARS_STATIC_SITE");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Http,
    Fake,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererChoice {
    Auto,
    WebGpu,
    WebGl,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub api_url: String,
    pub backend: BackendKind,
    pub websocket_updates: bool,
    pub renderer: RendererChoice,
    pub show_stats: bool,
}

fn query_param(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(name)
}

fn has_query_param(name: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok())
        .map(|p| p.has(name))
        .unwrap_or(false)
}

impl Config {
    pub fn from_environment() -> Self {
        let api_url = query_param("api")
            .or_else(|| option_env!("CITYWARS_API_URL").map(str::to_string))
            .unwrap_or_else(|| "http://localhost:3000".to_string())
            .trim_end_matches('/')
            .to_string();
        let backend = match query_param("backend")
            .or_else(|| option_env!("CITYWARS_BACKEND").map(str::to_string))
            .as_deref()
        {
            Some("fake") => BackendKind::Fake,
            _ => BackendKind::Http,
        };
        let renderer = match query_param("renderer").as_deref() {
            Some("webgpu") => RendererChoice::WebGpu,
            Some("webgl") | Some("webgl2") => RendererChoice::WebGl,
            _ => RendererChoice::Auto,
        };
        let websocket_updates = matches!(
            query_param("updates")
                .or_else(|| option_env!("CITYWARS_UPDATES").map(str::to_string))
                .as_deref(),
            Some("ws") | Some("websocket")
        );
        Config {
            api_url,
            backend,
            websocket_updates,
            renderer,
            show_stats: has_query_param("stats"),
        }
    }
}

pub fn static_url(path: &str) -> String {
    format!("{}/{}", STATIC_SITE.trim_end_matches('/'), path.trim_start_matches('/'))
}
