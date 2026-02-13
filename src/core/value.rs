use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Text(String),
    Bool(bool),
    Number(f64),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::None => true,
            Self::Text(v) => v.is_empty(),
            Self::List(v) => v.is_empty(),
            Self::Object(v) => v.is_empty(),
            _ => false,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Self::List(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn into_text(self) -> Option<String> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    pub fn to_text_scalar(&self) -> Option<String> {
        match self {
            Self::Text(value) => Some(value.clone()),
            Self::Number(value) => Some(value.to_string()),
            Self::Bool(value) => Some(if *value { "true" } else { "false" }.to_string()),
            Self::None | Self::List(_) | Self::Object(_) => None,
        }
    }

    pub fn to_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(value) => parse_number_text(value.as_str()),
            Self::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
            Self::List(_) | Self::Object(_) | Self::None => None,
        }
    }

    pub fn to_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            Self::Number(value) => Some(*value != 0.0),
            Self::Text(value) => parse_bool_text(value.as_str()),
            Self::List(_) | Self::Object(_) | Self::None => None,
        }
    }

    pub fn to_text_list(&self) -> Option<Vec<String>> {
        match self {
            Self::List(values) => Some(
                values
                    .iter()
                    .filter_map(Value::to_text_scalar)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    pub fn list_last(&self) -> Option<&Value> {
        match self {
            Self::List(values) => values.last(),
            _ => None,
        }
    }
}

fn parse_number_text(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(number) = trimmed.parse::<f64>() {
        return Some(number);
    }

    trimmed
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches('%')
        .parse::<f64>()
        .ok()
}

fn parse_bool_text(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "on" => Some(true),
        "false" | "0" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}
