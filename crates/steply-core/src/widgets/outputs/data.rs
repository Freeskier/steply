use crate::core::value::Value;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::traits::{DrawOutput, Drawable, OutputNode, RenderContext};
use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataOutputFormat {
    Text,
    Json,
    Yaml,
}

pub struct DataOutput {
    id: String,
    label: Option<String>,
    format: DataOutputFormat,
    value: Value,
}

impl DataOutput {
    pub fn new(id: impl Into<String>, label: Option<String>, format: DataOutputFormat) -> Self {
        Self {
            id: id.into(),
            label,
            format,
            value: Value::None,
        }
    }

    fn render_lines(&self) -> Vec<SpanLine> {
        match self.format {
            DataOutputFormat::Text => render_text_lines(&self.value),
            DataOutputFormat::Json => render_json_lines(&self.value),
            DataOutputFormat::Yaml => render_yaml_lines(&self.value),
        }
    }
}

impl Drawable for DataOutput {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        self.label.as_deref().unwrap_or("")
    }

    fn draw(&self, _ctx: &RenderContext) -> DrawOutput {
        let mut lines = Vec::new();
        if let Some(label) = &self.label
            && !label.is_empty()
        {
            lines.push(vec![Span::new(label.clone()).no_wrap()]);
        }
        lines.extend(self.render_lines());
        DrawOutput::with_lines(lines)
    }
}

impl OutputNode for DataOutput {
    fn value(&self) -> Option<Value> {
        Some(self.value.clone())
    }

    fn set_value(&mut self, value: Value) {
        self.value = value;
    }
}

fn render_text_lines(value: &Value) -> Vec<SpanLine> {
    value
        .to_text_scalar()
        .unwrap_or_else(|| value.to_json_pretty())
        .lines()
        .map(|line| vec![Span::new(line.to_string())])
        .collect()
}

fn render_json_lines(value: &Value) -> Vec<SpanLine> {
    let mut out = Vec::new();
    render_json_value(value, 0, &mut out, false);
    if out.is_empty() {
        out.push(vec![styled_null("null")]);
    }
    out
}

fn render_json_value(value: &Value, indent: usize, out: &mut Vec<SpanLine>, trailing_comma: bool) {
    match value {
        Value::Object(map) => render_json_object(map, indent, out, trailing_comma),
        Value::List(items) => render_json_list(items, indent, out, trailing_comma),
        scalar => {
            let mut line = indent_spans(indent);
            line.push(render_scalar_json(scalar));
            if trailing_comma {
                line.push(punctuation(","));
            }
            out.push(line);
        }
    }
}

fn render_json_object(
    map: &IndexMap<String, Value>,
    indent: usize,
    out: &mut Vec<SpanLine>,
    trailing_comma: bool,
) {
    if map.is_empty() {
        let mut line = indent_spans(indent);
        line.push(punctuation("{}"));
        if trailing_comma {
            line.push(punctuation(","));
        }
        out.push(line);
        return;
    }

    let mut open = indent_spans(indent);
    open.push(punctuation("{"));
    out.push(open);

    for (index, (key, value)) in map.iter().enumerate() {
        let is_last = index + 1 == map.len();
        match value {
            Value::Object(inner) if !inner.is_empty() => {
                let mut line = indent_spans(indent + 1);
                line.push(styled_key_json(key.as_str()));
                line.push(punctuation(": "));
                line.push(punctuation("{"));
                out.push(line);
                render_json_object_body(inner, indent + 2, out);
                let mut close = indent_spans(indent + 1);
                close.push(punctuation("}"));
                if !is_last {
                    close.push(punctuation(","));
                }
                out.push(close);
            }
            Value::List(items) if !items.is_empty() => {
                let mut line = indent_spans(indent + 1);
                line.push(styled_key_json(key.as_str()));
                line.push(punctuation(": "));
                line.push(punctuation("["));
                out.push(line);
                render_json_list_body(items, indent + 2, out);
                let mut close = indent_spans(indent + 1);
                close.push(punctuation("]"));
                if !is_last {
                    close.push(punctuation(","));
                }
                out.push(close);
            }
            _ => {
                let mut line = indent_spans(indent + 1);
                line.push(styled_key_json(key.as_str()));
                line.push(punctuation(": "));
                line.push(render_scalar_json(value));
                if !is_last {
                    line.push(punctuation(","));
                }
                out.push(line);
            }
        }
    }

    let mut close = indent_spans(indent);
    close.push(punctuation("}"));
    if trailing_comma {
        close.push(punctuation(","));
    }
    out.push(close);
}

