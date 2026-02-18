use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::widgets::components::tree_view::TreeNode;

use super::FileTreeItem;
use super::async_utils::{drain_receiver, recv_latest};
use super::search::ScanResult;
use super::tree_builder::build_tree_nodes_for;

pub(super) struct TreeBuildRequest {
    pub seq: u64,
    pub browse_dir: PathBuf,
    pub show_parent_option: bool,
    pub result: Arc<ScanResult>,
}

pub(super) struct TreeBuildResult {
    pub seq: u64,
    pub nodes: Vec<TreeNode<FileTreeItem>>,
}

pub(super) struct TreeScannerHandle {
    tx: Sender<TreeBuildRequest>,
    rx: Receiver<TreeBuildResult>,
}

impl TreeScannerHandle {
    pub(super) fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<TreeBuildRequest>();
        let (res_tx, res_rx) = mpsc::channel::<TreeBuildResult>();
        thread::spawn(move || worker(req_rx, res_tx));
        Self {
            tx: req_tx,
            rx: res_rx,
        }
    }

    pub(super) fn submit(&self, req: TreeBuildRequest) {
        let _ = self.tx.send(req);
    }

    pub(super) fn try_recv_all(&self) -> Vec<TreeBuildResult> {
        drain_receiver(&self.rx)
    }
}

fn worker(rx: Receiver<TreeBuildRequest>, tx: Sender<TreeBuildResult>) {
    while let Some(req) = recv_latest(&rx) {
        let nodes = build_tree_nodes_for(
            req.browse_dir.as_path(),
            req.show_parent_option,
            req.result.as_ref(),
        );

        let _ = tx.send(TreeBuildResult {
            seq: req.seq,
            nodes,
        });
    }
}
