#![cfg(not(target_arch = "wasm32"))]

use crate::common::{ClickPlanetClient, DynError, TileCount, CLIENT_NAME, OwnershipResponse, generate_websocket_key};
use std::sync::Arc;
use std::time::Duration;
use futures::stream::BoxStream;
use futures::StreamExt;
use prost::Message;
use rand::Rng;
use reqwest::Client;
use serde_json::json;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clickplanet_proto::clicks;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
use tokio::time::sleep as tokio_sleep;
use url::Url;
use http::Request;
use tokio_tungstenite;
use tokio_tungstenite::tungstenite;

#[derive(Clone)]
pub struct NativeClickPlanetRestClient {
    client: Arc<Client>,
    host: String,
    port: u16,
    secure: bool,
}

#[derive(Clone)]
struct WebSocketConfig {
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

impl NativeClickPlanetRestClient {
    pub fn new(base_url: &str, port: u16, secure: bool) -> Self {
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(32)
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        let host = if base_url.contains("://") {
            let parts: Vec<&str> = base_url.split("://").collect();
            parts[1].to_string()
        } else {
            base_url.to_string()
        };

        Self {
            client: Arc::new(client),
            host,
            port,
            secure,
        }
    }

    async fn create_websocket_connection(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        let config = WebSocketConfig::default();

        let retry_strategy = ExponentialBackoff::from_millis(config.initial_interval.as_millis() as u64)
            .max_delay(config.max_interval)
            .map(jitter);

        let result = Retry::spawn(retry_strategy, || async {
            let ws_url = format!("{}://{}:{}/v2/ws/listen", if self.secure { "wss" } else { "ws" }, self.host, self.port);

            println!("Attempting WebSocket connection to {}", ws_url);
            
            let url = Url::parse(&ws_url).map_err(|e| Box::new(e) as DynError)?;
            
            let request = Request::builder()
                .uri(url.as_str())
                .header("User-Agent", CLIENT_NAME)
                .header("Origin", format!("{}://{}:{}", if self.secure { "https" } else { "http" }, self.host, self.port))
                .header("Host", self.host.clone())
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header("Sec-WebSocket-Key", generate_websocket_key())
                .body(())
                .map_err(|e| Box::new(e) as DynError)?;
            
            let (ws_stream, _) = tokio_tungstenite::connect_async(request).await
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
                        if let tungstenite::Message::Binary(data) = msg {
                            match clicks::UpdateNotification::decode(&data[..]) {
                                Ok(notification) => {
                                    println!("Received notification for tile {} => country {}", notification.tile_id, notification.country_id);
                                    Some(notification)
                                }
                                Err(e) => {
                                    eprintln!("Error decoding protobuf: {}", e);
                                    None
                                }
                            }
                        } else {
                            None
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
}

#[async_trait::async_trait]
impl ClickPlanetClient for NativeClickPlanetRestClient {
    async fn click_tile(&self, tile_id: u32, country_id: &str) -> Result<(), DynError> {
        let request = clicks::ClickRequest {
            tile_id: tile_id.try_into().unwrap(),
            country_id: country_id.to_string(),
        };

        let mut proto_bytes = Vec::new();
        request.encode(&mut proto_bytes)?;

        let base_url = self.host.clone();

        let client = self.client.clone();
        let protocol = if self.secure { "https" } else { "http" };
        let url = format!("{}://{}:{}/v2/click", protocol, base_url, self.port);

        // Send as JSON with base64 encoded protobuf
        let encoded = STANDARD.encode(&proto_bytes);
        let json_body = json!({
            "data": encoded
        });

        let response = client
            .post(&url)
            .header("User-Agent", CLIENT_NAME)
            .json(&json_body)
            .send()
            .await
            .map_err(|e| Box::new(e) as DynError)?;

        if !response.status().is_success() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("HTTP Error: {:?}", response.status()),
            )) as DynError);
        }

        Ok(())
    }

    async fn get_ownerships_by_batch(
        &self,
        start_tile_id: u32,
        end_tile_id: u32,
    ) -> Result<clicks::OwnershipState, DynError> {
        let protocol = if self.secure { "https" } else { "http" };
        let url = format!(
            "{}://{}:{}/v2/ownerships?start_tile_id={}&end_tile_id={}",
            protocol, self.host, self.port, start_tile_id, end_tile_id
        );

        let response = self.client.get(&url).header("User-Agent", CLIENT_NAME).send().await
            .map_err(|e| Box::new(e) as DynError)?;

        let ownership_response: OwnershipResponse = response.json().await
            .map_err(|e| Box::new(e) as DynError)?;
            
        let str_result = &ownership_response.data;

        let result = STANDARD.decode(str_result);

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

        let mut start_tile_id = 0;
        while start_tile_id <= max_tile_id {
            let end_tile_id = (start_tile_id + BATCH_SIZE).min(max_tile_id);

            let millis = rand::thread_rng().gen_range(300..=1000);
            tokio_sleep(Duration::from_millis(millis)).await;
        
            let result: Result<clicks::OwnershipState, DynError> = self.get_ownerships_by_batch(start_tile_id, end_tile_id).await;

            match result {
                Ok(batch_state) => {
                    final_state.ownerships.extend(batch_state.ownerships);
                }
                Err(e) => {
                    if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                        if let Some(status) = reqwest_err.status() {
                            eprintln!("HTTP Status: {}", status);
                            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                                eprintln!("Rate limit hit, waiting before retry...");
                                tokio_sleep(Duration::from_secs(5)).await;
                                continue; // Retry this batch
                            }
                        }
                        if reqwest_err.is_timeout() {
                            eprintln!("Request timed out");
                        }
                        if reqwest_err.is_connect() {
                            eprintln!("Connection error");
                        }
                    }

                    return Err(e);
                }
            }

            start_tile_id = end_tile_id + 1;
        }

        Ok(final_state)
    }

    async fn listen_for_updates(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        let stream = Box::pin(futures::stream::unfold((), move |_| {
            let client = self;
            async move {
                loop {
                    match client.create_websocket_connection().await {
                        Ok(stream) => return Some((stream, ())),
                        Err(e) => {
                            eprintln!("Error in WebSocket connection: {}. Retrying...", e);
                            tokio_sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        })
        .flat_map(|stream| stream));

        Ok(stream.boxed())
    }
}

// Using generate_websocket_key from common module
