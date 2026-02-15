use crate::core::value::Value;
use crate::state::flow::Flow;
use crate::state::step::Step;
use crate::task::{TaskSpec, TaskSubscription};
use crate::widgets::components::calendar::{Calendar, CalendarMode};
use crate::widgets::components::file_browser::FileBrowserInput;
use crate::widgets::components::object_editor::ObjectEditor;
use crate::widgets::components::searchable_select::SearchableSelect;
use crate::widgets::components::select_list::SelectList;
use crate::widgets::components::select_list::SelectMode;
use crate::widgets::components::snippet::Snippet;
use crate::widgets::components::tree_view::{TreeNode, TreeView};
use crate::widgets::inputs::array::ArrayInput;
use crate::widgets::inputs::button::ButtonInput;
use crate::widgets::inputs::checkbox::CheckboxInput;
use crate::widgets::inputs::choice::ChoiceInput;
use crate::widgets::inputs::color::ColorInput;
use crate::widgets::inputs::masked::MaskedInput;
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::slider::SliderInput;
use crate::widgets::inputs::text::{TextInput, TextMode};
use crate::widgets::node::Node;
use crate::widgets::outputs::chart::{ChartOutput, ChartRenderMode};
use crate::widgets::outputs::diff::DiffOutput;
use crate::widgets::outputs::progress::{
    Easing, ProgressOutput, ProgressStyle, ProgressTransition,
};
use crate::widgets::outputs::text::TextOutput;
use crate::widgets::validators;

// ── Snippet ───────────────────────────────────────────────────────────────────

fn step_snippet() -> Step {
    Step::new(
        "step_snippet",
        "Snippet",
        vec![Node::Component(Box::new(
            Snippet::new(
                "snip",
                "Snippet",
                "  Connect to <ip> on port <port>\n  as user <user> since <date> <port>",
            )
            .with_input(Node::Input(Box::new(MaskedInput::new(
                "ip",
                "IP",
                "###.###.###.###",
            ))))
            .with_input(Node::Input(Box::new(MaskedInput::new(
                "port",
                "Port",
                "#{1,5:1-65535}",
            ))))
            .with_input(Node::Input(Box::new(TextInput::new("user", "User"))))
            .with_input(Node::Input(Box::new(MaskedInput::new(
                "date",
                "Date",
                "DD/MM/YYYY",
            )))),
        ))],
    )
    .with_hint("Tab → next field  •  Shift+Tab → prev  •  Enter → next/submit")
}

// ── Calendar input ────────────────────────────────────────────────────────────

fn step_calendar() -> Step {
    Step::new(
        "step_calendar",
        "Calendar input",
        vec![Node::Component(Box::new(
            Calendar::new("cal_dt", "Date").with_mode(CalendarMode::DateTime),
        ))],
    )
    .with_hint("Tab → month/year/grid  •  ←→ change  •  ↑↓ navigate  •  Enter select")
}

// ── Step 1: Text inputs ──────────────────────────────────────────────────────

fn step_text_inputs() -> Step {
    let completions = vec![
        "alice".into(),
        "bob".into(),
        "carol".into(),
        "charlie".into(),
        "dave".into(),
        "eve".into(),
    ];

    Step::new(
        "step_text",
        "Text inputs",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "txt_intro",
                "Type freely, use Tab for completion on the username field.",
            ))),
            Node::Input(Box::new(
                TextInput::new("txt_name", "Full name")
                    .with_validator(validators::required("Name is required"))
                    .with_validator(validators::min_length(2, "At least 2 characters"))
                    .with_completion_items(vec!["test".to_string(), "teflon".to_string()]),
            )),
            Node::Input(Box::new(
                TextInput::new("txt_user", "Username")
                    .with_completion_items(completions)
                    .with_validator(validators::required("Username is required")),
            )),
            Node::Input(Box::new(
                TextInput::new("txt_pass", "Password")
                    .with_mode(TextMode::Password)
                    .with_validator(validators::min_length(6, "Minimum 6 characters")),
            )),
            Node::Input(Box::new(
                TextInput::new("txt_pass_hidden", "Secret token (hidden)")
                    .with_mode(TextMode::Secret),
            )),
        ],
    )
    .with_hint("Tab → completion  •  Enter → submit step")
}

