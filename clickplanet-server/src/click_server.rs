#[cfg(all(feature = "nats", feature = "nonats"))]
compile_error!(
    "features `nats` and `nonats` are mutually exclusive: build the single-process mode with \
     `--no-default-features --features nonats`"
);

mod click_service;
mod nats_commons;
mod sse_events;
mod telemetry;
mod redis_click_persistence;
mod ownership_service;
mod click_persistence;
mod in_memory_click_persistence;

use crate::click_service::ClickService;
#[cfg(feature = "nats")]
use crate::click_service::get_or_create_jet_stream;
use axum::{
    extract::{Json, Query, State},
    extract::ws::{Message as WebsocketMessage},
    extract::ws::{WebSocket},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    routing::get,
    Router,
};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use serde_json::{json, Value};
use tokio;
use tokio::net::TcpListener;
use tracing::{error};
use base64::{encode};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use std::{time::Duration};
use axum::extract::WebSocketUpgrade;
use axum::http::header::{ACCEPT, CONTENT_TYPE};
use axum::http::HeaderMap;
use axum::http::HeaderName;
use axum::http::{Method, Request};
use axum::serve::Serve;
use prost::Message;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use clickplanet_proto::clicks::{Click, UpdateNotification};
use clickplanet_proto::clicks::{LeaderboardResponse, LeaderboardEntry};

use crate::click_persistence::{ClickRepository, LeaderboardRepository, LeaderboardOnClicks, LeaderboardMaintainer};
use crate::in_memory_click_persistence::{PapayaClickRepository};
use crate::nats_commons::ConsumerConfig;
use crate::ownership_service::OwnershipUpdateService;
use crate::redis_click_persistence::{RedisClickRepository};
use crate::telemetry::{init_telemetry, TelemetryConfig};

#[derive(Debug, Serialize, Deserialize)]
struct ClickPayload {
    data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BatchRequestPayload {
    data: Vec<u8>,
}

#[derive(Clone)]
struct AppState<T: ClickRepository + Send + Sync> {
    click_service: Arc<ClickService>,
    click_repository: Arc<T>,
    leaderboard_repo: Arc<dyn LeaderboardRepository>,
    update_notifification_broadcaster: Arc<Sender<UpdateNotification>>,
    ownership_update_service: Arc<OwnershipUpdateService>,
}


#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[cfg(feature = "nats")]
    #[arg(long, env = "NATS_URL", default_value = "nats://localhost:4222")]
    nats_url: String,

    #[cfg(feature = "nats")]
    #[arg(long, env = "REDIS_URL", default_value = "redis://localhost:6379")]
    redis_url: String,

    /// Redis for durable state (e.g. Memorystore). Without it the state lives in memory only and
    /// is lost on restart.
    #[cfg(not(feature = "nats"))]
    #[arg(long, env = "REDIS_URL")]
    redis_url: Option<String>,

    /// Interval of the SSE heartbeat comments.
    #[arg(long, env = "SSE_HEARTBEAT_SECS", default_value = "20")]
    sse_heartbeat_secs: u64,

    /// Number of recent events kept to resume SSE clients (`Last-Event-ID`).
    #[arg(long, env = "SSE_BUFFER", default_value = "50000")]
    sse_buffer: usize,

    #[arg(long, env = "OTEL_EXPORTER_OTLP_ENDPOINT", default_value = "http://localhost:4317")]
    otlp_endpoint: String,

    #[arg(long, env = "SERVICE_NAME", default_value = "clickplanet-server")]
    service_name: String,

    #[arg(long, env = "PORT", default_value = "3000")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    init_telemetry(TelemetryConfig {
        otlp_endpoint: args.otlp_endpoint.clone(),
        service_name: args.service_name.clone(),
    }).await?;

    run(&args).await?;

    Ok(())
}

async fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let (click_sender, _) = broadcast::channel(100000);
    let click_sender_ref = Arc::new(click_sender);

    let (update_notification_sender, _) = broadcast::channel(100000);
    let update_sender_ref: Arc<Sender<UpdateNotification>> = Arc::new(update_notification_sender);

    #[cfg(feature = "nats")]
    let (click_repository, leaderboard_repo, update_service, click_service) = {
        let cold_repository: Arc<RedisClickRepository> = Arc::new(RedisClickRepository::new(args.redis_url.as_str()).await?);
        let papaya_honey = PapayaClickRepository::populate_with(cold_repository).await?;

        let leaderboard_repo: Arc<dyn LeaderboardRepository> = Arc::new(LeaderboardOnClicks(papaya_honey.clone()));
        let click_repository: Arc<PapayaClickRepository> = Arc::new(papaya_honey.clone());

        let jetstream = Arc::new(get_or_create_jet_stream(args.nats_url.as_str()).await?);

        let update_service = Arc::new(OwnershipUpdateService::new(
            click_repository.clone(),
            click_repository.clone(),
            click_sender_ref.clone(),
            update_sender_ref.clone(),
            jetstream.clone(),
            Some(ConsumerConfig {
                concurrent_processors: 2,
                ack_wait: Duration::from_secs(20),
                ..Default::default()
            })
        ));
        let click_service = Arc::new(ClickService::new(jetstream.clone(), click_sender_ref.clone()).await.unwrap());
        (click_repository, leaderboard_repo, update_service, click_service)
    };

    // Single process: clicks go through the in-memory broadcast; Redis, when configured, restores
    // the state at start-up and keeps a durable copy of every click.
    #[cfg(not(feature = "nats"))]
    let (click_repository, leaderboard_repo, update_service, click_service) = {
        let cold_repository: Option<Arc<dyn ClickRepository>> = match &args.redis_url {
            Some(url) => Some(Arc::new(RedisClickRepository::new(url.as_str()).await?)),
            None => {
                tracing::warn!("no REDIS_URL: the state is kept in memory only and lost on restart");
                None
            }
        };
        let papaya_honey = match &cold_repository {
            Some(cold) => PapayaClickRepository::populate_with(cold.clone()).await?,
            None => PapayaClickRepository::new(),
        };
        let leaderboard_repo: Arc<dyn LeaderboardRepository> = Arc::new(LeaderboardOnClicks(papaya_honey.clone()));
        let click_repository: Arc<PapayaClickRepository> = Arc::new(papaya_honey.clone());
        let update_service = Arc::new(OwnershipUpdateService::new_local(
            click_repository.clone(),
            click_repository.clone(),
            click_sender_ref.clone(),
            update_sender_ref.clone(),
            Some(ConsumerConfig { concurrent_processors: 2, ..Default::default() }),
            cold_repository,
        ));
        let click_service = Arc::new(ClickService::new_local(click_sender_ref.clone()));
        (click_repository, leaderboard_repo, update_service, click_service)
    };

    // SSE: every update notification and click, with ids for Last-Event-ID resumption
    let hub = sse_events::EventHub::new(args.sse_buffer, 100_000);
    spawn_sse_feed(hub.clone(), update_sender_ref.subscribe(), click_sender_ref.subscribe());

    let state = AppState {
        click_service,
        click_repository: click_repository.clone(),
        leaderboard_repo: leaderboard_repo.clone(),
        update_notifification_broadcaster: update_sender_ref.clone(),
        ownership_update_service: update_service.clone(),
    };

    let app = Router::new()
        .route("/api/click", post(handle_click))
        .route("/v2/rpc/click", post(handle_click))
        .route("/api/ownerships-by-batch", post(handle_get_ownerships_by_batch))
        .route("/v2/rpc/ownerships-by-batch", post(handle_get_ownerships_by_batch))
        .route("/v2/rpc/ownerships", get(handle_get_ownerships))
        .route("/v2/rpc/leaderboard", get(handle_get_leaderboard))
        .route("/ws/listen", get(handle_ws_upgrade))
        .route("/v2/ws/listen", get(handle_ws_upgrade))
        .with_state(state)
        .merge(sse_events::router(hub, Duration::from_secs(args.sse_heartbeat_secs.max(1))))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([CONTENT_TYPE, HeaderName::from_static("last-event-id")])
        )
        .layer(TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                tracing::info_span!(
                    "http-request",
                    method = ?request.method(),
                    uri = ?request.uri(),
                    version = ?request.version(),
                )
            })
        );

    let listener = TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    println!("Server listening on 0.0.0.0:{}", args.port);

    let server: Serve<Router, Router> = axum::serve(listener, app);
    let update_service_clone = update_service.clone();
    let update_service_handle = update_service_clone.run();

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                error!("Server error: {:?}", e);
                return Err(e.to_string().into());
            } else {
                error!("Unexpected server exit");
            }
        }
        result = update_service_handle => {
            if let Err(e) = result {
               error!("Ownership update service error: {:?}", e);
               return Err(e.to_string().into());
            } else {
               error!("Unexpected update service exit");
            }
        }
    }

    Ok(())
}

