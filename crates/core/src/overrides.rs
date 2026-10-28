use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::CookieBucket;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideStore {
    pub map: HashMap<String, CookieBucket>,
}

impl OverrideStore {
    pub fn get(&self, host: &str, name: &str) -> Option<CookieBucket> {
        self.map
            .get(&key(host, Some(name)))
            .or_else(|| self.map.get(&key(host, None)))
            .copied()
    }

    pub fn set(&mut self, host: &str, name: Option<&str>, bucket: CookieBucket) {
        self.map.insert(key(host, name), bucket);
    }

    pub fn clear(&mut self, host: &str, name: Option<&str>) {
        self.map.remove(&key(host, name));
    }
}

fn key(host: &str, name: Option<&str>) -> String {
    match name {
        Some(name) => format!("{host}\0{name}"),
        None => host.to_string(),
    }
}
