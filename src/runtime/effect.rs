use crate::runtime::event::{SystemEvent, WidgetAction};
use crate::runtime::scheduler::SchedulerCommand;

#[derive(Debug, Clone)]
pub enum Effect {
    Action(WidgetAction),
    System(SystemEvent),
    Schedule(SchedulerCommand),
    RequestRender,
}
