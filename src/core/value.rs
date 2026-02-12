#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Text(String),
    Bool(bool),
    Number(i64),
    List(Vec<String>),
}

impl Value {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::None => true,
            Self::Text(v) => v.is_empty(),
            Self::List(v) => v.is_empty(),
            _ => false,
        }
    }
}
