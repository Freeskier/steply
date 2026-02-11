use std::collections::{HashMap, HashSet};

use super::cache::SearchKey;
use super::model::SearchResult;

pub(crate) struct SearchCache {
    cache: HashMap<SearchKey, SearchResult>,
    in_flight: HashSet<SearchKey>,
    last_applied: Option<SearchKey>,
}

impl SearchCache {
    pub(crate) fn new() -> Self {
        Self {
            cache: HashMap::new(),
            in_flight: HashSet::new(),
            last_applied: None,
        }
    }

    pub(crate) fn get(&self, key: &SearchKey) -> Option<SearchResult> {
        self.cache.get(key).cloned()
    }

    pub(crate) fn insert(&mut self, key: SearchKey, result: SearchResult) {
        self.cache.insert(key, result);
    }

    pub(crate) fn is_in_flight(&self, key: &SearchKey) -> bool {
        self.in_flight.contains(key)
    }

    pub(crate) fn mark_in_flight(&mut self, key: SearchKey) {
        self.in_flight.insert(key);
    }

    pub(crate) fn clear_in_flight(&mut self, key: &SearchKey) {
        self.in_flight.remove(key);
    }

    pub(crate) fn last_applied(&self) -> Option<&SearchKey> {
        self.last_applied.as_ref()
    }

    pub(crate) fn set_last_applied(&mut self, key: SearchKey) {
        self.last_applied = Some(key);
    }
}
