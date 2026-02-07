use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::model::EntryFilter;
use super::search_cache::SearchCache;

pub(crate) struct SearchState {
    pub(crate) recursive_search: bool,
    pub(crate) hide_hidden: bool,
    pub(crate) show_relative_paths: bool,
    pub(crate) show_info: bool,
    pub(crate) entry_filter: EntryFilter,
    pub(crate) extension_filter: Option<HashSet<String>>,
    pub(crate) cache: SearchCache,
    pub(crate) spinner_index: usize,
    pub(crate) spinner_tick: u8,
    input_debounce: Option<Instant>,
}

impl SearchState {
    pub(crate) fn new() -> Self {
        Self {
            recursive_search: true,
            hide_hidden: true,
            show_relative_paths: false,
            show_info: false,
            entry_filter: EntryFilter::All,
            extension_filter: None,
            cache: SearchCache::new(),
            spinner_index: 0,
            spinner_tick: 0,
            input_debounce: None,
        }
    }

    pub(crate) fn mark_input_changed(&mut self) {
        self.input_debounce = Some(Instant::now());
    }

    pub(crate) fn debounce_pending(&self) -> bool {
        self.input_debounce.is_some()
    }

    pub(crate) fn take_debounce_if_elapsed(&mut self, threshold: Duration) -> bool {
        let Some(at) = self.input_debounce else {
            return false;
        };
        if at.elapsed() < threshold {
            return false;
        }
        self.input_debounce = None;
        true
    }

    pub(crate) fn tick_spinner(&mut self, is_searching: bool) -> bool {
        if is_searching {
            self.spinner_tick = self.spinner_tick.wrapping_add(1);
            if self.spinner_tick % 3 == 0 {
                self.spinner_index = self.spinner_index.wrapping_add(1);
                return true;
            }
            return false;
        }

        self.spinner_tick = 0;
        self.spinner_index = 0;
        false
    }
}
