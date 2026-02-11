use crate::app::event::WidgetEvent;
use crate::app::scheduler::SchedulerCommand;

#[derive(Debug, Clone)]
pub enum Effect {
    EmitWidget(WidgetEvent),
    Schedule(SchedulerCommand),
    RequestRender,
}
