// Expose backend modules
pub mod backend;
pub mod fake_backend;
pub mod http_backend;

// Re-export common types from backend
pub use backend::{TileClicker, Ownerships, OwnershipsGetter, Update, UpdatesListener};
