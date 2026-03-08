use super::{AppState, ExitConfirmChoice};

impl AppState {
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn back_confirm(&self) -> Option<&str> {
        self.pending_back_confirm.as_deref()
    }

    pub fn exit_confirm_choice(&self) -> Option<ExitConfirmChoice> {
        self.pending_exit_confirm
    }

    pub fn exit_confirm_active(&self) -> bool {
        self.pending_exit_confirm.is_some()
    }

    pub fn begin_exit_confirm(&mut self) {
        self.pending_exit_confirm = Some(ExitConfirmChoice::Stay);
    }

    pub fn cancel_exit_confirm(&mut self) {
        self.pending_exit_confirm = None;
    }

    pub fn toggle_exit_confirm_choice(&mut self) -> bool {
        let Some(choice) = self.pending_exit_confirm else {
            return false;
        };
        self.pending_exit_confirm = Some(match choice {
            ExitConfirmChoice::Stay => ExitConfirmChoice::Exit,
            ExitConfirmChoice::Exit => ExitConfirmChoice::Stay,
        });
        true
    }

    pub fn set_exit_confirm_choice(&mut self, choice: ExitConfirmChoice) -> bool {
        let Some(current) = self.pending_exit_confirm else {
            return false;
        };
        if current == choice {
            return false;
        }
        self.pending_exit_confirm = Some(choice);
        true
    }

    pub fn resolve_exit_confirm(&mut self) -> bool {
        let Some(choice) = self.pending_exit_confirm.take() else {
            return false;
        };
        if choice == ExitConfirmChoice::Exit {
            self.request_exit();
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
