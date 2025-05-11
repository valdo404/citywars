use futures::stream::BoxStream;
use prost::Message;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, DecodeError, Engine as _};
use clickplanet_proto::clicks;
use clickplanet_proto::clicks::*;
use futures::StreamExt;
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use reqwest::Client;

#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::tungstenite::http::Uri;

#[cfg(target_arch = "wasm32")]
use http::Uri;

#[cfg(not(target_arch = "wasm32"))]
use {
    tokio_tungstenite::connect_async,
    tokio_retry::strategy::{jitter, ExponentialBackoff},
    tokio_retry::Retry,
    tokio::time::sleep as tokio_sleep,
};

#[cfg(target_arch = "wasm32")]
use {
    wasm_bindgen::prelude::*,
    wasm_bindgen_futures::JsFuture,
    web_sys::{MessageEvent, WebSocket},
    js_sys::{ArrayBuffer, Uint8Array},
    gloo_timers::future::sleep as gloo_sleep,
};

#[cfg(not(target_arch = "wasm32"))]
type DynError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(target_arch = "wasm32")]
type DynError = Box<dyn std::error::Error>;

#[async_trait::async_trait(?Send)]
pub trait ClickPlanetClient {
    async fn click_tile(&self, tile_id: u32, country_id: &str) -> Result<(), DynError>;
    async fn get_ownerships_by_batch(&self, start_tile_id: u32, end_tile_id: u32) -> Result<clicks::OwnershipState, DynError>;
    async fn get_ownerships(&self, index_coordinates: &Arc<dyn TileCount + Send + Sync>) -> Result<clicks::OwnershipState, DynError>;
    async fn listen_for_updates(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError>;
}


pub trait TileCount {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone)]
pub struct WebSocketConfig {
    initial_interval: Duration,
    max_interval: Duration
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(60)
        }
    }
}

#[derive(Deserialize)]
struct OwnershipResponse {
    data: String,  // base64 encoded protobuf
}

#[derive(Clone)]
pub struct ClickPlanetRestClient {
    client: Arc<Client>,
    host: String,
    port: u16,
    secure: bool,
}

pub const CLIENT_NAME: &'static str = "clickplanet client owned by valdo404";

