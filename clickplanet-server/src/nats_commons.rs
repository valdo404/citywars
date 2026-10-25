#[cfg(feature = "nats")]
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "nats")]
use async_nats::{jetstream, ConnectError};
#[cfg(feature = "nats")]
use async_nats::jetstream::Context;
use thiserror::Error;
use crate::click_persistence::{ClickRepositoryError, LeaderboardError};

pub const CLICK_SUBJECT_PREFIX: &'static str = "clicks.tile.";
pub const CLICK_STREAM_NAME: &'static str = "CLICKS";

#[derive(Clone, Debug)]
pub struct ConsumerConfig {
    pub consumer_name: String,
    pub ack_wait: Duration,
    pub max_deliver: i64,
    pub concurrent_processors: usize,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            consumer_name: "tile-state-processor".to_string(),
            ack_wait: Duration::from_secs(30),
            max_deliver: 3,
            concurrent_processors: 4,
        }
    }
}

#[derive(Error, Debug)]
pub enum PollingConsumerError {
    #[cfg(feature = "nats")]
    #[error("Failed to connect to NATS: {0}")]
    NatsConnection(#[from] ConnectError),
    #[error("Failed to process message: {0}")]
    Processing(String),
    #[error("Failed to decode protobuf: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),
    #[error("Failed to store data: {0}")]
    ClickPersistence(#[from] ClickRepositoryError),
    #[error("Failed to update leaderboard: {0}")]
    LeaderboardPersistence(#[from] LeaderboardError),
}

#[cfg(feature = "nats")]
pub async fn get_stream(jetstream: Arc<Context>) -> Result<jetstream::stream::Stream, PollingConsumerError> {
    jetstream.get_stream(CLICK_STREAM_NAME.to_string())
        .await
        .map_err(|e| PollingConsumerError::Processing(e.to_string()))
}