// ── Step 2: Masked + Array ───────────────────────────────────────────────────

fn step_structured_inputs() -> Step {
    Step::new(
        "step_structured",
        "Structured inputs",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "struct_intro",
                "Masked input guides cursor through a fixed pattern. Array lets you add/remove items.",
            ))),
            Node::Input(Box::new(
                MaskedInput::new("masked_phone", "Phone", "+## (###) ###-##-##")
                    .with_validator(validators::required("Phone is required")),
            )),
            Node::Input(Box::new(
                MaskedInput::new("masked_date", "Date", "YYYY-mm-DD")
                    .with_validator(validators::required("Date is required")),
            )),
            Node::Input(Box::new(
                MaskedInput::new("masked_ip", "IP address", "#{1,3:0-255}.###.###.###"),
            )),
            Node::Input(Box::new(
                ArrayInput::new("arr_tags", "Tags")
                    .with_items(vec!["rust".into(), "tui".into()])
                    .with_validator(validators::required("At least one tag")),
            )),
        ],
    )
    .with_hint("Masked: type digits, cursor skips separators  •  Array: Enter → add, Del → remove")
}

// ── Step 3: Choice + Select + SearchableSelect ───────────────────────────────

fn step_selection() -> Step {
    let languages = vec![
        "Rust",
        "Go",
        "Python",
        "TypeScript",
        "Zig",
        "Haskell",
        "OCaml",
        "C",
        "C++",
        "Kotlin",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();

    let editors = vec!["Neovim", "Emacs", "VS Code", "Helix", "Sublime", "Zed"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

    Step::new(
        "step_selection",
        "Selection widgets",
        vec![
            Node::Input(Box::new(
                ChoiceInput::new(
                    "choice_os",
                    "Operating system",
                    vec![
                        "Linux".into(),
                        "macOS".into(),
                        "Windows".into(),
                        "BSD".into(),
                    ],
                )
                .with_bullets(true)
                .with_validator(validators::required("Pick one")),
            )),
            Node::Input(Box::new(
                SelectInput::new("sel_editor", "Editor", editors)
                    .with_validator(validators::required("Pick an editor")),
            )),
            Node::Component(Box::new(
                SearchableSelect::new("ss_lang", "Language (searchable)", languages)
                    .with_mode(SelectMode::Single)
                    .with_max_visible(6),
            )),
        ],
    )
    .with_hint("Choice: Up/Down  •  Select: Up/Down  •  SearchableSelect: type to filter")
}

// ── Step 4: Checkbox + multi-select list ─────────────────────────────────────

fn step_toggles() -> Step {
    let features = vec![
        "Dark mode",
        "Notifications",
        "Auto-update",
        "Telemetry",
        "Beta features",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();

    Step::new(
        "step_toggles",
        "Toggles & multi-select",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "tog_intro",
                "Checkboxes are single toggles. SelectList in Multi mode allows picking many.",
            ))),
            Node::Input(Box::new(
                CheckboxInput::new("chk_agree", "I agree to the terms")
                    .with_validator(validators::required("You must agree to continue")),
            )),
            Node::Input(Box::new(
                CheckboxInput::new("chk_newsletter", "Subscribe to newsletter").with_checked(true),
            )),
            Node::Component(Box::new(
                SelectList::from_strings("ms_features", "Enable features", features)
                    .with_mode(SelectMode::Multi),
            )),
        ],
    )
    .with_hint("Space → toggle checkbox  •  SelectList: Space → check, Enter → confirm")
}

// ── Step 5: Slider + Progress + Chart ────────────────────────────────────────

