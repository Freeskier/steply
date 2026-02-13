use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::task::{
    ConcurrencyPolicy, RerunPolicy, TaskAssign, TaskParse, TaskSpec, TaskSubscription, TaskTrigger,
};
use crate::widgets::components::searchable_select::SearchableSelect;
use crate::widgets::components::select_list::{SelectList, SelectMode};
use crate::widgets::inputs::array::ArrayInput;
use crate::widgets::inputs::button::ButtonInput;
use crate::widgets::inputs::checkbox::CheckboxInput;
use crate::widgets::inputs::choice::ChoiceInput;
use crate::widgets::inputs::color::ColorInput;
use crate::widgets::inputs::input::Input;
use crate::widgets::inputs::masked::MaskedInput;
use crate::widgets::inputs::password::PasswordInput;
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::slider::SliderInput;
use crate::widgets::node::Node;
use crate::widgets::outputs::text::Text;
use crate::widgets::validators;

pub fn build_demo_flow() -> Flow {
    let step = Step::new(
        "step_all_inputs",
        "Demo: All Inputs In One Step",
        vec![
            Node::Output(Box::new(Text::new(
                "demo_intro",
                "Single-step playground with every input type.",
            ))),
            Node::Output(Box::new(Text::new(
                "demo_ops",
                "Ops: Tab navigation, Enter submit, Ctrl+Backspace/Ctrl+Delete text actions.",
            ))),
            Node::Output(Box::new(Text::new(
                "demo_select_components_intro",
                "Components: SelectList (Space toggle, Enter submit) + SearchableSelect (type to fuzzy filter).",
            ))),
            Node::Output(Box::new(Text::new(
                "demo_task_intro",
                "Task demo: button runs `date +%H:%M:%S`; changing Text also reruns it via OnNodeValueChanged debounce (500ms).",
            ))),
            Node::Input(Box::new(Input::new("task_result_input", "Task Result"))),
            Node::Input(Box::new(
                ButtonInput::new("task_run_button", "Run Task")
                    .with_text("Run date task")
                    .with_task_id("demo_date_task"),
            )),
            Node::Component(Box::new(
                SelectList::from_strings(
                    "select_list_component",
                    "Select List (Multi)",
                    vec![
                        "Rust".to_string(),
                        "Go".to_string(),
                        "TypeScript".to_string(),
                        "Zig".to_string(),
                        "Python".to_string(),
                        "C++".to_string(),
                    ],
                )
                .with_mode(SelectMode::Multi)
                .with_max_visible(4),
            )),
            Node::Component(Box::new(
                SearchableSelect::new(
                    "searchable_select_component",
                    "Searchable Select",
                    vec![
                        "test".to_string(),
                        "teflon".to_string(),
                        "terminal".to_string(),
                        "template".to_string(),
                        "trace".to_string(),
                        "vector".to_string(),
                        "version".to_string(),
                    ],
                )
                .with_mode(SelectMode::Single)
                .with_max_visible(5),
            )),
            Node::Input(Box::new(
                Input::new("text_input", "Text")
                    .with_validator(validators::required("Text is required"))
                    .with_validator(validators::min_length(
                        3,
                        "Text must be at least 3 characters",
                    ))
                    .with_completion_items(vec![
                        "test".to_string(),
                        "teflon".to_string(),
                        "terminal".to_string(),
                    ]),
            )),
            Node::Input(Box::new(
                PasswordInput::new("password_input", "Password")
                    .with_validator(validators::required("Password is required"))
                    .with_validator(validators::min_length(6, "Password min length is 6")),
            )),
            Node::Input(Box::new(
                CheckboxInput::new("checkbox_input", "Checkbox").with_checked(true),
            )),
            Node::Input(Box::new(SelectInput::new(
                "select_input",
                "Select",
                vec![
                    "Alpha".to_string(),
                    "Bravo".to_string(),
                    "Charlie".to_string(),
                ],
            ))),
            Node::Input(Box::new(ChoiceInput::new(
                "choice_input",
                "Choice",
                vec![
                    "Small".to_string(),
                    "Medium".to_string(),
                    "Large".to_string(),
                ],
            ))),
            Node::Input(Box::new(
                SliderInput::new("slider_input", "Slider", 0, 100)
                    .with_step(5)
                    .with_unit("%"),
            )),
            Node::Input(Box::new(
                ArrayInput::new("array_input", "Array")
                    .with_items(vec!["rust".to_string(), "tui".to_string()]),
            )),
            Node::Input(Box::new(
                ColorInput::new("color_input", "Color").with_rgb(48, 140, 220),
            )),
            Node::Input(Box::new(MaskedInput::date_dd_mm_yyyy(
                "masked_date_input",
                "Masked Date (DD/MM/YYYY)",
            ))),
            Node::Input(Box::new(MaskedInput::new(
                "masked_custom_input",
                "Masked Custom (AA-####)",
                "AA-#{4}",
            ))),
            Node::Input(Box::new(ButtonInput::new("button_input", "Button").with_text("Run"))),
        ],
    )
    .with_hint("All inputs on one step. Submit validates all fields.");

    Flow::new(vec![step])
}

pub fn build_demo_tasks() -> (Vec<TaskSpec>, Vec<TaskSubscription>) {
    let tasks = vec![
        TaskSpec::exec("demo_date_task", "date", vec!["+%H:%M:%S".to_string()])
            .with_rerun_policy(RerunPolicy::Cooldown { ms: 1_000 })
            .with_concurrency_policy(ConcurrencyPolicy::Restart)
            .with_parse(TaskParse::RawText)
            .with_assign(TaskAssign::WidgetValue("task_result_input".to_string())),
    ];

    let subscriptions = vec![TaskSubscription::new(
        "demo_date_task",
        TaskTrigger::OnNodeValueChanged {
            node_id: "text_input".to_string(),
            debounce_ms: 500,
        },
    )];

    (tasks, subscriptions)
}
