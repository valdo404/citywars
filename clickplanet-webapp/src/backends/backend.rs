use std::collections::HashMap;

#[allow(dead_code)]
pub trait TileClicker {
    fn click_tile(&mut self, tile_id: u32, country_id: String);
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Ownerships {
    pub bindings: HashMap<u32, String>,
}

#[allow(dead_code)]
impl Ownerships {
    pub fn new() -> Self {
        Ownerships {
            bindings: HashMap::new(),
        }
    }

    pub fn set_binding(&mut self, tile_id: u32, country_id: String) {
        self.bindings.insert(tile_id, country_id);
    }

    pub fn get_bindings(&self) -> &HashMap<u32, String> {
        &self.bindings
    }
}

#[allow(dead_code)]
pub trait OwnershipsGetter {
    fn get_current_ownerships_by_batch(
        &self,
        batch_size: usize,
        max_index: usize,
        callback: Box<dyn Fn(Ownerships) + Send + Sync>,
    );
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Update {
    pub tile: u32,
    pub previous_country: Option<String>,
    pub new_country: String,
}

#[allow(dead_code)]
impl Update {
    pub fn new(tile: u32, previous_country: Option<String>, new_country: String) -> Self {
        Update {
            tile,
            previous_country,
            new_country,
        }
    }
}

#[allow(dead_code)]
pub trait UpdatesListener {
    fn listen_for_updates(&self, callback: Box<dyn Fn(Update) + Send + Sync>) -> ();

    fn listen_for_updates_batch(
        &mut self,
        callback: Box<dyn Fn(Vec<Update>) + Send + Sync>,
    ) -> ();
}
