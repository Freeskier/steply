use crate::node::Node;
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::widgets::inputs::input::Input;
use crate::widgets::outputs::text::Text;

pub fn build_demo_flow() -> Flow {
    let step1 = Step::new(
        "step_collect_tags",
        "Step 1: Provide tags",
        vec![
            Node::Output(Box::new(Text::new(
                "desc_1",
                "Enter tags separated by comma and press Enter on input.",
            ))),
            Node::Input(Box::new(Input::new("tags_raw", "Tags"))),
        ],
    )
    .with_hint("Tab / Shift+Tab navigate. Enter submits focused widget.");

    let step2 = Step::new(
        "step_confirm",
        "Step 2: Confirm",
        vec![
            Node::Output(Box::new(Text::new(
                "desc_2",
                "Second step input (no automatic bindings).",
            ))),
            Node::Input(Box::new(Input::new("selected_tag", "Selected tag"))),
        ],
    )
    .with_hint("Type and press Enter.");

    Flow::new(vec![step1, step2])
}
