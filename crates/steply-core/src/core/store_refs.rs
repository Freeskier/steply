use crate::core::value::Value;
use crate::core::value_path::{PathSegment, ValuePath, ValueTarget};

pub fn parse_store_selector(selector: &str) -> Result<ValueTarget, String> {
    let trimmed = selector.trim();
    if trimmed.contains("::") {
        return ValueTarget::parse_selector(trimmed).map_err(|err| err.to_string());
    }

    if trimmed.is_empty() {
        return Err("empty selector".to_string());
    }

    let path = ValuePath::parse(trimmed).map_err(|err| err.to_string())?;
    let Some((first, rest)) = path.segments().split_first() else {
        return Err("empty selector".to_string());
    };
    let PathSegment::Key(root) = first else {
        return Err("selector must start with a root key".to_string());
    };

    if rest.is_empty() {
        return Ok(ValueTarget::node(root.clone()));
    }

    Ok(ValueTarget::path(
        root.clone(),
        ValuePath::new(rest.to_vec()),
    ))
}

pub fn normalize_store_selector(selector: &str) -> Result<String, String> {
    parse_store_selector(selector).map(|target| target.to_selector())
}

pub fn exact_template_expr(template: &str) -> Option<&str> {
    let trimmed = template.trim();
    if !(trimmed.starts_with("{{") && trimmed.ends_with("}}")) {
        return None;
    }
    let inner = &trimmed[2..trimmed.len().saturating_sub(2)];
    let inner = inner.trim();
    (!inner.is_empty() && !inner.contains("{{") && !inner.contains("}}")).then_some(inner)
}

pub fn template_expressions(template: &str) -> Vec<String> {
    let chars = template.chars().collect::<Vec<_>>();
    let mut expressions = Vec::new();
    let mut idx = 0usize;

    while idx < chars.len() {
        if chars[idx] == '{' && idx + 1 < chars.len() && chars[idx + 1] == '{' {
            let mut end = idx + 2;
            while end + 1 < chars.len() && !(chars[end] == '}' && chars[end + 1] == '}') {
                end += 1;
            }
            if end + 1 < chars.len() {
                let expr = chars[idx + 2..end]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
                if !expr.is_empty() {
                    expressions.push(expr);
                }
                idx = end + 2;
                continue;
            }
        }
        idx += 1;
    }

    expressions
}

pub fn render_template(
    template: &str,
    mut resolve: impl FnMut(&str) -> Option<Value>,
    mut format: impl FnMut(&Value) -> String,
) -> String {
    let chars = template.chars().collect::<Vec<_>>();
    let mut out = String::new();
    let mut idx = 0usize;

    while idx < chars.len() {
        if chars[idx] == '{' && idx + 1 < chars.len() && chars[idx + 1] == '{' {
            let mut end = idx + 2;
            while end + 1 < chars.len() && !(chars[end] == '}' && chars[end + 1] == '}') {
                end += 1;
            }
            if end + 1 < chars.len() {
                let expr = chars[idx + 2..end]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
                if let Some(value) = resolve(expr.as_str()) {
                    out.push_str(format(&value).as_str());
                }
                idx = end + 2;
                continue;
            }
        }

        out.push(chars[idx]);
        idx += 1;
    }

    out
}

pub fn resolve_template_value(
    template: &str,
    mut resolve: impl FnMut(&str) -> Option<Value>,
) -> Value {
    if let Some(expr) = exact_template_expr(template) {
        return resolve(expr).unwrap_or(Value::None);
    }

    Value::Text(render_template(template, resolve, |value| {
        value.to_text_scalar().unwrap_or_else(|| value.to_json())
    }))
}
