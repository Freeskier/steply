use crate::runtime::event::WidgetEvent;
use crate::runtime::scheduler::SchedulerCommand;

#[derive(Debug, Clone)]
pub enum Effect {
    EmitWidget(WidgetEvent),
    Schedule(SchedulerCommand),
    RequestRender,
}
