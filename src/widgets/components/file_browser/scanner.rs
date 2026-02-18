use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::DisplayMode;
use super::async_utils::{drain_receiver, recv_latest};
use super::cache::CacheKey;
use super::model::{EntryFilter, filter_entries, list_dir, list_dir_recursive};
use super::search::{ScanResult, fuzzy_search, glob_search, list_dir_recursive_glob, plain_result};

pub struct ScanRequest {
    pub key: CacheKey,
    pub dir: PathBuf,
    pub query: String,
    pub recursive: bool,
    pub hide_hidden: bool,
    pub entry_filter: EntryFilter,
    pub ext_filter: Option<HashSet<String>>,
    pub is_glob: bool,
    pub display_mode: DisplayMode,
}

/// Handle to the background scanner thread.
/// Drop to shut it down (channel closes, worker exits on next recv).
pub struct ScannerHandle {
    tx: Sender<ScanRequest>,
    rx: Receiver<(CacheKey, Arc<ScanResult>)>,
}

impl ScannerHandle {
    pub fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<ScanRequest>();
        let (res_tx, res_rx) = mpsc::channel::<(CacheKey, Arc<ScanResult>)>();
        thread::spawn(move || worker(req_rx, res_tx));
        Self {
            tx: req_tx,
            rx: res_rx,
        }
    }

    pub fn submit(&self, request: ScanRequest) {
        let _ = self.tx.send(request);
    }

    /// Drain all completed scan results. Returns `None` when channel is empty.
    pub fn try_recv_all(&self) -> Vec<(CacheKey, Arc<ScanResult>)> {
        drain_receiver(&self.rx)
    }
}

fn worker(rx: Receiver<ScanRequest>, tx: Sender<(CacheKey, Arc<ScanResult>)>) {
    while let Some(req) = recv_latest(&rx) {
        let display_root = req.dir.clone();

        // `**` in a glob pattern always implies recursive traversal
        let glob_is_recursive = req.is_glob && req.query.contains("**");
        let entries = if req.is_glob && (req.recursive || glob_is_recursive) {
            list_dir_recursive_glob(&req.dir, req.hide_hidden, &req.query)
        } else if req.recursive {
            list_dir_recursive(&req.dir, req.hide_hidden)
        } else {
            list_dir(&req.dir, req.hide_hidden)
        };

        let entries = filter_entries(entries, req.entry_filter, req.ext_filter.as_ref());

        let result = if req.is_glob {
            glob_search(&entries, &req.query, &display_root, req.display_mode)
        } else if req.query.is_empty() {
            plain_result(&entries, &display_root, req.display_mode)
        } else {
            fuzzy_search(&entries, &req.query, &display_root, req.display_mode)
        };

        let _ = tx.send((req.key, Arc::new(result)));
    }
}
