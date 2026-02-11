use crate::array_input::ArrayInput;
use crate::button_input::ButtonInput;
use crate::checkbox_input::CheckboxInput;
use crate::choice_input::ChoiceInput;
use crate::color_input::ColorInput;
use crate::components::file_browser::FileBrowserState;
use crate::components::json_tree_component::JsonTreeComponent;
use crate::components::select_component::{SelectComponent, SelectMode};
use crate::components::table_component::{TableBorders, TableColumn, TableComponent};
use crate::components::tree_view_component::{
    TreeNode, TreeNodeKind, TreeScalar, TreeViewComponent,
};
use crate::core::step::Step;
use crate::core::step_builder::StepBuilder;
use crate::core::value::Value;
use crate::password_input::{PasswordInput, PasswordRender};
use crate::path_input::PathInput;
use crate::segmented_input::SegmentedInput;
use crate::select_input::SelectInput;
use crate::slider_input::SliderInput;
use crate::text_input::TextInput;
use crate::validators;
use std::sync::{Arc, Mutex};

pub fn build_demo_steps() -> (Vec<Step>, Option<Arc<Mutex<FileBrowserState>>>) {
    let file_browser_state = Arc::new(Mutex::new(FileBrowserState::new("plan_select")));
    let steps = vec![
        build_step_five(),
        build_step_four(),
        build_step_zero(file_browser_state.clone()),
        build_step_one(),
        build_step_two(),
        build_step_three(),
    ];
    (steps, Some(file_browser_state))
}

fn build_step_zero(file_browser_state: Arc<Mutex<FileBrowserState>>) -> Step {
    let component =
        crate::components::file_browser::FileBrowserInputComponent::from_state(file_browser_state)
            .with_label("Select plan:")
            .with_recursive_search(true)
            .with_max_visible(6)
            // .with_entry_filter(crate::components::file_browser::EntryFilter::FilesOnly)
            // .with_extension_filter([".yml", ".yaml"])
            .with_relative_paths(true)
            .with_placeholder("Type to filter");

    let tags_component = SelectComponent::new("tags_select", Vec::new())
        .with_label("Tags (from input):")
        .with_mode(SelectMode::Multi)
        .bind_to_input("tags");

    StepBuilder::new("Component demo:")
        .component(component)
        .input(ArrayInput::new("tags", "Tags"))
        .input(ButtonInput::new("cta", "Continue").with_text("Continue"))
        .component(tags_component)
        .input(TextInput::new("username", "Username"))
        .build()
}

fn build_step_one() -> Step {
    let tags_component = SelectComponent::new("tags_select", Vec::new())
        .with_label("Tags (from input):")
        .with_mode(SelectMode::Multi)
        .bind_to_input("tags");

    StepBuilder::new("Please fill the form:")
        .hint("Press Tab/Shift+Tab to navigate, Enter to submit, Esc to exit")
        .input(
            TextInput::new("username", "Username")
                .with_validator(validators::required())
                .with_validator(validators::min_length(3)),
        )
        .input(
            TextInput::new("email", "Email")
                .with_validator(validators::required())
                .with_validator(validators::email()),
        )
        .input(ColorInput::new("accent_color", "Accent Color").with_rgb(64, 120, 200))
        .input(CheckboxInput::new("tos", "Accept Terms").with_checked(true))
        .input(
            ChoiceInput::new(
                "plan",
                "Plan",
                vec!["Free".to_string(), "Pro".to_string(), "Team".to_string()],
            )
            .with_bullets(true),
        )
        .input(ArrayInput::new("tags", "Tags"))
        .component(tags_component)
        .input(PathInput::new("path", "Path"))
        .input(SelectInput::new(
            "color",
            "Color",
            vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
        ))
        .build()
}

fn build_step_two() -> Step {
    StepBuilder::new("Almost there:")
        .input(
            TextInput::new("password", "Password")
                .with_validator(validators::required())
                .with_validator(validators::min_length(8)),
        )
        .build()
}

