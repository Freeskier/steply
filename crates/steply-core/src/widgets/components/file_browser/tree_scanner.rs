use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, Receiver, Sender};

use crate::widgets::components::tree_view::TreeNode;

use super::FileTreeItem;
use super::async_utils::{drain_receiver, recv_latest};
use super::query::ScanResult;
use super::tree_builder::build_tree_nodes_for;

pub(super) struct TreeBuildRequest {
    pub seq: u64,
    pub browse_dir: PathBuf,
    pub show_parent_option: bool,
    pub selected_paths: Vec<PathBuf>,
    pub expanded_paths: HashSet<PathBuf>,
    pub cached_subtrees: std::collections::HashMap<PathBuf, Vec<TreeNode<FileTreeItem>>>,
    pub result: Arc<ScanResult>,
}

pub(super) struct TreeBuildResult {
    pub seq: u64,
    pub nodes: Vec<TreeNode<FileTreeItem>>,
}

pub(super) struct TreeScannerHandle {
    #[cfg(not(target_arch = "wasm32"))]
    tx: Sender<TreeBuildRequest>,
    #[cfg(not(target_arch = "wasm32"))]
    rx: Receiver<TreeBuildResult>,
}

impl TreeScannerHandle {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<TreeBuildRequest>();
        let (res_tx, res_rx) = mpsc::channel::<TreeBuildResult>();
        std::thread::spawn(move || worker(req_rx, res_tx));
        Self {
            tx: req_tx,
            rx: res_rx,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn new() -> Self {
        Self {}
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn submit(&self, req: TreeBuildRequest) {
        let _ = self.tx.send(req);
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn submit(&self, _req: TreeBuildRequest) {}

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn try_recv_all(&self) -> Vec<TreeBuildResult> {
        drain_receiver(&self.rx)
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn try_recv_all(&self) -> Vec<TreeBuildResult> {
        Vec::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn worker(rx: Receiver<TreeBuildRequest>, tx: Sender<TreeBuildResult>) {
    while let Some(req) = recv_latest(&rx) {
        let nodes = build_tree_nodes_for(
            req.browse_dir.as_path(),
            req.show_parent_option,
            req.result.as_ref(),
            req.selected_paths.as_slice(),
            &req.expanded_paths,
            &req.cached_subtrees,
        );

        let _ = tx.send(TreeBuildResult {
            seq: req.seq,
            nodes,
        });
    }
}