fn render_json_object_body(map: &IndexMap<String, Value>, indent: usize, out: &mut Vec<SpanLine>) {
    for (index, (key, value)) in map.iter().enumerate() {
        let is_last = index + 1 == map.len();
        match value {
            Value::Object(inner) if !inner.is_empty() => {
                let mut line = indent_spans(indent);
                line.push(styled_key_json(key.as_str()));
                line.push(punctuation(": "));
                line.push(punctuation("{"));
                out.push(line);
                render_json_object_body(inner, indent + 1, out);
                let mut close = indent_spans(indent);
                close.push(punctuation("}"));
                if !is_last {
                    close.push(punctuation(","));
                }
                out.push(close);
            }
            Value::List(items) if !items.is_empty() => {
                let mut line = indent_spans(indent);
                line.push(styled_key_json(key.as_str()));
                line.push(punctuation(": "));
                line.push(punctuation("["));
                out.push(line);
                render_json_list_body(items, indent + 1, out);
                let mut close = indent_spans(indent);
                close.push(punctuation("]"));
                if !is_last {
                    close.push(punctuation(","));
                }
                out.push(close);
            }
            _ => {
                let mut line = indent_spans(indent);
                line.push(styled_key_json(key.as_str()));
                line.push(punctuation(": "));
                line.push(render_scalar_json(value));
                if !is_last {
                    line.push(punctuation(","));
                }
                out.push(line);
            }
        }
    }
}

fn render_json_list(items: &[Value], indent: usize, out: &mut Vec<SpanLine>, trailing_comma: bool) {
    if items.is_empty() {
        let mut line = indent_spans(indent);
        line.push(punctuation("[]"));
        if trailing_comma {
            line.push(punctuation(","));
        }
        out.push(line);
        return;
    }

    let mut open = indent_spans(indent);
    open.push(punctuation("["));
    out.push(open);
    render_json_list_body(items, indent + 1, out);
    let mut close = indent_spans(indent);
    close.push(punctuation("]"));
    if trailing_comma {
        close.push(punctuation(","));
    }
    out.push(close);
}

fn render_json_list_body(items: &[Value], indent: usize, out: &mut Vec<SpanLine>) {
    for (index, item) in items.iter().enumerate() {
        render_json_value(item, indent, out, index + 1 != items.len());
    }
}

fn render_yaml_lines(value: &Value) -> Vec<SpanLine> {
    let mut out = Vec::new();
    render_yaml_value(None, value, 0, &mut out);
    if out.is_empty() {
        out.push(vec![styled_null("null")]);
    }
    out
}

fn render_yaml_value(key: Option<&str>, value: &Value, indent: usize, out: &mut Vec<SpanLine>) {
    match value {
        Value::Object(map) => render_yaml_object(key, map, indent, out),
        Value::List(items) => render_yaml_list(key, items, indent, out),
        scalar => {
            let mut line = indent_spans(indent);
            if let Some(key) = key {
                line.push(styled_key_yaml(key));
                line.push(punctuation(": "));
            }
            line.push(render_scalar_yaml(scalar));
            out.push(line);
        }
    }
}

fn render_yaml_object(
    key: Option<&str>,
    map: &IndexMap<String, Value>,
    indent: usize,
    out: &mut Vec<SpanLine>,
) {
    if let Some(key) = key {
        let mut line = indent_spans(indent);
        line.push(styled_key_yaml(key));
        if map.is_empty() {
            line.push(punctuation(": {}"));
            out.push(line);
            return;
        }
        line.push(punctuation(":"));
        out.push(line);
    } else if map.is_empty() {
        out.push(vec![punctuation("{}")]);
        return;
    }

    let next_indent = indent + usize::from(key.is_some());
    for (child_key, child_value) in map {
        render_yaml_value(Some(child_key.as_str()), child_value, next_indent, out);
    }
}

fn render_yaml_list(key: Option<&str>, items: &[Value], indent: usize, out: &mut Vec<SpanLine>) {
    if let Some(key) = key {
        if items.is_empty() {
            let mut line = indent_spans(indent);
            line.push(styled_key_yaml(key));
            line.push(punctuation(": []"));
            out.push(line);
            return;
        }
        let mut line = indent_spans(indent);
        line.push(styled_key_yaml(key));
        line.push(punctuation(":"));
        out.push(line);
        for item in items {
            render_yaml_list_item(item, indent + 1, out);
        }
        return;
    }

    if items.is_empty() {
        out.push(vec![punctuation("[]")]);
        return;
    }
    for item in items {
        render_yaml_list_item(item, indent, out);
    }
}

