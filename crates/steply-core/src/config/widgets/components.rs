use crate::core::store_refs::{exact_template_expr, normalize_store_selector};
use crate::core::value::Value;
use crate::widgets::{
    components::{
        calendar::Calendar,
        command_runner::CommandRunner,
        file_browser::FileBrowserInput,
        object_editor::ObjectEditor,
        repeater::Repeater,
        select_list::{SelectItem, SelectList},
        snippet::Snippet,
        table::Table,
        textarea::TextAreaComponent,
        tree_view::{TreeNode, TreeView},
    },
    node::Node,
    shared::binding::ReadBinding,
};

use super::super::binding_compile::compile_read_binding_value;
use super::super::model::{
    CommandRunnerCommandDef, SelectListOptionDef, TableColumnDef, TreeNodeDef, ValidatorDef,
    WidgetDef,
};
use super::super::parse::{
    parse_browser_mode, parse_calendar_mode, parse_display_mode, parse_file_browser_entry_filter,
    parse_file_browser_selection_mode, parse_on_error, parse_repeater_entry_mode, parse_run_mode,
    parse_select_mode, parse_spinner_style, parse_table_style,
};
use super::super::utils::yaml_value_to_value;
use super::common::with_required_and_validators;
use super::compile_widget;
use super::embedded::compile_table_embedded_factory;

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_select_list(
    id: String,
    label: String,
    options: Vec<SelectListOptionDef>,
    mode: Option<String>,
    max_visible: Option<usize>,
    selected: Vec<usize>,
    show_label: Option<bool>,
) -> Result<Node, String> {
    let select_mode = parse_select_mode(mode.as_deref())?;
    let items = options
        .into_iter()
        .map(|option| match option {
            SelectListOptionDef::Plain(text) => SelectItem::plain(text),
            SelectListOptionDef::Detailed {
                value,
                title,
                description,
            } => SelectItem::detailed(value, title, description),
        })
        .collect::<Vec<_>>();
    let mut widget = SelectList::new(id, label, items)
        .with_mode(select_mode)
        .with_selected(selected);
    if let Some(max_visible) = max_visible {
        widget = widget.with_max_visible(max_visible);
    }
    if let Some(show_label) = show_label {
        widget = widget.with_show_label(show_label);
    }
    Ok(Node::Component(Box::new(widget)))
}

pub(super) fn compile_calendar(
    id: String,
    label: String,
    mode: Option<String>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut widget = Calendar::new(id, label).with_mode(parse_calendar_mode(mode.as_deref())?);
    widget = with_required_and_validators(widget, required, extra_validators);
    Ok(Node::Component(Box::new(widget)))
}

