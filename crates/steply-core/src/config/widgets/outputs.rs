use crate::widgets::{
    node::Node,
    outputs::{
        chart::ChartOutput,
        data::{DataOutput, DataOutputFormat},
        diff::DiffOutput,
        progress::ProgressOutput,
        table::TableOutput,
        task_log::{TaskLog, TaskLogStep},
        text::TextOutput,
        thinking::ThinkingOutput,
        url::UrlOutput,
    },
};

use crate::config::model::{DataOutputFormatDef, ProgressTransitionDef, TaskLogStepDef};

use super::super::parse::{
    parse_chart_mode, parse_progress_style, parse_progress_transition, parse_spinner_style,
    parse_table_output_style, parse_thinking_mode,
};

pub(super) fn compile_text_output(id: String, text: String) -> Node {
    Node::Output(Box::new(TextOutput::new(id, text)))
}

pub(super) fn compile_data_output(
    id: String,
    label: Option<String>,
    format: Option<DataOutputFormatDef>,
) -> Node {
    let format = match format.unwrap_or(DataOutputFormatDef::Json) {
        DataOutputFormatDef::Text => DataOutputFormat::Text,
        DataOutputFormatDef::Json => DataOutputFormat::Json,
        DataOutputFormatDef::Yaml => DataOutputFormat::Yaml,
    };
    Node::Output(Box::new(DataOutput::new(id, label, format)))
}

pub(super) fn compile_url_output(
    id: String,
    url: String,
    name: Option<String>,
) -> Result<Node, String> {
    let mut output = UrlOutput::new(id, url);
    if let Some(name) = name {
        output = output.with_name(name);
    }
    Ok(Node::Output(Box::new(output)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_thinking_output(
    id: String,
    label: String,
    text: String,
    mode: Option<String>,
    tail_len: Option<usize>,
    tick_ms: Option<u64>,
    base_rgb: Option<[u8; 3]>,
    peak_rgb: Option<[u8; 3]>,
) -> Result<Node, String> {
    let mut output =
        ThinkingOutput::new(id, label, text).with_mode(parse_thinking_mode(mode.as_deref())?);
    if let Some(tail_len) = tail_len {
        output = output.with_tail_len(tail_len);
    }
    if let Some(tick_ms) = tick_ms {
        output = output.with_tick_ms(tick_ms);
    }
    if let (Some(base), Some(peak)) = (base_rgb, peak_rgb) {
        output = output.with_gradient_rgb((base[0], base[1], base[2]), (peak[0], peak[1], peak[2]));
    }
    Ok(Node::Output(Box::new(output)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_progress_output(
    id: String,
    label: String,
    min: Option<f64>,
    max: Option<f64>,
    unit: Option<String>,
    bar_width: Option<usize>,
    style: Option<String>,
    transition: Option<ProgressTransitionDef>,
) -> Result<Node, String> {
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
    Ok(Node::Output(Box::new(output)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_chart_output(
    id: String,
    label: String,
    mode: Option<String>,
    capacity: Option<usize>,
    min: Option<f64>,
    max: Option<f64>,
    unit: Option<String>,
    gradient: Option<bool>,
) -> Result<Node, String> {
    let mut output = ChartOutput::new(id, label).with_mode(parse_chart_mode(mode.as_deref())?);
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
    Ok(Node::Output(Box::new(output)))
}

pub(super) fn compile_table_output(
    id: String,
    label: String,
    style: Option<String>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
) -> Result<Node, String> {
    Ok(Node::Output(Box::new(
        TableOutput::new(id, label)
            .with_style(parse_table_output_style(style.as_deref())?)
            .with_headers(headers)
            .with_rows(rows),
    )))
}

pub(super) fn compile_diff_output(
    id: String,
    label: String,
    old: String,
    new: String,
    max_visible: Option<usize>,
) -> Result<Node, String> {
    let mut output = DiffOutput::new(id, label, old, new);
    if let Some(max_visible) = max_visible {
        output = output.with_max_visible(max_visible);
    }
    Ok(Node::Component(Box::new(output)))
}

pub(super) fn compile_task_log_output(
    id: String,
    visible_lines: Option<usize>,
    spinner_style: Option<String>,
    steps: Vec<TaskLogStepDef>,
) -> Result<Node, String> {
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
    Ok(Node::Output(Box::new(output)))
}
