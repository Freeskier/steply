use super::{AppState, ExitConfirmChoice, ExitConfirmMode, ExitConfirmState};

impl AppState {
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn back_confirm(&self) -> Option<&str> {
        self.pending_back_confirm.as_deref()
    }

    pub fn exit_confirm_choice(&self) -> Option<ExitConfirmChoice> {
        self.pending_exit_confirm.map(|state| state.choice)
    }

    pub fn exit_confirm_mode(&self) -> Option<ExitConfirmMode> {
        self.pending_exit_confirm.map(|state| state.mode)
    }

    pub fn exit_confirm_active(&self) -> bool {
        self.pending_exit_confirm.is_some()
    }

    pub fn begin_exit_confirm(&mut self) {
        self.pending_exit_confirm = Some(ExitConfirmState {
            mode: ExitConfirmMode::ExitApplication,
            choice: ExitConfirmChoice::Stay,
        });
    }

    pub fn begin_completion_confirm(&mut self) {
        self.pending_exit_confirm = Some(ExitConfirmState {
            mode: ExitConfirmMode::FinishFlow,
            choice: ExitConfirmChoice::Stay,
        });
    }

    pub fn cancel_exit_confirm(&mut self) {
        self.pending_exit_confirm = None;
    }

    pub fn toggle_exit_confirm_choice(&mut self) -> bool {
        let Some(state) = self.pending_exit_confirm else {
            return false;
        };
        self.pending_exit_confirm = Some(ExitConfirmState {
            mode: state.mode,
            choice: match state.choice {
                ExitConfirmChoice::Stay => ExitConfirmChoice::Exit,
                ExitConfirmChoice::Exit => ExitConfirmChoice::Stay,
            },
        });
        true
    }

    pub fn set_exit_confirm_choice(&mut self, choice: ExitConfirmChoice) -> bool {
        let Some(current) = self.pending_exit_confirm else {
            return false;
        };
        if current.choice == choice {
            return false;
        }
        self.pending_exit_confirm = Some(ExitConfirmState {
            mode: current.mode,
            choice,
        });
        true
    }

    pub fn resolve_exit_confirm(&mut self) -> bool {
        let Some(state) = self.pending_exit_confirm.take() else {
            return false;
        };
        if state.choice == ExitConfirmChoice::Exit {
            match state.mode {
                ExitConfirmMode::ExitApplication => self.request_exit(),
                ExitConfirmMode::FinishFlow => self.finalize_flow_exit(),
            }
        }
        true
    }

    pub fn request_exit(&mut self) {
        self.pending_exit_confirm = None;
        self.should_exit = true;
        if matches!(
            self.flow.current_status(),
            crate::state::step::StepStatus::Active | crate::state::step::StepStatus::Running
        ) {
            self.flow.cancel_current();
        }
        crate::task::engine::cancel_interval_tasks(self);
        self.cancel_all_running_tasks();
        self.runtime.queued_task_requests.clear();
    }
}