impl ClickPlanetRestClient {
    pub fn new(base_url: &str, port: u16, secure: bool) -> Self {
        let client = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                Client::builder()
                    .pool_idle_timeout(Duration::from_secs(30))
                    .pool_max_idle_per_host(32)
                    .timeout(Duration::from_secs(10))
                    .connect_timeout(Duration::from_secs(5))
                    .build()
                    .expect("Failed to create HTTP client")
            }
            #[cfg(target_arch = "wasm32")]
            {
                Client::builder()
                    .build()
                    .expect("Failed to create HTTP client")
            }
        };

        Self {
            client: Arc::new(client),
            host: base_url.to_string(),
            port,
            secure: secure,
        }
    }


    #[cfg(not(target_arch = "wasm32"))]
    async fn connect_websocket(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        let stream = Box::pin(futures::stream::unfold((), move |_| {
            let client = self;
            async move {
                loop {
                    match Self::create_websocket_stream(client).await {
                        Ok(stream) => return Some((stream, ())),
                        Err(e) => {
                            eprintln!("Error in WebSocket connection: {}. Retrying...", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        })
        .flat_map(|stream| stream));

        Ok(stream.boxed())
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn create_websocket_stream(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        let config = WebSocketConfig::default();

        let retry_strategy = ExponentialBackoff::from_millis(config.initial_interval.as_millis() as u64)
            .max_delay(config.max_interval)
            .map(jitter);

        let result = Retry::spawn(retry_strategy, || async {
            let ws_url = format!("{}://{}:{}/v2/ws/listen", if self.secure { "wss" } else { "ws" }, self.host, self.port);

            let url = ws_url.parse::<Uri>().map_err(|e| Box::new(e) as DynError)?;

            println!("Attempting WebSocket connection...");
            let (ws_stream, _) = connect_async(url).await
                .map_err(|e| {
                    println!("Connection attempt failed: {:?}", e);
                    Box::new(e) as DynError
                })?;

            println!("Successfully connected to WebSocket");

            Ok::<_, DynError>(ws_stream)
        }).await?;

        let (_, read) = result.split();
        let stream = read
            .filter_map(|message| async move {
                match message {
                    Ok(msg) => {
                        match clicks::UpdateNotification::decode(&*msg.into_data()) {
                            Ok(notification) => Some(notification),
                            Err(e) => {
                                eprintln!("Error decoding message: {}", e);
                                None
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("WebSocket message error: {}", e);
                        None
                    }
                }
            })
            .boxed();

        Ok(stream)
    }

    #[cfg(target_arch = "wasm32")]
    async fn connect_websocket(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        // Create a WebSocket connection
        let ws_url = format!("{}://{}:{}/v2/ws/listen", if self.secure { "wss" } else { "ws" }, self.host, self.port);
        let ws = WebSocket::new(&ws_url).map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("WebSocket error: {:?}", e))) as DynError)?;
        
        // Set binary type to arraybuffer for protobuf messages
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        // Create onmessage callback
        let sender_clone = sender.clone();
        let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Ok(array_buffer) = e.data().dyn_into::<ArrayBuffer>() {
                let uint8_array = Uint8Array::new(&array_buffer);
                let data = uint8_array.to_vec();
                if let Ok(notification) = clicks::UpdateNotification::decode(&data[..]) {
                    sender_clone.unbounded_send(notification).unwrap_or_else(|e| {
                        web_sys::console::error_1(&format!("Error sending notification: {}", e).into());
                    });
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        
        // Wait for WebSocket to connect
        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            let onopen = Closure::once_into_js(move || resolve.call0(&JsValue::NULL).unwrap_or_default());
            let onerror = Closure::once_into_js(move |e| reject.call1(&JsValue::NULL, &e).unwrap_or_default());
            
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        });
        
        // Wait for connection
        JsFuture::from(promise).await
            .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("WebSocket connection error: {:?}", e))) as DynError)?;
        
        // Setup message handler
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget(); // Prevent closure from being dropped
        
        Ok(receiver.boxed())
    }

    #[cfg(target_arch = "wasm32")]
    async fn create_websocket_stream(&self) -> Result<WebSocket, DynError> {
        let config = WebSocketConfig::default();
        let mut retry_count = 0;
        let max_retries = 5;

        loop {
            let ws_url = format!("{}://{}:{}/v2/ws/listen", if self.secure { "wss" } else { "ws" }, self.host, self.port);

            let ws = match WebSocket::new(&ws_url) {
                Ok(ws) => ws,
                Err(e) => {
                    if retry_count >= max_retries {
                        return Err(format!("WebSocket error: {:?}", e).into());
                    }
                    let delay = config.initial_interval.as_millis() as u32 * (1 << retry_count);
                    gloo_sleep(Duration::from_millis(delay as u64)).await;
                    retry_count += 1;
                    continue;
                }
            };

            // Set binary type to arraybuffer for protobuf messages
            ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

            let promise = js_sys::Promise::new(&mut |resolve, reject| {
                let onopen = Closure::once_into_js(move || resolve.call0(&JsValue::NULL).unwrap_or_default());
                let onerror = Closure::once_into_js(move |e| reject.call1(&JsValue::NULL, &e).unwrap_or_default());

                ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
                ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            });

            match JsFuture::from(promise).await {
                Ok(_) => return Ok(ws),
                Err(e) => {
                    if retry_count >= max_retries {
                        return Err(format!("WebSocket error: {:?}", e).into());
                    }
                    let delay = config.initial_interval.as_millis() as u32 * (1 << retry_count);
                    gloo_sleep(Duration::from_millis(delay as u64)).await;
                    retry_count += 1;
                    continue;
                }
            }
        }
    }


    // Helper method used internally. Removed to fix compilation issues.

    // WASM implementation now directly uses connect_websocket
    // No need for a separate create_update_stream method
}

#[async_trait::async_trait(?Send)]
impl ClickPlanetClient for ClickPlanetRestClient {
    async fn click_tile(&self, tile_id: u32, country_id: &str) -> Result<(), DynError> {
        let request = clicks::ClickRequest {
            tile_id: tile_id.try_into().unwrap(),
            country_id: country_id.to_string(),
        };

        let mut proto_bytes = Vec::new();
        request.encode(&mut proto_bytes)?;

        let base_url = self.host.clone();

        let client = self.client.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let retry_strategy = ExponentialBackoff::from_millis(100)
                .max_delay(Duration::from_secs(5))
                .take(2)
                .map(jitter);

            let result = Retry::spawn(retry_strategy, || async {
                let response = client
                    .post(format!("{}://{}:{}/v2/rpc/click", if self.secure { "https" } else { "http" }, base_url, self.port))
                    .header("User-Agent", CLIENT_NAME)
                    .header("Content-Type", "application/json")
                    .header("Origin", format!("https://{}", base_url))
                    .header("Referer", format!("https://{}/", base_url))
                    .json(&json!({
                        "data": proto_bytes,
                    }))
                    .send()
                    .await
                    .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                if !response.status().is_success() {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Server returned error: {}", response.status())
                    )));
                }
                Ok(())
            }).await;

            match result {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e)),
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let response = client
                .post(format!("{}://{}:{}/v2/rpc/click", if self.secure { "https" } else { "http" }, base_url, self.port))
                .header("User-Agent", CLIENT_NAME)
                .header("Content-Type", "application/json")
                .header("Origin", format!("https://{}", base_url))
                .header("Referer", format!("https://{}/", base_url))
                .json(&json!({
                    "data": proto_bytes,
                }))
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Server returned error: {}", response.status())
                )));
            }
            Ok(())
        }

        
    }

    async fn get_ownerships_by_batch(
        &self,
        start_tile_id: u32,
        end_tile_id: u32,
    ) -> Result<clicks::OwnershipState, DynError> {
        let client = reqwest::Client::new();

        let batch_request = clicks::BatchRequest {
            start_tile_id: start_tile_id.try_into().unwrap(),
            end_tile_id: end_tile_id.try_into().unwrap(),
        };

        let mut proto_bytes = Vec::new();
        batch_request.encode(&mut proto_bytes)
            .map_err(|e| Box::new(e) as DynError)?;

        let payload = json!({
            "data": proto_bytes,
        });

        let response = client
            .post(format!("{}://{}:{}/v2/rpc/ownerships-by-batch", if self.secure { "https" } else { "http" }, self.host, self.port))
            .header("User-Agent", CLIENT_NAME)
            .header("Content-Type", "application/json")
            .header("Origin", format!("https://{}", self.host))
            .header("Referer", format!("https://{}/", self.host))
            .json(&payload)
            .send()
            .await
            .map_err(|e| Box::new(e) as DynError)?;

        let response_json: serde_json::Value = response.json().await
            .map_err(|e| Box::new(e) as DynError)?;

        let str_result = response_json["data"]
            .as_str()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid or missing data field in response"
            ))
            .map_err(|e| Box::new(e) as DynError)?;

        let result: Result<Vec<u8>, DecodeError> = STANDARD.decode(
            str_result
        );

        let proto_bytes: Vec<u8> = result.map_err(|e| Box::new(e) as DynError)?;

        let ownership_state = clicks::OwnershipState::decode(&proto_bytes[..])
            .map_err(|e| Box::new(e) as DynError)?;


        Ok(ownership_state)
    }

    async fn get_ownerships(
        &self,
        index_coordinates: &Arc<dyn TileCount + Send + Sync>,
    ) -> Result<clicks::OwnershipState, DynError> {
        const BATCH_SIZE: u32 = 10000;

        let max_tile_id = (index_coordinates.len() as u32) - 1;

        let mut final_state = clicks::OwnershipState {
            ownerships: Vec::new(),
        };

        let mut start_tile_id = 1;
        while start_tile_id <= max_tile_id {
            let end_tile_id = (start_tile_id + BATCH_SIZE).min(max_tile_id);

            let millis = rand::thread_rng().gen_range(300..=1000);
            #[cfg(not(target_arch = "wasm32"))]
            tokio_sleep(Duration::from_millis(millis)).await;
            #[cfg(target_arch = "wasm32")]
            gloo_sleep(Duration::from_millis(millis)).await;
        
            let result: Result<OwnershipState, DynError> = self.get_ownerships_by_batch(start_tile_id, end_tile_id).await;

            match result {
                Ok(batch_state) => {
                    final_state.ownerships.extend(batch_state.ownerships);
                },
                Err(e) => {
                    eprintln!("Error fetching batch {} to {}: {:?}", start_tile_id, end_tile_id, e);

                    if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                        if let Some(status) = reqwest_err.status() {
                            eprintln!("HTTP Status: {}", status);
                            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                                eprintln!("Rate limit hit, waiting before retry...");
                                #[cfg(not(target_arch = "wasm32"))]
                                tokio_sleep(Duration::from_secs(5)).await;
                                #[cfg(target_arch = "wasm32")]
                                gloo_sleep(Duration::from_secs(5)).await;
                                continue; // Retry this batch
                            }
                        }
                        if reqwest_err.is_timeout() {
                            eprintln!("Request timed out");
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        if reqwest_err.is_connect() {
                            eprintln!("Connection error");
                        }
                    }

                    return Err(e);
                }
            }

            start_tile_id += BATCH_SIZE;
        }

        Ok(final_state)
    }


    #[cfg(not(target_arch = "wasm32"))]
    async fn listen_for_updates(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        let stream = Box::pin(futures::stream::unfold((), move |_| {
            let client = self;
            async move {
                loop {
                    match client.connect_websocket().await {
                        Ok(stream) => return Some((stream, ())),
                        Err(e) => {
                            eprintln!("Error in WebSocket connection: {}. Retrying...", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        })
        .flat_map(|stream| stream));

        Ok(stream.boxed())
    }
    
    #[cfg(target_arch = "wasm32")]
    async fn listen_for_updates(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        // Simply call connect_websocket which already handles all the WebSocket setup
        self.connect_websocket().await
    }
}

pub fn generate_websocket_key() -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let mut key = [0u8; 16];
    rng.fill(&mut key);
    STANDARD.encode(key)
}