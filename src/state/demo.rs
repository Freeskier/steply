use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::state::validation::ValidationIssue;
use crate::widgets::components::filter_select::FilterSelect;
use crate::widgets::components::modal::Modal;
use crate::widgets::inputs::input::Input;
use crate::widgets::node::Node;
use crate::widgets::outputs::text::Text;
use crate::widgets::traits::OverlayPlacement;
use crate::widgets::validators;

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
                    .with_validator(validators::min_length(
                        3,
                        "Tags must be at least 3 characters",
                    )),
            )),
            Node::Input(Box::new(
                Input::new("dupa", "Dupa")
                    .with_validator(validators::required("dupa are required"))
                    .with_validator(validators::min_length(
                        4,
                        "dupa must be at least 3 characters",
                    )),
            )),
            Node::Component(Box::new(
                FilterSelect::new(
                    "tag_picker",
                    "Tag picker",
                    vec![
                        "alpha".to_string(),
                        "bravo".to_string(),
                        "charlie".to_string(),
                        "delta".to_string(),
                        "echo".to_string(),
                    ],
                )
                .with_max_visible(4)
                .with_submit_target("tags_raw"),
            )),
            Node::Component(Box::new(Modal::new(
                "demo_overlay",
                "Demo overlay",
                OverlayPlacement::new(3, 3, 46, 6),
                vec![
                    Node::Output(Box::new(Text::new(
                        "overlay_label",
                        "Overlay active. Enter copies value to tags_raw. Esc closes overlay.",
                    ))),
                    Node::Input(Box::new(
                        Input::new("overlay_input", "Overlay input")
                            .with_submit_target("tags_raw".to_string()),
                    )),
                ],
            ))),
        ],
    )
    .with_validator(Box::new(|ctx| {
        let tags = ctx.text("tags_raw").unwrap_or_default().trim();
        let dupa = ctx.text("dupa").unwrap_or_default().trim();

        if !tags.is_empty() && !dupa.is_empty() && tags.eq_ignore_ascii_case(dupa) {
            return vec![
                ValidationIssue::node("dupa", "Dupa must be different than Tags"),
                ValidationIssue::step("Tags and Dupa cannot be the same value"),
            ];
        }

        Vec::new()
    }))
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
    .with_validator(Box::new(|ctx| {
        if ctx
            .text("selected_tag")
            .is_some_and(|text| text.eq_ignore_ascii_case("forbidden"))
        {
            return vec![
                ValidationIssue::node("selected_tag", "Value 'forbidden' is not allowed"),
                ValidationIssue::step("Selected tag contains forbidden value"),
            ];
        }
        Vec::new()
    }))
    .with_hint("Errors are hidden while typing and shown on blur/submit.");

    Flow::new(vec![step1, step2])
}
