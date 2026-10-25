//! Offline backend for development, like the original `FakeBackend`: every tile starts owned by
//! France, and one robot per country clicks tiles at random intervals. Updates are delivered in
//! batches every `batch_update_duration_ms`.

use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::callback::Interval;
use rand::Rng;

use crate::backends::backend::{Update, UpdatesCallback};

/// Same tile range as the original fake backend.
const FAKE_TILE_COUNT: u32 = 257_000;
pub struct FakeBackend {
    bindings: RefCell<Vec<String>>,
    pending: RefCell<Vec<Update>>,
    callbacks: RefCell<Vec<UpdatesCallback>>,
}

impl FakeBackend {
    pub fn new(batch_update_duration_ms: u32, countries: Vec<String>) -> Rc<Self> {
        let backend = Rc::new(FakeBackend {
            bindings: RefCell::new(vec!["fr".to_string(); FAKE_TILE_COUNT as usize + 1]),
            pending: RefCell::new(Vec::new()),
            callbacks: RefCell::new(Vec::new()),
        });

        let flush = Rc::downgrade(&backend);
        Interval::new(batch_update_duration_ms, move || {
            if let Some(b) = flush.upgrade() {
                let updates = std::mem::take(&mut *b.pending.borrow_mut());
                if !updates.is_empty() {
                    for callback in b.callbacks.borrow().iter() {
                        callback(updates.clone());
                    }
                }
            }
        })
        .forget();

        let mut rng = rand::thread_rng();
        for country in countries {
            let mut tile_id: u32 = rng.gen_range(0..10_000);
            let gap: u32 = rng.gen_range(1..100);
            let period: u32 = rng.gen_range(1..1000);
            let robot = Rc::downgrade(&backend);
            Interval::new(period, move || {
                if let Some(b) = robot.upgrade() {
                    tile_id = (tile_id + gap) % FAKE_TILE_COUNT;
                    b.click_tile(tile_id.max(1), &country);
                }
            })
            .forget();
        }
        backend
    }

    pub fn click_tile(&self, tile_id: u32, country_id: &str) {
        let mut bindings = self.bindings.borrow_mut();
        let Some(slot) = bindings.get_mut(tile_id as usize) else { return };
        let previous = std::mem::replace(slot, country_id.to_string());
        self.pending.borrow_mut().push(Update {
            tile: tile_id,
            previous_country: Some(previous),
            new_country: country_id.to_string(),
        });
    }

    pub fn get_current_ownerships_by_batch(
        &self,
        batch_size: u32,
        max_index: u32,
        callback: &mut dyn FnMut(Vec<(u32, String)>),
    ) {
        let bindings = self.bindings.borrow();
        let max_index = max_index.min(FAKE_TILE_COUNT);
        let mut index = 1;
        while index < max_index {
            let end = (index + batch_size).min(max_index);
            callback((index..end).map(|i| (i, bindings[i as usize].clone())).collect());
            index = end;
        }
    }

    pub fn leaderboard(&self) -> Vec<(String, u32)> {
        let mut counts = std::collections::HashMap::<String, u32>::new();
        for code in self.bindings.borrow().iter().skip(1) {
            *counts.entry(code.clone()).or_default() += 1;
        }
        counts.into_iter().collect()
    }

    pub fn listen_for_updates_batch(&self, callback: UpdatesCallback) {
        self.callbacks.borrow_mut().push(callback);
    }
}
