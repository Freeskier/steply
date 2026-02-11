use crate::node::Node;
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::widgets::inputs::input::Input;
use crate::widgets::inputs::validators;
use crate::widgets::outputs::text::Text;

pub fn build_demo_flow() -> Flow {
    let step1 = Step::new(
        "step_collect_tags",
        "Step 1: Validation + Overlay Mirror Demo",
        vec![
            Node::Output(Box::new(Text::new(
                "desc_1",
                "Type tags. Ctrl+O opens overlay; Enter in overlay mirrors text into this field.",
            ))),
            Node::Output(Box::new(Text::new(
                "desc_1_ops",
                "Global text ops: Ctrl+Backspace / Ctrl+W (delete word left), Ctrl+Delete (delete word right).",
            ))),
            Node::Input(Box::new(
                Input::new("tags_raw", "Tags")
                    .with_validator(validators::required("Tags are required"))
                    .with_validator(validators::min_length(3, "Tags must be at least 3 characters")),
            )),
        ],
    )
    .with_hint("Tab / Shift+Tab navigate. Enter submits. Esc closes overlay or exits.");

    let step2 = Step::new(
        "step_confirm",
        "Step 2: Submit/Error Visibility Demo",
        vec![
            Node::Output(Box::new(Text::new(
                "desc_2",
                "Try submit on empty field to see inline errors.",
            ))),
            Node::Input(Box::new(
                Input::new("selected_tag", "Selected tag")
                    .with_validator(validators::required("Selected tag is required"))
                    .with_validator(validators::min_length(
                        2,
                        "Selected tag must be at least 2 characters",
                    )),
            )),
        ],
    )
    .with_hint("Errors are hidden while typing and shown on blur/submit.");

    Flow::new(vec![step1, step2])
}
