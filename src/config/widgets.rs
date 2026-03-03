use std::sync::Arc;

use crate::core::value::Value;
use crate::widgets::{
    components::{
        calendar::Calendar,
        command_runner::CommandRunner,
        file_browser::FileBrowserInput,
        object_editor::ObjectEditor,
        repeater::{Repeater, RepeaterFieldFactory},
        select_list::{SelectItem, SelectList},
        snippet::Snippet,
        table::{CellFactory, Table},
        textarea::TextAreaComponent,
        tree_view::{TreeNode, TreeView},
    },
    inputs::{
        array::ArrayInput, button::ButtonInput, checkbox::CheckboxInput, choice::ChoiceInput,
        color::ColorInput, confirm::ConfirmInput, masked::MaskedInput, select::SelectInput,
        slider::SliderInput, text::TextInput,
    },
    node::Node,
    outputs::{
        chart::ChartOutput,
        diff::DiffOutput,
        progress::ProgressOutput,
        table::TableOutput,
        task_log::{TaskLog, TaskLogStep},
        text::TextOutput,
        thinking::ThinkingOutput,
        url::UrlOutput,
    },
    traits::InteractiveNode,
    validators,
};

use super::model::{CellWidgetDef, SelectListOptionDef, WidgetDef};
use super::parse::{
    WithChangeTargetPathValue, WithSubmitTargetPathValue, compile_validators, parse_browser_mode,
    parse_calendar_mode, parse_chart_mode, parse_confirm_mode, parse_display_mode, parse_on_error,
    parse_progress_style, parse_progress_transition, parse_repeater_layout, parse_run_mode,
    parse_select_mode, parse_spinner_style, parse_table_output_style, parse_table_style,
    parse_text_mode, parse_thinking_mode, parse_value_target,
};
use super::utils::yaml_value_to_value;

