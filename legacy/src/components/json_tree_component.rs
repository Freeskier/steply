use crate::components::tree_view_component::{
    TreeNode, TreeNodeKind, TreeScalar, TreeViewComponent,
};
use crate::core::binding::BindTarget;
use crate::core::component::{Component, ComponentBase, ComponentResponse, FocusMode};
use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine, RenderOutput};
use crate::ui::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
enum JsonValue {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Number(String),
    Bool(bool),
    Null,
}

pub struct JsonTreeComponent {
    base: ComponentBase,
    tree: TreeViewComponent,
    bind_target: Option<BindTarget>,
    last_error: Option<String>,
}

impl JsonTreeComponent {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            tree: TreeViewComponent::new(format!("{}_tree", id)),
            base: ComponentBase::new(id),
            bind_target: None,
            last_error: None,
        }
    }

    pub fn with_bind_target(mut self, target: BindTarget) -> Self {
        self.bind_target = Some(target);
        self
    }

    pub fn bind_to_input(mut self, id: impl Into<String>) -> Self {
        self.bind_target = Some(BindTarget::Input(id.into()));
        self
    }

    pub fn set_json(&mut self, input: &str) -> Result<(), String> {
        let parsed = JsonParser::parse(input)?;
        self.tree.set_nodes(vec![json_to_tree_node(None, parsed)]);
        self.last_error = None;
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, String> {
        let value = tree_nodes_to_json(self.tree.nodes())?;
        let mut out = String::new();
        write_json_pretty(&value, 0, &mut out);
        Ok(out)
    }
}

impl Component for JsonTreeComponent {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn bind_target(&self) -> Option<BindTarget> {
        self.bind_target.clone()
    }

    fn render(&self, ctx: &RenderContext) -> RenderOutput {
        let mut prefixed_lines = Vec::new();
        let mut cursor_offset = 0usize;

        if let Some(err) = &self.last_error {
            prefixed_lines.push(RenderLine {
                spans: vec![
                    Span::new("JSON error: ").with_style(ctx.theme().error.clone()),
                    Span::new(err).with_style(ctx.theme().error.clone()),
                ],
            });
            cursor_offset = 1;
        }

        let mut tree_output = self.tree.render(ctx);
        let tree_cursor = tree_output
            .cursor
            .take()
            .map(|cursor| (cursor.line + cursor_offset, cursor.offset));

        prefixed_lines.extend(tree_output.lines);
        prefixed_lines.push(ctx.render_hint_line(
            "Ctrl+S export JSON | Tab key/value | Ctrl+A child | Ctrl+I sibling | Ctrl+D delete",
        ));

        let mut output = RenderOutput::from_lines(prefixed_lines);
        if let Some((line, col)) = tree_cursor {
            output = output.with_cursor(line, col);
        }
        output
    }

    fn value(&self) -> Option<Value> {
        self.to_json().ok().map(Value::Text)
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::Text(text) => {
                if text.trim().is_empty() {
                    self.tree.set_nodes(Vec::new());
                    self.last_error = None;
                    return;
                }
                if let Err(err) = self.set_json(&text) {
                    self.last_error = Some(err);
                }
            }
            other => {
                self.tree.set_value(other);
                self.last_error = None;
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('s') {
            return match self.to_json() {
                Ok(json) => {
                    self.last_error = None;
                    ComponentResponse::produced(Value::Text(json))
                }
                Err(err) => {
                    self.last_error = Some(err);
                    ComponentResponse::handled()
                }
            };
        }

        let response = self.tree.handle_key(code, modifiers);
        if response.handled {
            self.last_error = None;
        }
        response
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        self.tree.set_focused(focused);
    }
}

fn json_to_tree_node(key: Option<String>, value: JsonValue) -> TreeNode {
    match value {
        JsonValue::Object(entries) => {
            let mut node = TreeNode::object(key);
            node.children = entries
                .into_iter()
                .map(|(entry_key, entry_value)| json_to_tree_node(Some(entry_key), entry_value))
                .collect();
            node
        }
        JsonValue::Array(items) => {
            let mut node = TreeNode::array(key);
            node.children = items
                .into_iter()
                .map(|item| json_to_tree_node(None, item))
                .collect();
            node
        }
        JsonValue::String(text) => TreeNode {
            id: 0,
            key,
            kind: TreeNodeKind::Value(TreeScalar::Text(text)),
            expanded: false,
            children: Vec::new(),
        },
        JsonValue::Number(number) => TreeNode {
            id: 0,
            key,
            kind: TreeNodeKind::Value(TreeScalar::Number(number)),
            expanded: false,
            children: Vec::new(),
        },
        JsonValue::Bool(flag) => TreeNode {
            id: 0,
            key,
            kind: TreeNodeKind::Value(TreeScalar::Bool(flag)),
            expanded: false,
            children: Vec::new(),
        },
        JsonValue::Null => TreeNode {
            id: 0,
            key,
            kind: TreeNodeKind::Value(TreeScalar::Null),
            expanded: false,
            children: Vec::new(),
        },
    }
}

fn tree_nodes_to_json(nodes: &[TreeNode]) -> Result<JsonValue, String> {
    if nodes.is_empty() {
        return Ok(JsonValue::Object(Vec::new()));
    }

    if nodes.len() == 1 && nodes[0].key.is_none() {
        return tree_node_to_json(&nodes[0]);
    }

    if nodes.iter().all(|node| node.key.is_some()) {
        let mut entries = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            let key = node
                .key
                .clone()
                .unwrap_or_else(|| format!("field_{}", index + 1));
            entries.push((key, tree_node_to_json(node)?));
        }
        return Ok(JsonValue::Object(entries));
    }

