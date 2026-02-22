use crate::core::value_path::{PathSegment, ValuePath, ensure_value_path_mut};
use indexmap::IndexMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Text(String),
    Bool(bool),
    Number(f64),
    List(Vec<Value>),
    Object(IndexMap<String, Value>),
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

    pub fn get_path(&self, path: &ValuePath) -> Option<&Value> {
        let mut current = self;
        for segment in path.segments() {
            match segment {
                PathSegment::Key(key) => {
                    let Value::Object(map) = current else {
                        return None;
                    };
                    current = map.get(key.as_str())?;
                }
                PathSegment::Index(index) => {
                    let Value::List(list) = current else {
                        return None;
                    };
                    current = list.get(*index)?;
                }
            }
        }
        Some(current)
    }

    pub fn get_path_mut(&mut self, path: &ValuePath) -> Option<&mut Value> {
        if path.is_empty() {
            return Some(self);
        }

        let segments = path.segments();
        let mut current = self;
        for segment in segments {
            match segment {
                PathSegment::Key(key) => {
                    let Value::Object(map) = current else {
                        return None;
                    };
                    current = map.get_mut(key.as_str())?;
                }
                PathSegment::Index(index) => {
                    let Value::List(list) = current else {
                        return None;
                    };
                    current = list.get_mut(*index)?;
                }
            }
        }
        Some(current)
    }

    pub fn set_path(&mut self, path: &ValuePath, value: Value) {
        if path.is_empty() {
            *self = value;
            return;
        }
        let target = ensure_value_path_mut(self, path);
        *target = value;
    }




    pub fn from_json(s: &str) -> Result<Self, String> {
        let jv: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        Ok(Self::from_serde(jv))
    }


    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.to_serde()).unwrap_or_else(|_| "null".to_string())
    }


    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.to_serde()).unwrap_or_else(|_| "null".to_string())
    }

    fn from_serde(jv: serde_json::Value) -> Self {
        match jv {
            serde_json::Value::Null => Self::None,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => Self::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Self::Text(s),
            serde_json::Value::Array(arr) => {
                Self::List(arr.into_iter().map(Self::from_serde).collect())
            }
            serde_json::Value::Object(map) => Self::Object(
                map.into_iter()
                    .map(|(k, v)| (k, Self::from_serde(v)))
                    .collect(),
            ),
        }
    }

    fn to_serde(&self) -> serde_json::Value {
        match self {
            Self::None => serde_json::Value::Null,
            Self::Bool(b) => serde_json::Value::Bool(*b),
            Self::Number(n) => serde_json::json!(n),
            Self::Text(s) => serde_json::Value::String(s.clone()),
            Self::List(arr) => serde_json::Value::Array(arr.iter().map(|v| v.to_serde()).collect()),
            Self::Object(map) => serde_json::Value::Object(
                map.iter().map(|(k, v)| (k.clone(), v.to_serde())).collect(),
            ),
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
