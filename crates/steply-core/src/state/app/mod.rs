use crate::core::NodeId;
use crate::state::flow::Flow;
use crate::state::validation::ValidationState;
use crate::task::TaskSetupError;
use crate::widgets::node::{Node, find_overlay};
use crate::widgets::traits::FocusMode;
use std::error::Error;
use std::fmt;

use self::state::{DataState, RuntimeState, ViewState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitConfirmChoice {
    Stay,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppStateInitError {
    InvalidTaskSetup(TaskSetupError),
}

impl fmt::Display for AppStateInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTaskSetup(err) => write!(f, "invalid task setup: {err}"),
        }
    }
}

impl Error for AppStateInitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidTaskSetup(err) => Some(err),
        }
    }
}

impl From<TaskSetupError> for AppStateInitError {
    fn from(value: TaskSetupError) -> Self {
        Self::InvalidTaskSetup(value)
    }
}

pub struct AppState {
    flow: Flow,
    ui: ViewState,
    data: DataState,
    runtime: RuntimeState,
    scratch_nodes: Vec<Node>,
    should_exit: bool,
    pending_back_confirm: Option<String>,
    pending_exit_confirm: Option<ExitConfirmChoice>,
}

impl AppState {
    pub(super) fn reconcile_current_step_visibility(&mut self) {
        if self.flow.is_empty() || self.step_visible_at(self.flow.current_index()) {
            return;
        }

        while self.flow.advance() {
            if self.step_visible_at(self.flow.current_index()) {
                return;
            }
        }

        while self.flow.go_back() {
            if self.step_visible_at(self.flow.current_index()) {
                return;
            }
        }
    }

    fn active_blocking_overlay_info(&self) -> Option<(NodeId, FocusMode)> {
        let entry = self.ui.overlays.active_blocking()?;
        Some((entry.id.clone(), entry.focus_mode))
    }

    fn overlay_has_persistent_children(&self, overlay_id: &str) -> bool {
        find_overlay(self.flow.current_step().nodes.as_slice(), overlay_id)
            .and_then(Node::persistent_children)
            .is_some()
    }

    pub fn validation_state(&self) -> &ValidationState {
        &self.runtime.validation
    }

    pub fn visible_error(&self, id: &str) -> Option<&str> {
        self.runtime.validation.visible_error(id)
    }

    pub fn is_hidden_invalid(&self, id: &str) -> bool {
        self.runtime.validation.is_hidden_invalid(id)
    }

    pub fn clear_step_errors(&mut self) {
        self.runtime.validation.clear_step_errors();
        self.runtime.validation.clear_step_warnings();
        self.runtime.validation.reset_warnings_acknowledged();
    }

    pub(super) fn refresh_validation_after_change(&mut self) {
        self.validate_focused_live();
        self.runtime.validation.reset_warnings_acknowledged();
    }

    pub fn current_step_errors(&self) -> &[String] {
        self.runtime.validation.step_errors()
    }

    pub fn current_step_warnings(&self) -> &[String] {
        self.runtime.validation.step_warnings()
    }
}

mod adapters;
mod derived;
mod effects;
mod exit;
mod flow;
mod input;
mod lifecycle;
mod overlay_access;
mod query;
mod state;
mod transaction;
mod validation_runtime;
mod value_sync;

#[cfg(test)]
mod tests;