fn build_step_three() -> Step {
    StepBuilder::new("Additional inputs:")
        .hint("Try arrows left/right in select, and masked input")
        .input(
            PasswordInput::new("new_password", "New Password")
                .with_render_mode(PasswordRender::Stars)
                .with_validator(validators::required())
                .with_validator(validators::min_length(8)),
        )
        .input(SliderInput::new("volume", "Volume", 1, 20))
        .input(SegmentedInput::ipv4("ip_address", "IP Address"))
        .input(SegmentedInput::phone_us("phone", "Phone"))
        .input(SegmentedInput::number("num", "Number"))
        .input(SegmentedInput::date_dd_mm_yyyy("birthdate", "Birth Date"))
        .build()
}

fn build_step_four() -> Step {
    let mut config_root = TreeNode::object(Some("config".to_string()));
    config_root.children = vec![
        TreeNode::text(Some("app".to_string()), "steply"),
        TreeNode {
            id: 0,
            key: Some("retries".to_string()),
            kind: TreeNodeKind::Value(TreeScalar::Number("3".to_string())),
            expanded: false,
            children: Vec::new(),
        },
        TreeNode {
            id: 0,
            key: Some("debug".to_string()),
            kind: TreeNodeKind::Value(TreeScalar::Bool(false)),
            expanded: false,
            children: Vec::new(),
        },
    ];

    let mut tags = TreeNode::array(Some("tags".to_string()));
    tags.children = vec![
        TreeNode::text(None, "cli"),
        TreeNode::text(None, "interactive"),
        TreeNode::text(None, "demo"),
    ];

    let tree_component = TreeViewComponent::new("tree_demo").with_nodes(vec![config_root, tags]);

    let mut json_component = JsonTreeComponent::new("json_demo").bind_to_input("json_payload");
    let _ = json_component.set_json(
        r#"{"service":"steply","version":"0.1.0","flags":{"dry_run":false},"ports":[8080,8081]}"#,
    );

    StepBuilder::new("Tree / JSON components:")
        .hint(
            "Tree: Tab key/value, Ctrl+A child, Ctrl+I sibling, Ctrl+D delete. JSON: Ctrl+S export.",
        )
        .component(tree_component)
        .component(json_component)
        .input(TextInput::new("json_payload", "JSON output"))
        .build()
}

fn build_step_five() -> Step {
    let columns = vec![
        TableColumn::new("name", "Name", |input_id| {
            Box::new(TextInput::new(input_id, ""))
        })
        .with_min_width(14),
        TableColumn::new("port", "Port", |input_id| {
            Box::new(TextInput::new(input_id, ""))
        })
        .with_min_width(8),
        TableColumn::new("ssl", "SSL", |input_id| {
            Box::new(CheckboxInput::new(input_id, ""))
        })
        .with_min_width(5),
        TableColumn::new("color", "Color", |input_id| {
            Box::new(ColorInput::new(input_id, ""))
        })
        .with_min_width(10),
        TableColumn::new("weight", "Weight", |input_id| {
            Box::new(SliderInput::new(input_id, "", 0, 100).with_step(5))
        })
        .with_min_width(8),
    ];

    let table = TableComponent::new("servers_table", columns)
        .with_title("Table: Servers")
        .with_row_count(3)
        .with_cell_value(0, "name", Value::Text("api-main".to_string()))
        .with_cell_value(0, "port", Value::Text("443".to_string()))
        .with_cell_value(0, "ssl", Value::Bool(true))
        .with_cell_value(0, "color", Value::Text("#2EA043".to_string()))
        .with_cell_value(0, "weight", Value::Number(72))
        .with_cell_value(1, "name", Value::Text("worker-a".to_string()))
        .with_cell_value(1, "port", Value::Text("8080".to_string()))
        .with_cell_value(1, "ssl", Value::Bool(false))
        .with_cell_value(1, "color", Value::Text("#D29922".to_string()))
        .with_cell_value(1, "weight", Value::Number(40))
        .with_borders(TableBorders {
            outer: true,
            between_cells: true,
        });

    StepBuilder::new("Table component:")
        .hint("Arrows move, Tab next, Shift+Tab prev, Ctrl+A row, Ctrl+D row, Ctrl+S export.")
        .component(table)
        .build()
}