/// Publishes the broadcast updates and clicks on the SSE hub.
fn spawn_sse_feed(
    hub: Arc<sse_events::EventHub>,
    mut updates: Receiver<UpdateNotification>,
    mut clicks: Receiver<Click>,
) {
    let update_hub = hub.clone();
    tokio::spawn(async move {
        loop {
            match updates.recv().await {
                Ok(update) => {
                    update_hub.publish("update", &update.encode_to_vec());
                }
                Err(broadcast::error::RecvError::Lagged(n)) => error!("SSE feed lagged by {n} updates"),
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    tokio::spawn(async move {
        loop {
            match clicks.recv().await {
                Ok(click) => {
                    hub.publish("click", &click.encode_to_vec());
                }
                Err(broadcast::error::RecvError::Lagged(n)) => error!("SSE feed lagged by {n} clicks"),
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

async fn handle_click<T: ClickRepository>(
    State(state): State<AppState<T>>,
    Json(payload): Json<ClickPayload>,
) -> Result<impl IntoResponse, StatusCode> {
    let click_request = clickplanet_proto::clicks::ClickRequest::decode(Bytes::from(payload.data))
        .map_err(|_| {
            StatusCode::BAD_REQUEST
        })?;

    tokio::time::timeout(
        Duration::from_secs(10),
        state.click_service.process_click(click_request)
    )
        .await
        .map_err(|e| {
            error!("Timeout error while clicking: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map_err(|e| {
            error!("Error while processing click: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;


    Ok(axum::Json(serde_json::json!({})))
}

async fn handle_get_ownerships<T: ClickRepository>(
    State(state): State<AppState<T>>,
) -> Result<Json<Value>, StatusCode> {
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        state.click_repository.get_ownerships(),
    )
        .await
        .map_err(|e| {
            error!("Timeout error while calling get_ownerships: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map_err(|e| {
            error!("Error while processing get_ownerships: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut response_bytes = Vec::new();

    response
        .encode(&mut response_bytes)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let base64_data = encode(&response_bytes);

    let payload = json!({
        "data": base64_data,
    });

    Ok(axum::Json(payload))
}

async fn handle_get_ownerships_by_batch<T: ClickRepository>(
    State(state): State<AppState<T>>,
    Json(payload): Json<BatchRequestPayload>,
) -> Result<Json<Value>, StatusCode> {
    let batch_request = clickplanet_proto::clicks::BatchRequest::decode(Bytes::from(payload.data))
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let response = tokio::time::timeout(
        Duration::from_secs(5),
        state.click_repository.get_ownerships_by_batch(
            batch_request.start_tile_id as u32,
            batch_request.end_tile_id as u32,
        ),
    )
        .await
        .map_err(|e| {
            error!("Timeout error while calling get_ownerships_by_batch: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map_err(|e| {
            error!("Error while processing get_ownerships_by_batch: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut response_bytes = Vec::new();

    response
        .encode(&mut response_bytes)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let base64_data = encode(&response_bytes);

    let payload = json!({
        "data": base64_data,
    });

    Ok(axum::Json(payload))
}


async fn handle_ws_upgrade<T: ClickRepository+ 'static>(
    ws: WebSocketUpgrade,
    State(state): State<AppState<T>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection<T: ClickRepository>(socket: WebSocket, state: AppState<T>) {
    let (sender, mut receiver) = socket.split();
    let sender_arc = Arc::new(Mutex::new(sender));

    let mut update_notification_subscription: Receiver<UpdateNotification> = state.update_notifification_broadcaster.subscribe();
    let sender_arc_clone = sender_arc.clone();

    let mut send_task = tokio::spawn(async move {
        while let Ok(notification) = update_notification_subscription.recv().await {
            let mut buf = Vec::new();
            if notification.encode(&mut buf).is_ok() {
                let mut sender = sender_arc.lock().await;
                if let Err(e) = sender.send(WebsocketMessage::Binary(buf)).await {
                    eprintln!("Error sending WebSocket message: {}", e);
                    break;
                }
            }
        }
    });


    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            match message {
                WebsocketMessage::Ping(payload) => {
                    let mut sender = sender_arc_clone.lock().await;
                    if let Err(e) = sender.send(WebsocketMessage::Pong(payload)).await {
                        eprintln!("Error sending pong: {}", e);
                        break;
                    }
                }
                WebsocketMessage::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

/// Response formats of `/v2/rpc/leaderboard`.
///
/// The default stays the original Clickplanet RPC envelope, `{"data": "<base64 protobuf>"}`, which
/// the web app and the original front end decode. The other formats are opt-in only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaderboardFormat {
    /// `{"data": "<base64 protobuf>"}` (default, whatever the `Accept` header otherwise says).
    Base64Envelope,
    /// Raw protobuf bytes, for `Accept: application/protobuf` (or `application/x-protobuf`).
    Protobuf,
    /// Readable JSON, `{"entries": [{"country_id", "score"}]}`, for `?format=json`.
    ReadableJson,
}

#[derive(Debug, Default, Deserialize)]
struct LeaderboardQuery {
    format: Option<String>,
}

impl LeaderboardFormat {
    fn negotiate(headers: &HeaderMap, query: &LeaderboardQuery) -> Self {
        if query.format.as_deref() == Some("json") {
            return LeaderboardFormat::ReadableJson;
        }
        let wants_protobuf = headers
            .get_all(ACCEPT)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .map(|media| media.split(';').next().unwrap_or("").trim().to_ascii_lowercase())
            .any(|media| media == "application/protobuf" || media == "application/x-protobuf");
        if wants_protobuf {
            LeaderboardFormat::Protobuf
        } else {
            LeaderboardFormat::Base64Envelope
        }
    }
}

fn leaderboard_response(response: &LeaderboardResponse, format: LeaderboardFormat) -> Response {
    match format {
        LeaderboardFormat::Base64Envelope => {
            let payload = json!({
                "data": encode(response.encode_to_vec()),
            });
            axum::Json(payload).into_response()
        }
        LeaderboardFormat::Protobuf => (
            [(CONTENT_TYPE, "application/protobuf")],
            response.encode_to_vec(),
        )
            .into_response(),
        LeaderboardFormat::ReadableJson => {
            let payload = json!({
                "entries": response.entries.iter().map(|entry| {
                    json!({
                        "country_id": entry.country_id,
                        "score": entry.score,
                    })
                }).collect::<Vec<_>>()
            });
            axum::Json(payload).into_response()
        }
    }
}

async fn handle_get_leaderboard<T: ClickRepository>(
    State(state): State<AppState<T>>,
    Query(query): Query<LeaderboardQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let format = LeaderboardFormat::negotiate(&headers, &query);

    let leaderboard_data = tokio::time::timeout(
        Duration::from_secs(5),
        state.leaderboard_repo.leaderboard(),
    )
        .await
        .map_err(|e| {
            error!("Timeout error while fetching leaderboard: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map_err(|e| {
            error!("Error while fetching leaderboard: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Convert HashMap to vec and sort by score in descending order
    let mut entries: Vec<_> = leaderboard_data
        .into_iter()
        .map(|(country_id, score)| LeaderboardEntry {
            country_id,
            score,
        })
        .collect();

    entries.sort_by(|a, b| b.score.cmp(&a.score));
    let response = LeaderboardResponse { entries };

    Ok(leaderboard_response(&response, format))
}

#[cfg(test)]
mod leaderboard_format_tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::HeaderValue;

    fn sample() -> LeaderboardResponse {
        LeaderboardResponse {
            entries: vec![
                LeaderboardEntry { country_id: "fr".to_string(), score: 3 },
                LeaderboardEntry { country_id: "us".to_string(), score: 1 },
            ],
        }
    }

    fn negotiate(accept: Option<&str>, format: Option<&str>) -> LeaderboardFormat {
        let mut headers = HeaderMap::new();
        if let Some(accept) = accept {
            headers.insert(ACCEPT, HeaderValue::from_str(accept).unwrap());
        }
        let query = LeaderboardQuery { format: format.map(str::to_string) };
        LeaderboardFormat::negotiate(&headers, &query)
    }

    async fn body_of(response: Response) -> (Option<String>, Vec<u8>) {
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_string());
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (content_type, body.to_vec())
    }

    #[test]
    fn default_is_the_base64_envelope() {
        assert_eq!(negotiate(None, None), LeaderboardFormat::Base64Envelope);
        assert_eq!(negotiate(Some("*/*"), None), LeaderboardFormat::Base64Envelope);
        assert_eq!(negotiate(Some("application/json"), None), LeaderboardFormat::Base64Envelope);
        assert_eq!(negotiate(Some("text/html,*/*;q=0.8"), None), LeaderboardFormat::Base64Envelope);
        assert_eq!(negotiate(None, Some("other")), LeaderboardFormat::Base64Envelope);
    }

    #[test]
    fn other_formats_are_opt_in() {
        assert_eq!(negotiate(Some("application/protobuf"), None), LeaderboardFormat::Protobuf);
        assert_eq!(
            negotiate(Some("application/json;q=0.5, application/x-protobuf"), None),
            LeaderboardFormat::Protobuf
        );
        assert_eq!(negotiate(None, Some("json")), LeaderboardFormat::ReadableJson);
    }

    #[tokio::test]
    async fn base64_envelope_is_unchanged() {
        let (content_type, body) = body_of(leaderboard_response(&sample(), LeaderboardFormat::Base64Envelope)).await;
        assert_eq!(content_type.as_deref(), Some("application/json"));
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json, json!({ "data": encode(sample().encode_to_vec()) }));
    }

    #[tokio::test]
    async fn protobuf_is_the_raw_message() {
        let (content_type, body) = body_of(leaderboard_response(&sample(), LeaderboardFormat::Protobuf)).await;
        assert_eq!(content_type.as_deref(), Some("application/protobuf"));
        assert_eq!(LeaderboardResponse::decode(body.as_slice()).unwrap(), sample());
    }

    #[tokio::test]
    async fn readable_json_lists_the_entries() {
        let (_, body) = body_of(leaderboard_response(&sample(), LeaderboardFormat::ReadableJson)).await;
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            json!({ "entries": [
                { "country_id": "fr", "score": 3 },
                { "country_id": "us", "score": 1 },
            ]})
        );
    }
}
