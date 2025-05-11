#![cfg(target_arch = "wasm32")]

use crate::common::{ClickPlanetClient, DynError, TileCount, CLIENT_NAME, OwnershipResponse};
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
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageEvent, WebSocket};
use js_sys::{ArrayBuffer, Uint8Array};
use gloo_timers::future::sleep as gloo_sleep;

#[derive(Clone)]
pub struct WasmClickPlanetRestClient {
    client: Arc<Client>,
    host: String,
    port: u16,
    secure: bool,
}

impl WasmClickPlanetRestClient {
    pub fn new(base_url: &str, port: u16, secure: bool) -> Self {
        let client = Client::builder()
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

    async fn connect_websocket(&self) -> Result<BoxStream<'_, clicks::UpdateNotification>, DynError> {
        let ws_url = format!("{}://{}:{}/v2/ws/listen", if self.secure { "wss" } else { "ws" }, self.host, self.port);
        let ws = WebSocket::new(&ws_url).map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("WebSocket error: {:?}", e))) as DynError)?;
        
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
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
        
        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            let onopen = Closure::once_into_js(move || resolve.call0(&JsValue::NULL).unwrap_or_default());
            let onerror = Closure::once_into_js(move |e| reject.call1(&JsValue::NULL, &e).unwrap_or_default());
            
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        });
        
        JsFuture::from(promise).await
            .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("WebSocket connection error: {:?}", e))) as DynError)?;
        
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
        
        Ok(receiver.boxed())
    }
}

#[async_trait::async_trait(?Send)]
impl ClickPlanetClient for WasmClickPlanetRestClient {
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
            gloo_sleep(Duration::from_millis(millis)).await;
        
            let result: Result<clicks::OwnershipState, DynError> = self.get_ownerships_by_batch(start_tile_id, end_tile_id).await;

            match result {
                Ok(batch_state) => {
                    final_state.ownerships.extend(batch_state.ownerships);
                }
                Err(e) => {
                    if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                        if let Some(status) = reqwest_err.status() {
                            web_sys::console::error_1(&format!("HTTP Status: {}", status).into());
                            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                                web_sys::console::error_1(&"Rate limit hit, waiting before retry...".into());
                                gloo_sleep(Duration::from_secs(5)).await;
                                continue; // Retry this batch
                            }
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
        self.connect_websocket().await
    }
}