    let mut items = Vec::with_capacity(nodes.len());
    for node in nodes {
        items.push(tree_node_to_json(node)?);
    }
    Ok(JsonValue::Array(items))
}

fn tree_node_to_json(node: &TreeNode) -> Result<JsonValue, String> {
    match &node.kind {
        TreeNodeKind::Object => {
            let mut entries = Vec::with_capacity(node.children.len());
            for (index, child) in node.children.iter().enumerate() {
                let key = child
                    .key
                    .clone()
                    .unwrap_or_else(|| format!("field_{}", index + 1));
                entries.push((key, tree_node_to_json(child)?));
            }
            Ok(JsonValue::Object(entries))
        }
        TreeNodeKind::Array => {
            let mut items = Vec::with_capacity(node.children.len());
            for child in &node.children {
                items.push(tree_node_to_json(child)?);
            }
            Ok(JsonValue::Array(items))
        }
        TreeNodeKind::Value(TreeScalar::Text(text)) => Ok(infer_json_scalar(text)),
        TreeNodeKind::Value(TreeScalar::Number(number)) => {
            if is_valid_json_number(number) {
                Ok(JsonValue::Number(number.clone()))
            } else {
                Err(format!("invalid number literal: {}", number))
            }
        }
        TreeNodeKind::Value(TreeScalar::Bool(flag)) => Ok(JsonValue::Bool(*flag)),
        TreeNodeKind::Value(TreeScalar::Null) => Ok(JsonValue::Null),
    }
}

fn infer_json_scalar(text: &str) -> JsonValue {
    if text == "null" {
        return JsonValue::Null;
    }
    if text == "true" {
        return JsonValue::Bool(true);
    }
    if text == "false" {
        return JsonValue::Bool(false);
    }
    if is_valid_json_number(text) {
        return JsonValue::Number(text.to_string());
    }
    JsonValue::String(text.to_string())
}

fn is_valid_json_number(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;

    if chars[i] == '-' {
        i += 1;
        if i >= chars.len() {
            return false;
        }
    }

    match chars[i] {
        '0' => {
            i += 1;
        }
        '1'..='9' => {
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
        }
        _ => return false,
    }

    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let mut digits = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            digits += 1;
            i += 1;
        }
        if digits == 0 {
            return false;
        }
    }

    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        i += 1;
        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
            i += 1;
        }
        let mut digits = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            digits += 1;
            i += 1;
        }
        if digits == 0 {
            return false;
        }
    }

    i == chars.len()
}

fn write_json_pretty(value: &JsonValue, depth: usize, out: &mut String) {
    match value {
        JsonValue::Object(entries) => {
            out.push('{');
            if entries.is_empty() {
                out.push('}');
                return;
            }

            out.push('\n');
            for (index, (key, value)) in entries.iter().enumerate() {
                push_indent(out, depth + 1);
                out.push('"');
                out.push_str(&escape_json_string(key));
                out.push_str("\": ");
                write_json_pretty(value, depth + 1, out);
                if index + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, depth);
            out.push('}');
        }
        JsonValue::Array(items) => {
            out.push('[');
            if items.is_empty() {
                out.push(']');
                return;
            }

            out.push('\n');
            for (index, item) in items.iter().enumerate() {
                push_indent(out, depth + 1);
                write_json_pretty(item, depth + 1, out);
                if index + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, depth);
            out.push(']');
        }
        JsonValue::String(text) => {
            out.push('"');
            out.push_str(&escape_json_string(text));
            out.push('"');
        }
        JsonValue::Number(number) => out.push_str(number),
        JsonValue::Bool(flag) => {
            if *flag {
                out.push_str("true");
            } else {
                out.push_str("false");
            }
        }
        JsonValue::Null => out.push_str("null"),
    }
}

