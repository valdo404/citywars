// The web app only targets wasm32; on native this library is empty so that
// `cargo check --workspace` works.
#![cfg(target_arch = "wasm32")]

mod app {
    mod countries;
}

mod backends {
    mod backend;
    mod fake_backend;
    mod http_backend;
}