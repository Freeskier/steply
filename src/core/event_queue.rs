use crate::event::Action;
use crate::terminal::KeyEvent;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Action(Action),
    LayerResult {
        layer_id: String,
        value: String,
        target_id: Option<String>,
    },
    RequestRerender,
    InputChanged {
        id: String,
        value: String,
    },
    FocusChanged {
        from: Option<String>,
        to: Option<String>,
    },
    Submitted,
}

#[derive(Debug, Clone)]
struct ScheduledEvent {
    due: Instant,
    event: AppEvent,
}

pub struct EventQueue {
    queue: VecDeque<AppEvent>,
    scheduled: Vec<ScheduledEvent>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            scheduled: Vec::new(),
        }
    }

    pub fn emit(&mut self, event: AppEvent) {
        self.queue.push_back(event);
    }

    pub fn emit_after(&mut self, event: AppEvent, delay: Duration) {
        self.scheduled.push(ScheduledEvent {
            due: Instant::now() + delay,
            event,
        });
    }

    pub fn cancel_clear_error_message(&mut self, id: &str) {
        self.queue.retain(|queued| match queued {
            AppEvent::Action(Action::ClearErrorMessage(queued_id)) => queued_id != id,
            _ => true,
        });
        self.scheduled.retain(|scheduled| match &scheduled.event {
            AppEvent::Action(Action::ClearErrorMessage(scheduled_id)) => scheduled_id != id,
            _ => true,
        });
    }

    pub fn next_ready(&mut self, now: Instant) -> Option<AppEvent> {
        self.move_due_to_queue(now);
        self.queue.pop_front()
    }

    fn move_due_to_queue(&mut self, now: Instant) {
        let mut due = Vec::new();
        self.scheduled.retain(|scheduled| {
            if scheduled.due <= now {
                due.push(scheduled.event.clone());
                false
            } else {
                true
            }
        });
        self.queue.extend(due);
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}