fn step_outputs() -> Step {
    Step::new(
        "step_outputs",
        "Outputs: slider → progress & chart",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "out_intro",
                "Move the sliders to see progress bar and chart update in real time.",
            ))),
            Node::Output(Box::new(
                ProgressOutput::new("prog_cpu", "CPU load")
                    .with_range(0.0, 100.0)
                    .with_unit("%")
                    .with_bar_width(36)
                    .with_style(ProgressStyle::BlockClassic)
                    .with_transition(ProgressTransition::Tween {
                        duration_ms: 300,
                        easing: Easing::OutCubic,
                    }),
            )),
            Node::Output(Box::new(
                ChartOutput::new("chart_hist", "History")
                    .with_mode(ChartRenderMode::Braille)
                    .with_capacity(40)
                    .with_range(0.0, 100.0)
                    .with_unit("%")
                    .with_gradient(true),
            )),
            Node::Input(Box::new(
                SliderInput::new("sld_cpu", "CPU %", 0, 100)
                    .with_step(5)
                    .with_unit("%")
                    .with_change_target("prog_cpu")
                    .with_change_target("chart_hist"),
            )),
            Node::Output(Box::new(
                ProgressOutput::new("prog_mem", "Memory")
                    .with_range(0.0, 100.0)
                    .with_unit(" MB")
                    .with_bar_width(36)
                    .with_style(ProgressStyle::ClassicLine)
                    .with_transition(ProgressTransition::Tween {
                        duration_ms: 3200,
                        easing: Easing::Linear,
                    }),
            )),
            Node::Input(Box::new(
                SliderInput::new("sld_mem", "Memory (MB)", 0, 100)
                    .with_step(100)
                    .with_unit(" MB")
                    .with_change_target("prog_mem"),
            )),
        ],
    )
    .with_hint("Left/Right → adjust  •  Shift+Left/Right → large step  •  Enter → submit")
}

// ── Step 6: Color picker ─────────────────────────────────────────────────────

fn step_color() -> Step {
    Step::new(
        "step_color",
        "Color picker",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "col_intro",
                "Pick a foreground and background color using hex input or channel sliders.",
            ))),
            Node::Input(Box::new(
                ColorInput::new("col_fg", "Foreground")
                    .with_rgb(220, 220, 220)
                    .with_validator(validators::required("Required")),
            )),
            Node::Input(Box::new(
                ColorInput::new("col_bg", "Background").with_rgb(30, 30, 46),
            )),
        ],
    )
    .with_hint("Tab between R/G/B channels  •  type hex or adjust with Up/Down")
}

// ── Step 7: File browser ─────────────────────────────────────────────────────

fn step_file_browser() -> Step {
    Step::new(
        "step_file_browser",
        "File browser",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "fb_intro",
                "Type a path directly (Tab for completion) or press Ctrl+Space to open the browser.",
            ))),
            Node::Component(Box::new(
                FileBrowserInput::new("fb_any", "Any file")
                    .with_validator(validators::required("Path is required"))
                    .with_browser_mode(crate::widgets::components::file_browser::BrowserMode::Tree),
            )),
            Node::Component(Box::new(
                FileBrowserInput::new("fb_rust", "Rust file")
                    .with_ext_filter(&["rs"])
                    .with_hide_hidden(false)
                    .with_recursive(true),
            )),
        ],
    )
    .with_hint("Tab → path completion  •  Ctrl+Space → browser  •  ← → navigate dirs  •  Enter → select")
}

// ── Step 8: Tree view ────────────────────────────────────────────────────────

fn step_tree_view() -> Step {
    // Build a small sample tree: project structure
    //  src/
    //    main.rs
    //    lib.rs
    //    widgets/
    //      mod.rs
    //      button.rs
    //  tests/
    //    integration.rs
    //  Cargo.toml

    let nodes: Vec<TreeNode<String>> = vec![
        TreeNode::new("src/".into(), 0, true).expanded(),
        TreeNode::new("main.rs".into(), 1, false),
        TreeNode::new("lib.rs".into(), 1, false),
        TreeNode::new("widgets/".into(), 1, true),
        TreeNode::new("mod.rs".into(), 2, false),
        TreeNode::new("button.rs".into(), 2, false),
        TreeNode::new("tests/".into(), 0, true).expanded(),
        TreeNode::new("integration.rs".into(), 1, false),
        TreeNode::new("Cargo.toml".into(), 0, false),
    ];

    Step::new(
        "step_tree",
        "Tree view",
        vec![
            Node::Output(Box::new(crate::widgets::outputs::text::TextOutput::new(
                "tree_intro",
                "Navigate a collapsible tree. Expand/collapse folders with → and ←.",
            ))),
            Node::Component(Box::new(
                TreeView::new("tree_files", "Project files", nodes).with_max_visible(3),
            )),
        ],
    )
    .with_hint("↑/↓ → navigate  •  → expand  •  ← collapse/jump to parent  •  Enter → select")
}

