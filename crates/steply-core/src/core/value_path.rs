use crate::core::NodeId;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ValuePath {
    segments: Vec<PathSegment>,
}

impl ValuePath {
    pub fn new(segments: Vec<PathSegment>) -> Self {
        Self { segments }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn segments(&self) -> &[PathSegment] {
        self.segments.as_slice()
    }

    pub fn parse(input: &str) -> Result<Self, ValuePathParseError> {
        parse_path(input, false)
    }

    pub fn parse_relative(input: &str) -> Result<Self, ValuePathParseError> {
        parse_path(input, true)
    }
}

impl fmt::Display for ValuePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.segments.is_empty() {
            return Ok(());
        }

        for (idx, segment) in self.segments.iter().enumerate() {
            match segment {
                PathSegment::Key(key) => {
                    if idx == 0 && is_identifier(key) {
                        f.write_str(key)?;
                    } else if is_identifier(key) {
                        f.write_str(".")?;
                        f.write_str(key)?;
                    } else {
                        f.write_str("[\"")?;
                        f.write_str(key.replace('\\', "\\\\").replace('"', "\\\"").as_str())?;
                        f.write_str("\"]")?;
                    }
                }
                PathSegment::Index(index) => {
                    write!(f, "[{index}]")?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuePathParseError {
    message: String,
}

impl ValuePathParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ValuePathParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl std::error::Error for ValuePathParseError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueTarget {
    Node(NodeId),
    Path { root: NodeId, path: ValuePath },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTargetRelation {
    Equal,
    Contains,
    ContainedBy,
    Disjoint,
}

impl ValueTarget {
    pub fn node(target: impl Into<NodeId>) -> Self {
        Self::Node(target.into())
    }

    pub fn path(root: impl Into<NodeId>, path: ValuePath) -> Self {
        if path.is_empty() {
            return Self::Node(root.into());
        }
        Self::Path {
            root: root.into(),
            path,
        }
    }

    pub fn parse_selector(selector: &str) -> Result<Self, ValuePathParseError> {
        let trimmed = selector.trim();
        if trimmed.is_empty() {
            return Err(ValuePathParseError::new("empty selector"));
        }
        let Some((root, raw_path)) = trimmed.split_once("::") else {
            return Ok(Self::Node(NodeId::from(trimmed)));
        };
        let root = root.trim();
        if root.is_empty() {
            return Err(ValuePathParseError::new("selector root is empty"));
        }
        let path = ValuePath::parse_relative(raw_path.trim())?;
        Ok(Self::path(NodeId::from(root), path))
    }

    pub fn root(&self) -> &NodeId {
        match self {
            Self::Node(root) => root,
            Self::Path { root, .. } => root,
        }
    }

    pub fn nested_path(&self) -> Option<&ValuePath> {
        match self {
            Self::Node(_) => None,
            Self::Path { path, .. } => Some(path),
        }
    }

    pub fn to_selector(&self) -> String {
        match self {
            Self::Node(root) => root.to_string(),
            Self::Path { root, path } => format!("{}::{}", root, path),
        }
    }

    pub fn relation_to(&self, other: &Self) -> ValueTargetRelation {
        if self.root() != other.root() {
            return ValueTargetRelation::Disjoint;
        }

        let self_path = self
            .nested_path()
            .map(|path| path.segments())
            .unwrap_or(&[]);
        let other_path = other
            .nested_path()
            .map(|path| path.segments())
            .unwrap_or(&[]);

        if self_path == other_path {
            ValueTargetRelation::Equal
        } else if path_is_prefix(self_path, other_path) {
            ValueTargetRelation::Contains
        } else if path_is_prefix(other_path, self_path) {
            ValueTargetRelation::ContainedBy
        } else {
            ValueTargetRelation::Disjoint
        }
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        !matches!(self.relation_to(other), ValueTargetRelation::Disjoint)
    }

    pub fn contains_target(&self, other: &Self) -> bool {
        matches!(
            self.relation_to(other),
            ValueTargetRelation::Equal | ValueTargetRelation::Contains
        )
    }
}

impl From<NodeId> for ValueTarget {
    fn from(value: NodeId) -> Self {
        Self::Node(value)
    }
}

impl From<&str> for ValueTarget {
    fn from(value: &str) -> Self {
        Self::Node(NodeId::from(value))
    }
}

impl From<String> for ValueTarget {
    fn from(value: String) -> Self {
        Self::Node(NodeId::from(value))
    }
}

fn parse_path(
    input: &str,
    allow_leading_separator: bool,
) -> Result<ValuePath, ValuePathParseError> {
    let raw = input.trim();
    if raw.is_empty() {
        return Ok(ValuePath::empty());
    }

    let chars: Vec<char> = raw.chars().collect();
    let mut idx = 0usize;
    let mut out = Vec::<PathSegment>::new();

    while idx < chars.len() {
        let ch = chars[idx];
        if ch == '.' {
            if !allow_leading_separator && out.is_empty() {
                return Err(ValuePathParseError::new("path cannot start with '.'"));
            }
            idx += 1;
            let key = parse_key(&chars, &mut idx)?;
            out.push(PathSegment::Key(key));
            continue;
        }

        if ch == '[' {
            let segment = parse_bracket_segment(&chars, &mut idx)?;
            out.push(segment);
            continue;
        }

        if out.is_empty() {
            let key = parse_key(&chars, &mut idx)?;
            out.push(PathSegment::Key(key));
            continue;
        }

        return Err(ValuePathParseError::new(format!(
            "unexpected character '{}' at position {}",
            ch, idx
        )));
    }

    Ok(ValuePath::new(out))
}

fn parse_key(chars: &[char], idx: &mut usize) -> Result<String, ValuePathParseError> {
    let start = *idx;
    while *idx < chars.len() {
        let ch = chars[*idx];
        if ch == '.' || ch == '[' || ch == ']' {
            break;
        }
        *idx += 1;
    }
    if *idx == start {
        return Err(ValuePathParseError::new(format!(
            "expected key at position {}",
            start
        )));
    }
    Ok(chars[start..*idx].iter().collect::<String>())
}

fn parse_bracket_segment(
    chars: &[char],
    idx: &mut usize,
) -> Result<PathSegment, ValuePathParseError> {
    if chars.get(*idx).copied() != Some('[') {
        return Err(ValuePathParseError::new("expected '['"));
    }
    *idx += 1;
    if *idx >= chars.len() {
        return Err(ValuePathParseError::new("unterminated '[' segment"));
    }

    let ch = chars[*idx];
    if ch == '"' || ch == '\'' {
        let quote = ch;
        *idx += 1;
        let mut key = String::new();
        while *idx < chars.len() {
            let c = chars[*idx];
            *idx += 1;
            if c == '\\' {
                let Some(next) = chars.get(*idx).copied() else {
                    return Err(ValuePathParseError::new(
                        "unterminated escape in quoted key",
                    ));
                };
                key.push(next);
                *idx += 1;
                continue;
            }
            if c == quote {
                break;
            }
            key.push(c);
        }
        if *idx >= chars.len() || chars[*idx - 1] != quote {
            return Err(ValuePathParseError::new("unterminated quoted key"));
        }
        if chars.get(*idx).copied() != Some(']') {
            return Err(ValuePathParseError::new("expected closing ']'"));
        }
        *idx += 1;
        return Ok(PathSegment::Key(key));
    }

    let start = *idx;
    while *idx < chars.len() && chars[*idx] != ']' {
        *idx += 1;
    }
    if *idx >= chars.len() {
        return Err(ValuePathParseError::new("unterminated '[' segment"));
    }
    let raw = chars[start..*idx].iter().collect::<String>();
    *idx += 1;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ValuePathParseError::new("empty bracket segment"));
    }
    if let Ok(index) = trimmed.parse::<usize>() {
        return Ok(PathSegment::Index(index));
    }
    Ok(PathSegment::Key(trimmed.to_string()))
}

fn is_identifier(input: &str) -> bool {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn path_is_prefix(prefix: &[PathSegment], full: &[PathSegment]) -> bool {
    prefix.len() <= full.len() && prefix.iter().zip(full.iter()).all(|(a, b)| a == b)
}

#[cfg(test)]
#[path = "tests/value_path.rs"]
mod tests;