pub(super) fn compile_textarea(
    id: String,
    min_height: Option<usize>,
    max_height: Option<usize>,
    default: Option<String>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mut widget = TextAreaComponent::new(id);
    if let Some(min_height) = min_height {
        widget = widget.with_min_height(min_height);
    }
    if let Some(max_height) = max_height {
        widget = widget.with_max_height(max_height);
    }
    if let Some(default) = default {
        widget = widget.with_default(Value::Text(default));
    }
    widget = with_required_and_validators(widget, required, extra_validators);
    Ok(Node::Input(Box::new(widget)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_command_runner(
    id: String,
    label: String,
    run_mode: Option<String>,
    on_error: Option<String>,
    advance_on_success: Option<bool>,
    visible_lines: Option<usize>,
    spinner_style: Option<String>,
    timeout_ms: Option<u64>,
    commands: Vec<CommandRunnerCommandDef>,
) -> Result<Node, String> {
    let mut runner = CommandRunner::new(id, label)
        .with_run_mode(parse_run_mode(run_mode.as_deref())?)
        .with_on_error(parse_on_error(on_error.as_deref())?)
        .with_advance_on_success(advance_on_success.unwrap_or(false));
    for command in commands {
        let reads = command
            .reads
            .as_ref()
            .map(|value| compile_read_binding_value(value, true))
            .transpose()?;
        runner = runner.command_with_reads(command.label, command.program, command.args, reads);
    }
    if let Some(timeout_ms) = timeout_ms {
        runner = runner.with_timeout_ms(timeout_ms);
    }
    if let Some(visible_lines) = visible_lines {
        runner = runner.with_visible_lines(visible_lines);
    }
    if let Some(spinner_style) = spinner_style {
        runner = runner.with_spinner_style(parse_spinner_style(spinner_style.as_str())?);
    }
    Ok(Node::Component(Box::new(runner)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_file_browser(
    id: String,
    label: String,
    browser_mode: Option<String>,
    selection_mode: Option<String>,
    entry_filter: Option<String>,
    display_mode: Option<String>,
    value_mode: Option<String>,
    cwd: Option<String>,
    recursive: Option<bool>,
    hide_hidden: Option<bool>,
    ext_filter: Vec<String>,
    max_visible: Option<usize>,
    required: Option<bool>,
    extra_validators: Vec<ValidatorDef>,
) -> Result<Node, String> {
    let mode = parse_browser_mode(browser_mode.as_deref())?;
    let selection_mode = parse_file_browser_selection_mode(selection_mode.as_deref())?;
    let entry_filter = parse_file_browser_entry_filter(entry_filter.as_deref())?;
    let mut widget = FileBrowserInput::new(id, label)
        .with_browser_mode(mode)
        .with_entry_filter(entry_filter)
        .with_selection_mode(selection_mode);
    if let Some(display_mode) = display_mode {
        widget = widget.with_display_mode(parse_display_mode(display_mode.as_str())?);
    }
    if let Some(value_mode) = value_mode {
        widget = widget.with_value_mode(parse_display_mode(value_mode.as_str())?);
    }
    if let Some(cwd) = cwd {
        widget = widget.with_cwd(cwd);
    }
    if let Some(recursive) = recursive {
        widget = widget.with_recursive(recursive);
    }
    if let Some(hide_hidden) = hide_hidden {
        widget = widget.with_hide_hidden(hide_hidden);
    }
    if !ext_filter.is_empty() {
        let refs = ext_filter.iter().map(String::as_str).collect::<Vec<_>>();
        widget = widget.with_ext_filter(refs.as_slice());
    }
    if let Some(max_visible) = max_visible {
        widget = widget.with_max_visible(max_visible);
    }
    widget = with_required_and_validators(widget, required, extra_validators);
    widget.initialize_open();
    Ok(Node::Component(Box::new(widget)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_tree_view(
    id: String,
    label: String,
    nodes: Vec<TreeNodeDef>,
    max_visible: Option<usize>,
    show_label: Option<bool>,
    indent_guides: Option<bool>,
) -> Result<Node, String> {
    let mut tree_nodes = Vec::with_capacity(nodes.len());
    for node in nodes {
        let mut item = TreeNode::new(node.item, node.depth, node.has_children);
        if node.expanded.unwrap_or(false) {
            item = item.expanded();
        }
        tree_nodes.push(item);
    }
    let mut widget = TreeView::new(id, label, tree_nodes);
    if let Some(max_visible) = max_visible {
        widget = widget.with_max_visible(max_visible);
    }
    if let Some(show_label) = show_label {
        widget = widget.with_show_label(show_label);
    }
    if let Some(indent_guides) = indent_guides {
        widget = widget.with_indent_guides(indent_guides);
    }
    Ok(Node::Component(Box::new(widget)))
}

pub(super) fn compile_object_editor(
    id: String,
    label: String,
    default: Option<serde_yaml::Value>,
    max_visible: Option<usize>,
) -> Result<Node, String> {
    let mut widget = ObjectEditor::new(id, label);
    if let Some(default) = default {
        widget = widget.with_value(yaml_value_to_value(&default)?);
    }
    if let Some(max_visible) = max_visible {
        widget = widget.with_max_visible(max_visible);
    }
    Ok(Node::Component(Box::new(widget)))
}

pub(super) fn compile_snippet(
    id: String,
    label: String,
    template: String,
    inputs: Vec<WidgetDef>,
) -> Result<Node, String> {
    let mut widget = Snippet::new(id, label, template);
    for input_def in inputs {
        let node = compile_widget(input_def)?;
        if matches!(node, Node::Output(_)) {
            return Err("snippet inputs must be interactive widgets".to_string());
        }
        widget = widget.with_input(node);
    }
    Ok(Node::Component(Box::new(widget)))
}

pub(super) fn compile_table(
    id: String,
    label: String,
    style: Option<String>,
    row_numbers: Option<bool>,
    initial_rows: Option<usize>,
    columns: Vec<TableColumnDef>,
) -> Result<Node, String> {
    let mut widget = Table::new(id, label).with_style(parse_table_style(style.as_deref())?);
    if let Some(row_numbers) = row_numbers {
        widget = widget.with_row_numbers(row_numbers);
    }
    for column in columns {
        let cell_factory = compile_table_embedded_factory(column.widget)?;
        widget = widget.column_boxed(column.header, cell_factory);
    }
    if let Some(initial_rows) = initial_rows {
        widget = widget.with_initial_rows(initial_rows);
    }
    Ok(Node::Component(Box::new(widget)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_repeater(
    id: String,
    label: String,
    iterate: serde_yaml::Value,
    entry_mode: Option<String>,
    show_label: Option<bool>,
    show_progress: Option<bool>,
    header_template: Option<String>,
    item_label_path: Option<String>,
    finish_when: Option<super::super::model::WhenDef>,
    widgets: Vec<WidgetDef>,
) -> Result<Node, String> {
    let iterate_binding = compile_repeater_iterate_binding(&iterate)?;
    let mut widget = Repeater::new(id, label)
        .with_entry_mode(parse_repeater_entry_mode(entry_mode.as_deref())?)
        .with_iterate_binding(iterate_binding);
    if let Some(show_label) = show_label {
        widget = widget.with_show_label(show_label);
    }
    if let Some(show_progress) = show_progress {
        widget = widget.with_show_progress(show_progress);
    }
    if let Some(header_template) = header_template {
        let binding =
            compile_read_binding_value(&serde_yaml::Value::String(header_template), true)?;
        widget = widget.with_header_binding(binding);
    }
    if let Some(item_label_path) = item_label_path {
        let path = crate::core::value_path::ValuePath::parse_relative(item_label_path.as_str())
            .map_err(|err| format!("invalid repeater item_label_path: {err}"))?;
        widget = widget.with_item_label_path(path);
    }
    if let Some(finish_when) = finish_when {
        widget = widget.with_finish_condition(super::super::assemble::assemble_when(&finish_when)?);
    }
    for child in widgets {
        widget = widget.with_widget(compile_widget(child)?);
    }
    Ok(Node::Component(Box::new(widget)))
}

fn compile_repeater_iterate_binding(value: &serde_yaml::Value) -> Result<ReadBinding, String> {
    let normalized = normalize_repeater_iterate_value(value)?;
    let binding = compile_read_binding_value(&normalized, true)?;
    match &binding {
        ReadBinding::Selector(_) | ReadBinding::Template(_) => Ok(binding),
        ReadBinding::Literal(crate::core::value::Value::Number(_))
        | ReadBinding::Literal(crate::core::value::Value::None)
        | ReadBinding::List(_) => Ok(binding),
        ReadBinding::Literal(crate::core::value::Value::Text(_)) => {
            Err("repeater iterate must be a number, list, or store selector".to_string())
        }
        ReadBinding::Literal(other) => Err(format!(
            "repeater iterate must be a number, list, or store selector, got {}",
            other.kind_name()
        )),
        ReadBinding::Object(_) => {
            Err("repeater iterate must be a number, list, or store selector".to_string())
        }
    }
}

fn normalize_repeater_iterate_value(
    value: &serde_yaml::Value,
) -> Result<serde_yaml::Value, String> {
    let serde_yaml::Value::String(text) = value else {
        return Ok(value.clone());
    };

    if let Some(expr) = exact_template_expr(text)
        && let Ok(selector) = normalize_store_selector(expr)
    {
        return Ok(serde_yaml::Value::String(format!("{{{{{selector}}}}}")));
    }

    if let Ok(selector) = normalize_store_selector(text) {
        return Ok(serde_yaml::Value::String(selector));
    }

    Ok(value.clone())
}