// ── Step 9: Object editor ────────────────────────────────────────────────────

fn step_object_editor() -> Step {
    let value = Value::from_json(
        r#"{
        "name": "Alice",
        "age": 30,
        "active": true,
        "address": {
            "city": "Warsaw",
            "zip": "00-001"
        },
        "tags": ["rust", "tui", "cli"]
    }"#,
    )
    .unwrap_or(Value::Object(Default::default()));

    Step::new(
        "step_object_editor",
        "Object editor",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "obj_intro",
                "Edit a structured value. Navigate with ↑/↓, edit with Enter/Tab, insert with i, delete with d, move with m.",
            ))),
            Node::Component(Box::new(
                ObjectEditor::new("obj_main", "Config")
                    .with_value(value)
                    .with_max_visible(12),
            )),
        ],
    )
    .with_hint("↑/↓ → navigate  •  Enter/Tab → edit  •  i → insert  •  d → delete  •  m → move")
}

// ── Step 10: Diff output ─────────────────────────────────────────────────────

fn step_diff() -> Step {
    let old = r#"fn main() {
    let name = "Alice";
    let age = 30;
    println!("Hello, {}!", name);
    println!("Age: {}", age);
}

fn greet(name: &str) {
    println!("Hi {}!", name);
}

fn farewell(name: &str) {
    println!("Bye {}!", name);
}

#[cfg(test)]
mod tests {
    fn test_greet() {
        assert!(true);
    }
}"#;

    let new = r#"fn main() {
    let name = "Bob";
    let age = 30;
    println!("Hello, {}!", name);
}

fn greet(name: &str, msg: &str) {
    println!("{}: {}!", name, msg);
}

fn farewell(name: &str) {
    println!("Bye {}!", name);
}

#[cfg(test)]
mod tests {
    fn test_greet() {
        assert!(true);
    }

    fn test_farewell() {
        assert!(true);
    }
}"#;

    Step::new(
        "step_diff",
        "Diff viewer",
        vec![Node::Component(Box::new(
            DiffOutput::new("diff_main", "main.rs", old, new).with_max_visible(18),
        ))],
    )
    .with_hint("↑↓ navigate  Tab next chunk  Shift+Tab prev  Enter expand gap")
}

// ── Step 11: Summary + button ─────────────────────────────────────────────────

fn step_finish() -> Step {
    Step::new(
        "step_finish",
        "All done!",
        vec![
            Node::Output(Box::new(TextOutput::new(
                "fin_text",
                "You have reached the end of the demo. Press the button below to finish.",
            ))),
            Node::Input(Box::new(
                ButtonInput::new("btn_finish", "Finish demo").with_text("  Finish  "),
            )),
        ],
    )
    .with_hint("Enter → activate button")
}

// ── Public API ───────────────────────────────────────────────────────────────

pub fn build_demo_flow() -> Flow {
    Flow::new(vec![
        step_snippet(),
        step_calendar(),
        step_diff(),
        step_object_editor(),
        step_file_browser(),
        step_tree_view(),
        step_text_inputs(),
        step_structured_inputs(),
        step_selection(),
        step_toggles(),
        step_outputs(),
        step_color(),
        step_finish(),
    ])
}

pub fn build_demo_tasks() -> (Vec<TaskSpec>, Vec<TaskSubscription>) {
    (Vec::new(), Vec::new())
}