pub(super) fn compile_widget(def: WidgetDef) -> Result<Node, String> {
    let node = match def {
        WidgetDef::TextOutput { id, text } => Node::Output(Box::new(TextOutput::new(id, text))),
        WidgetDef::UrlOutput { id, url, name } => {
            let mut output = UrlOutput::new(id, url);
            if let Some(name) = name {
                output = output.with_name(name);
            }
            Node::Output(Box::new(output))
        }
        WidgetDef::ThinkingOutput {
            id,
            label,
            text,
            mode,
            tail_len,
            tick_ms,
            base_rgb,
            peak_rgb,
        } => {
            let mut output = ThinkingOutput::new(id, label, text)
                .with_mode(parse_thinking_mode(mode.as_deref())?);
            if let Some(tail_len) = tail_len {
                output = output.with_tail_len(tail_len);
            }
            if let Some(tick_ms) = tick_ms {
                output = output.with_tick_ms(tick_ms);
            }
            if let (Some(base), Some(peak)) = (base_rgb, peak_rgb) {
                output = output
                    .with_gradient_rgb((base[0], base[1], base[2]), (peak[0], peak[1], peak[2]));
            }
            Node::Output(Box::new(output))
        }
        WidgetDef::ProgressOutput {
            id,
            label,
            min,
            max,
            unit,
            bar_width,
            style,
            transition,
        } => {
            let mut output =
                ProgressOutput::new(id, label).with_style(parse_progress_style(style.as_deref())?);
            if let (Some(min), Some(max)) = (min, max) {
                output = output.with_range(min, max);
            }
            if let Some(unit) = unit {
                output = output.with_unit(unit);
            }
            if let Some(bar_width) = bar_width {
                output = output.with_bar_width(bar_width);
            }
            if let Some(transition) = transition {
                output = output.with_transition(parse_progress_transition(transition)?);
            }
            Node::Output(Box::new(output))
        }
        WidgetDef::ChartOutput {
            id,
            label,
            mode,
            capacity,
            min,
            max,
            unit,
            gradient,
        } => {
            let mut output =
                ChartOutput::new(id, label).with_mode(parse_chart_mode(mode.as_deref())?);
            if let Some(capacity) = capacity {
                output = output.with_capacity(capacity);
            }
            if let (Some(min), Some(max)) = (min, max) {
                output = output.with_range(min, max);
            }
            if let Some(unit) = unit {
                output = output.with_unit(unit);
            }
            if let Some(gradient) = gradient {
                output = output.with_gradient(gradient);
            }
            Node::Output(Box::new(output))
        }
        WidgetDef::TableOutput {
            id,
            label,
            style,
            headers,
            rows,
        } => Node::Output(Box::new(
            TableOutput::new(id, label)
                .with_style(parse_table_output_style(style.as_deref())?)
                .with_headers(headers)
                .with_rows(rows),
        )),
        WidgetDef::DiffOutput {
            id,
            label,
            old,
            new,
            max_visible,
        } => {
            let mut output = DiffOutput::new(id, label, old, new);
            if let Some(max_visible) = max_visible {
                output = output.with_max_visible(max_visible);
            }
            Node::Component(Box::new(output))
        }
        WidgetDef::TaskLogOutput {
            id,
            visible_lines,
            spinner_style,
            steps,
        } => {
            let step_defs = steps
                .into_iter()
                .map(|step| TaskLogStep::new(step.label, step.task_id))
                .collect::<Vec<_>>();
            let mut output = TaskLog::new(id, step_defs);
            if let Some(visible_lines) = visible_lines {
                output = output.with_visible_lines(visible_lines);
            }
            if let Some(spinner_style) = spinner_style {
                output = output.with_spinner_style(parse_spinner_style(spinner_style.as_str())?);
            }
            Node::Output(Box::new(output))
        }
        WidgetDef::TextInput {
            id,
            label,
            placeholder,
            default,
            mode,
            required,
            validators: extra_validators,
            completion_items,
            submit_target,
            change_targets,
        } => {
            let mut input = TextInput::new(id, label)
                .with_mode(parse_text_mode(mode.as_deref())?)
                .with_completion_items(completion_items);
            if let Some(placeholder) = placeholder {
                input = input.with_placeholder(placeholder);
            }
            if let Some(default) = default {
                input = input.with_default(Value::Text(default));
            }
            if let Some(submit_target) = submit_target {
                input = input
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            for target in change_targets {
                input = input.with_change_target_path_value(parse_value_target(target.as_str())?);
            }
            if required.unwrap_or(false) {
                input = input.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                input = input.with_validator(validator);
            }
            Node::Input(Box::new(input))
        }
        WidgetDef::ArrayInput {
            id,
            label,
            items,
            required,
            validators: extra_validators,
        } => {
            let mut input = ArrayInput::new(id, label).with_items(items);
            if required.unwrap_or(false) {
                input = input.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                input = input.with_validator(validator);
            }
            Node::Input(Box::new(input))
        }
        WidgetDef::ButtonInput {
            id,
            label,
            text,
            task_id,
        } => {
            let mut input = ButtonInput::new(id, label);
            if let Some(text) = text {
                input = input.with_text(text);
            }
            if let Some(task_id) = task_id {
                input = input.with_task_id(task_id);
            }
            Node::Input(Box::new(input))
        }
        WidgetDef::Select {
            id,
            label,
            options,
            selected,
            default,
            required,
            validators: extra_validators,
            submit_target,
        } => {
            let mut input = SelectInput::new(id, label, options);
            if let Some(selected) = selected {
                input = input.with_selected(selected);
            }
            if let Some(default) = default {
                input = input.with_default(Value::Text(default));
            }
            if let Some(submit_target) = submit_target {
                input = input
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            if required.unwrap_or(false) {
                input = input.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                input = input.with_validator(validator);
            }
            Node::Input(Box::new(input))
        }
        WidgetDef::ChoiceInput {
            id,
            label,
            options,
            bullets,
            default,
            required,
            validators: extra_validators,
            submit_target,
        } => {
            let mut input =
                ChoiceInput::new(id, label, options).with_bullets(bullets.unwrap_or(true));
            if let Some(default) = default {
                input = input.with_default(Value::Text(default));
            }
            if let Some(submit_target) = submit_target {
                input = input
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            if required.unwrap_or(false) {
                input = input.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                input = input.with_validator(validator);
            }
            Node::Input(Box::new(input))
        }
        WidgetDef::SelectList {
            id,
            label,
            options,
            mode,
            max_visible,
            selected,
            show_label,
            submit_target,
        } => {
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
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            Node::Component(Box::new(widget))
        }
        WidgetDef::MaskedInput {
            id,
            label,
            mask,
            default,
            required,
            validators: extra_validators,
            submit_target,
        } => {
            let mut input = MaskedInput::new(id, label, mask);
            if let Some(default) = default {
                input = input.with_default(Value::Text(default));
            }
            if let Some(submit_target) = submit_target {
                input = input
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            if required.unwrap_or(false) {
                input = input.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                input = input.with_validator(validator);
            }
            Node::Input(Box::new(input))
        }
        WidgetDef::Slider {
            id,
            label,
            min,
            max,
            step,
            unit,
            track_len,
            default,
            required,
            validators: extra_validators,
            change_targets,
        } => {
            let mut widget = SliderInput::new(id, label, min, max);
            if let Some(step) = step {
                widget = widget.with_step(step);
            }
            if let Some(unit) = unit {
                widget = widget.with_unit(unit);
            }
            if let Some(track_len) = track_len {
                widget = widget.with_track_len(track_len);
            }
            if let Some(default) = default {
                widget = widget.with_default(Value::Number(default));
            }
            for target in change_targets {
                widget = widget.with_change_target_path_value(parse_value_target(target.as_str())?);
            }
            if required.unwrap_or(false) {
                widget = widget.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                widget = widget.with_validator(validator);
            }
            Node::Input(Box::new(widget))
        }
        WidgetDef::ColorInput {
            id,
            label,
            rgb,
            required,
            validators: extra_validators,
            submit_target,
        } => {
            let mut widget = ColorInput::new(id, label);
            if let Some([r, g, b]) = rgb {
                widget = widget.with_rgb(r, g, b);
            }
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            if required.unwrap_or(false) {
                widget = widget.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                widget = widget.with_validator(validator);
            }
            Node::Input(Box::new(widget))
        }
        WidgetDef::ConfirmInput {
            id,
            label,
            mode,
            yes_label,
            no_label,
            default,
        } => {
            let mut widget = ConfirmInput::new(id, label).with_mode(parse_confirm_mode(mode));
            if let (Some(yes), Some(no)) = (yes_label, no_label) {
                widget = widget.with_options(yes, no);
            }
            if let Some(default) = default {
                widget = widget.with_default(Value::Bool(default));
            }
            Node::Input(Box::new(widget))
        }
        WidgetDef::Checkbox {
            id,
            label,
            checked,
            required,
            validators: extra_validators,
        } => {
            let mut widget = CheckboxInput::new(id, label);
            if checked.unwrap_or(false) {
                widget = widget.with_checked(true);
            }
            if required.unwrap_or(false) {
                widget = widget.with_validator(validators::must_be_checked());
            }
            for validator in compile_validators(extra_validators) {
                widget = widget.with_validator(validator);
            }
            Node::Input(Box::new(widget))
        }
        WidgetDef::Calendar {
            id,
            label,
            mode,
            required,
            validators: extra_validators,
            submit_target,
        } => {
            let mut widget =
                Calendar::new(id, label).with_mode(parse_calendar_mode(mode.as_deref())?);
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            if required.unwrap_or(false) {
                widget = widget.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                widget = widget.with_validator(validator);
            }
            Node::Component(Box::new(widget))
        }
        WidgetDef::Textarea {
            id,
            min_height,
            max_height,
            default,
            required,
            validators: extra_validators,
        } => {
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
            if required.unwrap_or(false) {
                widget = widget.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                widget = widget.with_validator(validator);
            }
            Node::Input(Box::new(widget))
        }
        WidgetDef::CommandRunner {
            id,
            label,
            run_mode,
            on_error,
            advance_on_success,
            visible_lines,
            spinner_style,
            timeout_ms,
            commands,
        } => {
            let mut runner = CommandRunner::new(id, label)
                .with_run_mode(parse_run_mode(run_mode.as_deref())?)
                .with_on_error(parse_on_error(on_error.as_deref())?)
                .with_advance_on_success(advance_on_success.unwrap_or(false));
            for command in commands {
                runner = runner.command(command.label, command.program, command.args);
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
            Node::Component(Box::new(runner))
        }
        WidgetDef::FileBrowser {
            id,
            label,
            browser_mode,
            display_mode,
            cwd,
            recursive,
            hide_hidden,
            ext_filter,
            max_visible,
            submit_target,
            required,
            validators: extra_validators,
        } => {
            let mode = parse_browser_mode(browser_mode.as_deref())?;
            let mut widget = FileBrowserInput::new(id, label).with_browser_mode(mode);
            if let Some(display_mode) = display_mode {
                widget = widget.with_display_mode(parse_display_mode(display_mode.as_str())?);
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
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            if required.unwrap_or(false) {
                widget = widget.with_validator(validators::required_msg("Field is required"));
            }
            for validator in compile_validators(extra_validators) {
                widget = widget.with_validator(validator);
            }
            Node::Component(Box::new(widget))
        }
        WidgetDef::TreeView {
            id,
            label,
            nodes,
            max_visible,
            show_label,
            indent_guides,
            submit_target,
        } => {
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
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            Node::Component(Box::new(widget))
        }
        WidgetDef::ObjectEditor {
            id,
            label,
            value,
            max_visible,
            submit_target,
        } => {
            let mut widget = ObjectEditor::new(id, label);
            if let Some(value) = value {
                widget = widget.with_value(yaml_value_to_value(&value)?);
            }
            if let Some(max_visible) = max_visible {
                widget = widget.with_max_visible(max_visible);
            }
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            Node::Component(Box::new(widget))
        }
        WidgetDef::Snippet {
            id,
            label,
            template,
            inputs,
            submit_target,
        } => {
            let mut widget = Snippet::new(id, label, template);
            for input_def in inputs {
                let node = compile_widget(input_def)?;
                if matches!(node, Node::Output(_)) {
                    return Err("snippet inputs must be interactive widgets".to_string());
                }
                widget = widget.with_input(node);
            }
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            Node::Component(Box::new(widget))
        }
        WidgetDef::Table {
            id,
            label,
            style,
            row_numbers,
            initial_rows,
            columns,
        } => {
            let mut widget = Table::new(id, label).with_style(parse_table_style(style.as_deref())?);
            if let Some(row_numbers) = row_numbers {
                widget = widget.with_row_numbers(row_numbers);
            }
            for column in columns {
                let cell_factory = compile_table_cell_factory(column.widget)?;
                widget = widget.column_boxed(column.header, cell_factory);
            }
            if let Some(initial_rows) = initial_rows {
                widget = widget.with_initial_rows(initial_rows);
            }
            Node::Component(Box::new(widget))
        }
        WidgetDef::Repeater {
            id,
            label,
            layout,
            show_label,
            show_progress,
            header_template,
            item_label_path,
            items,
            submit_target,
            fields,
        } => {
            let mut widget =
                Repeater::new(id, label).with_layout(parse_repeater_layout(layout.as_deref())?);
            if let Some(show_label) = show_label {
                widget = widget.with_show_label(show_label);
            }
            if let Some(show_progress) = show_progress {
                widget = widget.with_show_progress(show_progress);
            }
            if let Some(header_template) = header_template {
                widget = widget.with_header_template(header_template);
            }
            if let Some(item_label_path) = item_label_path {
                let path =
                    crate::core::value_path::ValuePath::parse_relative(item_label_path.as_str())
                        .map_err(|err| format!("invalid repeater item_label_path: {err}"))?;
                widget = widget.with_item_label_path(path);
            }
            if !items.is_empty() {
                let parsed_items = items
                    .into_iter()
                    .map(|item| yaml_value_to_value(&item))
                    .collect::<Result<Vec<_>, _>>()?;
                widget = widget.with_items(parsed_items);
            }
            for field in fields {
                let make_input = compile_repeater_field_factory(field.widget)?;
                widget = widget.field_boxed(field.key, field.label, make_input);
            }
            if let Some(submit_target) = submit_target {
                widget = widget
                    .with_submit_target_path_value(parse_value_target(submit_target.as_str())?);
            }
            Node::Component(Box::new(widget))
        }
    };
    Ok(node)
}

fn compile_table_cell_factory(def: CellWidgetDef) -> Result<CellFactory, String> {
    let template = def.clone();
    validate_cell_widget_def(&template)?;
    Ok(Arc::new(move |id, label| {
        // prevalidated in compile_table_cell_factory
        compile_cell_widget(template.clone(), id, label).expect("cell widget template validated")
    }))
}

fn compile_repeater_field_factory(def: CellWidgetDef) -> Result<RepeaterFieldFactory, String> {
    let template = def.clone();
    validate_cell_widget_def(&template)?;
    Ok(Arc::new(move |id, label| {
        // prevalidated in compile_repeater_field_factory
        compile_cell_widget(template.clone(), id, label)
            .expect("repeater field widget template validated")
    }))
}

fn validate_cell_widget_def(def: &CellWidgetDef) -> Result<(), String> {
    if let CellWidgetDef::TextInput {
        mode: Some(mode), ..
    } = def
    {
        let _ = parse_text_mode(Some(mode.as_str()))?;
    }
    Ok(())
}

fn compile_cell_widget(
    def: CellWidgetDef,
    id: String,
    label: String,
) -> Result<Box<dyn InteractiveNode>, String> {
    match def {
        CellWidgetDef::TextInput { placeholder, mode } => {
            let mut input = TextInput::new(id, label).with_mode(parse_text_mode(mode.as_deref())?);
            if let Some(placeholder) = placeholder {
                input = input.with_placeholder(placeholder);
            }
            Ok(Box::new(input))
        }
        CellWidgetDef::MaskedInput { mask } => Ok(Box::new(MaskedInput::new(id, label, mask))),
        CellWidgetDef::Select { options } => Ok(Box::new(SelectInput::new(id, label, options))),
        CellWidgetDef::Slider {
            min,
            max,
            step,
            unit,
        } => {
            let mut input = SliderInput::new(id, label, min, max);
            if let Some(step) = step {
                input = input.with_step(step);
            }
            if let Some(unit) = unit {
                input = input.with_unit(unit);
            }
            Ok(Box::new(input))
        }
        CellWidgetDef::Checkbox { checked } => {
            let mut input = CheckboxInput::new(id, label);
            if checked.unwrap_or(false) {
                input = input.with_checked(true);
            }
            Ok(Box::new(input))
        }
        CellWidgetDef::ArrayInput { items } => {
            Ok(Box::new(ArrayInput::new(id, label).with_items(items)))
        }
    }
}
