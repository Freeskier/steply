use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::task::{TaskSpec, TaskSubscription};
use crate::widgets::inputs::slider::SliderInput;
use crate::widgets::node::Node;
use crate::widgets::outputs::progress::{
    Easing, ProgressOutput, ProgressStyle, ProgressTransition,
};
use crate::widgets::outputs::text::Text;

pub fn build_demo_flow() -> Flow {
    let step = Step::new(
        "step_slider_progress",
        "Demo: Slider -> Progress",
        vec![
            Node::Output(Box::new(Text::new(
                "demo_intro",
                "Move slider left/right in large steps. Progress interpolates smoothly.",
            ))),
            Node::Output(Box::new(
                ProgressOutput::new("demo_progress", "Progress")
                    .with_range(0.0, 100.0)
                    .with_unit("%")
                    .with_bar_width(40)
                    .with_style(ProgressStyle::ClassicLine)
                    .with_transition(ProgressTransition::Tween {
                        duration_ms: 450,
                        easing: Easing::Linear,
                    }),
            )),
            Node::Input(Box::new(
                SliderInput::new("demo_slider", "Slider", 0, 100)
                    .with_step(50)
                    .with_unit("%")
                    .with_change_target("demo_progress"),
            )),
        ],
    )
    .with_hint("Use Left/Right to move slider. Enter submits.");

    Flow::new(vec![step])
}

pub fn build_demo_tasks() -> (Vec<TaskSpec>, Vec<TaskSubscription>) {
    (Vec::new(), Vec::new())
}
