use crate::app::command::Command;
use crate::domain::value::Value;
use crate::terminal::terminal::TerminalEvent;

pub type NodeId = String;

#[derive(Debug, Clone)]
pub enum WidgetEvent {
    ValueProduced { target: NodeId, value: Value },
    RequestSubmit,
    RequestFocus { target: NodeId },
    OpenLayer { layer_id: String },
    CloseLayer,
    RequestRender,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Terminal(TerminalEvent),
    Command(Command),
    Widget(WidgetEvent),
}