fn render_yaml_list_item(value: &Value, indent: usize, out: &mut Vec<SpanLine>) {
    match value {
        Value::Object(map) if map.is_empty() => {
            let mut line = indent_spans(indent);
            line.push(punctuation("- "));
            line.push(punctuation("{}"));
            out.push(line);
        }
        Value::Object(map) => {
            let mut entries = map.iter();
            let Some((first_key, first_value)) = entries.next() else {
                return;
            };
            match first_value {
                Value::Object(_) | Value::List(_) => {
                    let mut line = indent_spans(indent);
                    line.push(punctuation("- "));
                    line.push(styled_key_yaml(first_key.as_str()));
                    line.push(punctuation(":"));
                    out.push(line);
                    render_yaml_value(None, first_value, indent + 2, out);
                }
                _ => {
                    let mut line = indent_spans(indent);
                    line.push(punctuation("- "));
                    line.push(styled_key_yaml(first_key.as_str()));
                    line.push(punctuation(": "));
                    line.push(render_scalar_yaml(first_value));
                    out.push(line);
                }
            }
            for (next_key, next_value) in entries {
                render_yaml_value(Some(next_key.as_str()), next_value, indent + 1, out);
            }
        }
        Value::List(items) if items.is_empty() => {
            let mut line = indent_spans(indent);
            line.push(punctuation("- "));
            line.push(punctuation("[]"));
            out.push(line);
        }
        Value::List(items) => {
            let mut line = indent_spans(indent);
            line.push(punctuation("-"));
            out.push(line);
            for item in items {
                render_yaml_list_item(item, indent + 1, out);
            }
        }
        scalar => {
            let mut line = indent_spans(indent);
            line.push(punctuation("- "));
            line.push(render_scalar_yaml(scalar));
            out.push(line);
        }
    }
}

fn indent_spans(indent: usize) -> SpanLine {
    if indent == 0 {
        Vec::new()
    } else {
        vec![Span::new("  ".repeat(indent)).no_wrap()]
    }
}

fn styled_key_json(key: &str) -> Span {
    Span::styled(
        format!("\"{}\"", escape_json_string(key)),
        Style::new().color(Color::Yellow).bold(),
    )
}

fn styled_key_yaml(key: &str) -> Span {
    Span::styled(key.to_string(), Style::new().color(Color::Yellow).bold())
}

fn render_scalar_json(value: &Value) -> Span {
    match value {
        Value::Text(text) => Span::styled(
            format!("\"{}\"", escape_json_string(text)),
            Style::new().color(Color::Green),
        ),
        Value::Number(number) => {
            Span::styled(number.to_string(), Style::new().color(Color::Cyan).bold())
        }
        Value::Bool(flag) => Span::styled(
            if *flag { "true" } else { "false" },
            Style::new().color(Color::Magenta).bold(),
        ),
        Value::None => styled_null("null"),
        Value::List(_) | Value::Object(_) => Span::new(value.to_json()),
    }
}

fn render_scalar_yaml(value: &Value) -> Span {
    match value {
        Value::Text(text) => {
            Span::styled(quote_yaml_string(text), Style::new().color(Color::Green))
        }
        Value::Number(number) => {
            Span::styled(number.to_string(), Style::new().color(Color::Cyan).bold())
        }
        Value::Bool(flag) => Span::styled(
            if *flag { "true" } else { "false" },
            Style::new().color(Color::Magenta).bold(),
        ),
        Value::None => styled_null("null"),
        Value::List(_) | Value::Object(_) => Span::new(value.to_json()),
    }
}

fn punctuation(text: &str) -> Span {
    Span::styled(text.to_string(), Style::new().color(Color::DarkGrey))
}

fn styled_null(text: &str) -> Span {
    Span::styled(text.to_string(), Style::new().color(Color::DarkGrey).bold())
}

fn escape_json_string(text: &str) -> String {
    serde_json::to_string(text)
        .unwrap_or_else(|_| "\"\"".to_string())
        .trim_matches('"')
        .to_string()
}

fn quote_yaml_string(text: &str) -> String {
    serde_yaml::to_string(text)
        .ok()
        .map(|yaml| yaml.trim().to_string())
        .unwrap_or_else(|| format!("{text:?}"))
}
