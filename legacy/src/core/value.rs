#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Text(String),
    Bool(bool),
    Number(i64),
    List(Vec<String>),
    Map(Vec<(String, String)>),
}

impl Value {
    pub fn is_empty(&self) -> bool {
        match self {
            Value::None => true,
            Value::Text(text) => text.is_empty(),
            Value::List(items) => items.is_empty(),
            Value::Map(items) => items.is_empty(),
            _ => false,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(String, String)]> {
        match self {
            Value::Map(items) => Some(items),
            _ => None,
        }
    }
}
