use super::model::EntryFilter;
use super::search::ScanResult;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub dir: PathBuf,
    pub query: String,
    pub recursive: bool,
    pub hide_hidden: bool,
    pub entry_filter: EntryFilter,
}

pub struct ScanCache {
    results: HashMap<CacheKey, ScanResult>,
    in_flight: Option<CacheKey>,
}

impl ScanCache {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            in_flight: None,
        }
    }

    pub fn get(&self, key: &CacheKey) -> Option<&ScanResult> {
        self.results.get(key)
    }

    pub fn insert(&mut self, key: CacheKey, result: ScanResult) {
        if self.in_flight.as_ref() == Some(&key) {
            self.in_flight = None;
        }
        self.results.insert(key, result);
    }

    pub fn is_in_flight(&self, key: &CacheKey) -> bool {
        self.in_flight.as_ref() == Some(key)
    }

    pub fn mark_in_flight(&mut self, key: CacheKey) {
        self.in_flight = Some(key);
    }

    #[allow(dead_code)]
    pub fn clear_in_flight(&mut self) {
        self.in_flight = None;
    }
}