fn push_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn escape_json_string(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if c < '\u{20}' => {
                let code = c as u32;
                out.push_str("\\u");
                out.push_str(&format!("{:04X}", code));
            }
            _ => out.push(ch),
        }
    }
    out
}

struct JsonParser {
    chars: Vec<char>,
    pos: usize,
}

impl JsonParser {
    fn parse(input: &str) -> Result<JsonValue, String> {
        let mut parser = Self {
            chars: input.chars().collect(),
            pos: 0,
        };
        parser.skip_ws();
        let value = parser.parse_value()?;
        parser.skip_ws();
        if parser.peek().is_some() {
            return Err(parser.error("unexpected trailing characters"));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(JsonValue::String),
            Some('t') => {
                self.expect_keyword("true")?;
                Ok(JsonValue::Bool(true))
            }
            Some('f') => {
                self.expect_keyword("false")?;
                Ok(JsonValue::Bool(false))
            }
            Some('n') => {
                self.expect_keyword("null")?;
                Ok(JsonValue::Null)
            }
            Some('-') | Some('0'..='9') => self.parse_number().map(JsonValue::Number),
            Some(_) => Err(self.error("unexpected token")),
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect_char('{')?;
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(JsonValue::Object(Vec::new()));
        }

        let mut entries = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err(self.error("object key must be a string"));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect_char(':')?;
            self.skip_ws();
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }

        Ok(JsonValue::Object(entries))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect_char('[')?;
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(JsonValue::Array(Vec::new()));
        }

        let mut items = Vec::new();
        loop {
            self.skip_ws();
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }

        Ok(JsonValue::Array(items))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect_char('"')?;
        let mut out = String::new();

        loop {
            let Some(ch) = self.next() else {
                return Err(self.error("unterminated string"));
            };

            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let Some(esc) = self.next() else {
                        return Err(self.error("unterminated escape sequence"));
                    };
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{08}'),
                        'f' => out.push('\u{0C}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let code = self.parse_hex4()?;
                            let decoded = if (0xD800..=0xDBFF).contains(&code) {
                                self.expect_char('\\')?;
                                self.expect_char('u')?;
                                let low = self.parse_hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&low) {
                                    return Err(self.error("invalid unicode surrogate pair"));
                                }
                                let high_ten = code - 0xD800;
                                let low_ten = low - 0xDC00;
                                let scalar = 0x10000 + ((high_ten << 10) | low_ten);
                                char::from_u32(scalar)
                            } else if (0xDC00..=0xDFFF).contains(&code) {
                                None
                            } else {
                                char::from_u32(code)
                            };
                            let Some(decoded) = decoded else {
                                return Err(self.error("invalid unicode escape"));
                            };
                            out.push(decoded);
                        }
                        _ => return Err(self.error("invalid escape sequence")),
                    }
                }
                c if c <= '\u{1F}' => return Err(self.error("control character in string")),
                _ => out.push(ch),
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(ch) = self.next() else {
                return Err(self.error("unterminated unicode escape"));
            };
            let digit = ch
                .to_digit(16)
                .ok_or_else(|| self.error("invalid unicode escape"))?;
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<String, String> {
        let start = self.pos;

        if self.peek() == Some('-') {
            self.pos += 1;
        }

        match self.peek() {
            Some('0') => {
                self.pos += 1;
            }
            Some('1'..='9') => {
                self.pos += 1;
                while matches!(self.peek(), Some('0'..='9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.error("invalid number")),
        }

        if self.peek() == Some('.') {
            self.pos += 1;
            let mut digits = 0usize;
            while matches!(self.peek(), Some('0'..='9')) {
                digits += 1;
                self.pos += 1;
            }
            if digits == 0 {
                return Err(self.error("invalid fraction"));
            }
        }

        if matches!(self.peek(), Some('e' | 'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.pos += 1;
            }
            let mut digits = 0usize;
            while matches!(self.peek(), Some('0'..='9')) {
                digits += 1;
                self.pos += 1;
            }
            if digits == 0 {
                return Err(self.error("invalid exponent"));
            }
        }

        Ok(self.chars[start..self.pos].iter().collect())
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), String> {
        for expected in keyword.chars() {
            if self.next() != Some(expected) {
                return Err(self.error("invalid literal"));
            }
        }
        Ok(())
    }

    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        match self.next() {
            Some(found) if found == expected => Ok(()),
            _ => Err(self.error(&format!("expected '{}'", expected))),
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\n' | '\r' | '\t')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    fn error(&self, message: &str) -> String {
        format!("{} at char {}", message, self.pos)
    }
}
