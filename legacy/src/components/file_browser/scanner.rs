use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::cache::SearchKey;
use super::model::{EntryFilter, SearchMode, SearchResult};
use super::model::{filter_entries, list_dir, list_dir_recursive};
use super::parser::split_segments;
use super::search::{build_glob_options, list_dir_recursive_glob, options_from_query};

pub(crate) struct ScanRequest {
    pub(crate) key: SearchKey,
    pub(crate) dir: PathBuf,
    pub(crate) recursive: bool,
    pub(crate) query: String,
    pub(crate) display_root: PathBuf,
    pub(crate) hide_hidden: bool,
    pub(crate) show_relative: bool,
    pub(crate) show_info: bool,
    pub(crate) mode: SearchMode,
    pub(crate) entry_filter: EntryFilter,
    pub(crate) ext_filter: Option<HashSet<String>>,
}

pub(crate) struct ScannerHandle {
    tx: Sender<ScanRequest>,
}

impl ScannerHandle {
    pub(crate) fn new(result_tx: Sender<(SearchKey, SearchResult)>) -> Self {
        let (tx, rx) = mpsc::channel::<ScanRequest>();
        thread::spawn(move || worker(rx, result_tx));
        Self { tx }
    }

    pub(crate) fn submit(&self, request: ScanRequest) {
        let _ = self.tx.send(request);
    }
}

fn worker(rx: Receiver<ScanRequest>, result_tx: Sender<(SearchKey, SearchResult)>) {
    while let Ok(request) = rx.recv() {
        let result = match request.mode {
            SearchMode::Glob => {
                let mut entries = if request.recursive {
                    list_dir_recursive_glob(&request.dir, request.hide_hidden, &request.query)
                } else {
                    list_dir(&request.dir, request.hide_hidden)
                };
                entries =
                    filter_entries(entries, request.entry_filter, request.ext_filter.as_ref());
                let normalized = request.query.replace('\\', "/");
                let segments = split_segments(&normalized);
                let name_pattern = segments.last().cloned().unwrap_or_else(String::new);
                let _ = build_glob_options(
                    &mut entries,
                    &name_pattern,
                    Some(request.display_root.as_path()),
                    request.show_relative,
                    request.show_info,
                );
                SearchResult {
                    entries,
                    matches: Vec::new(),
                    display_root: Some(request.display_root.clone()),
                    show_relative: request.show_relative,
                    show_info: request.show_info,
                }
            }
            SearchMode::Fuzzy => {
                let entries = if request.recursive {
                    list_dir_recursive(&request.dir, request.hide_hidden)
                } else {
                    list_dir(&request.dir, request.hide_hidden)
                };
                let entries =
                    filter_entries(entries, request.entry_filter, request.ext_filter.as_ref());
                let (entries, _, matches) = options_from_query(
                    &entries,
                    &request.query,
                    Some(request.display_root.as_path()),
                    request.show_relative,
                    request.show_info,
                );
                SearchResult {
                    entries,
                    matches,
                    display_root: Some(request.display_root.clone()),
                    show_relative: request.show_relative,
                    show_info: request.show_info,
                }
            }
        };
        let _ = result_tx.send((request.key, result));
    }
}
