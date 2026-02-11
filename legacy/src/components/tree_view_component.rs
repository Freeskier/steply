use crate::core::binding::BindTarget;
use crate::core::component::{Component, ComponentBase, ComponentResponse, FocusMode};
use crate::core::value::Value;
use crate::inputs::text_edit;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine, RenderOutput};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeScalar {
    Text(String),
    Number(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNodeKind {
    Object,
    Array,
    Value(TreeScalar),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub id: u64,
    pub key: Option<String>,
    pub kind: TreeNodeKind,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn object(key: Option<String>) -> Self {
        Self {
            id: 0,
            key,
            kind: TreeNodeKind::Object,
            expanded: true,
            children: Vec::new(),
        }
    }

    pub fn array(key: Option<String>) -> Self {
        Self {
            id: 0,
            key,
            kind: TreeNodeKind::Array,
            expanded: true,
            children: Vec::new(),
        }
    }

    pub fn text(key: Option<String>, value: impl Into<String>) -> Self {
        Self {
            id: 0,
            key,
            kind: TreeNodeKind::Value(TreeScalar::Text(value.into())),
            expanded: false,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct VisibleRow {
    path: Vec<usize>,
    depth: usize,
    index_in_parent: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Key,
    Value,
}

#[derive(Debug, Clone)]
struct TreeEditor {
    path: Vec<usize>,
    field: EditField,
    buffer: String,
    cursor: usize,
}

pub struct TreeViewComponent {
    base: ComponentBase,
    nodes: Vec<TreeNode>,
    next_id: u64,
    active_row: usize,
    editor: Option<TreeEditor>,
    active_edit_field: EditField,
    bind_target: Option<BindTarget>,
}

impl TreeViewComponent {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            base: ComponentBase::new(id),
            nodes: Vec::new(),
            next_id: 1,
            active_row: 0,
            editor: None,
            active_edit_field: EditField::Key,
            bind_target: None,
        }
    }

    pub fn with_nodes(mut self, nodes: Vec<TreeNode>) -> Self {
        self.set_nodes(nodes);
        self
    }

    pub fn with_bind_target(mut self, target: BindTarget) -> Self {
        self.bind_target = Some(target);
        self
    }

    pub fn bind_to_input(mut self, id: impl Into<String>) -> Self {
        self.bind_target = Some(BindTarget::Input(id.into()));
        self
    }

    pub fn set_nodes(&mut self, nodes: Vec<TreeNode>) {
        self.nodes = nodes;
        self.reindex_ids();
        self.active_row = 0;
        self.editor = None;
        self.active_edit_field = EditField::Key;
    }

    pub fn nodes(&self) -> &[TreeNode] {
        &self.nodes
    }

    fn active_path(&self) -> Option<Vec<usize>> {
        self.visible_rows()
            .get(self.active_row)
            .map(|row| row.path.clone())
    }

    fn visible_rows(&self) -> Vec<VisibleRow> {
        let mut out = Vec::new();
        Self::collect_visible_rows(&self.nodes, 0, &mut Vec::new(), &mut out);
        out
    }

    fn collect_visible_rows(
        nodes: &[TreeNode],
        depth: usize,
        prefix: &mut Vec<usize>,
        out: &mut Vec<VisibleRow>,
    ) {
        for (idx, node) in nodes.iter().enumerate() {
            prefix.push(idx);
            out.push(VisibleRow {
                path: prefix.clone(),
                depth,
                index_in_parent: idx,
            });
            if node.expanded && Self::is_branch(&node.kind) {
                Self::collect_visible_rows(&node.children, depth + 1, prefix, out);
            }
            let _ = prefix.pop();
        }
    }

    fn clamp_active_row(&mut self) {
        let len = self.visible_rows().len();
        if len == 0 {
            self.active_row = 0;
        } else if self.active_row >= len {
            self.active_row = len - 1;
        }
    }

    fn reindex_ids(&mut self) {
        let mut next = 1u64;
        for node in &mut self.nodes {
            Self::assign_ids_recursive(node, &mut next);
        }
        self.next_id = next;
    }

    fn assign_ids_recursive(node: &mut TreeNode, next: &mut u64) {
        node.id = *next;
        *next += 1;
        for child in &mut node.children {
            Self::assign_ids_recursive(child, next);
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn new_leaf(&mut self, key: Option<String>) -> TreeNode {
        TreeNode {
            id: self.alloc_id(),
            key,
            kind: TreeNodeKind::Value(TreeScalar::Text(String::new())),
            expanded: false,
            children: Vec::new(),
        }
    }

    fn is_branch(kind: &TreeNodeKind) -> bool {
        matches!(kind, TreeNodeKind::Object | TreeNodeKind::Array)
    }

    fn node_by_path<'a>(nodes: &'a [TreeNode], path: &[usize]) -> Option<&'a TreeNode> {
        if path.is_empty() {
            return None;
        }

        let idx = *path.first()?;
        let node = nodes.get(idx)?;
        if path.len() == 1 {
            return Some(node);
        }

        Self::node_by_path(&node.children, &path[1..])
    }

    fn node_by_path_mut<'a>(nodes: &'a mut [TreeNode], path: &[usize]) -> Option<&'a mut TreeNode> {
        if path.is_empty() {
            return None;
        }

        let idx = *path.first()?;
        let node = nodes.get_mut(idx)?;
        if path.len() == 1 {
            return Some(node);
        }

        Self::node_by_path_mut(node.children.as_mut_slice(), &path[1..])
    }

    fn children_of_path_mut<'a>(
        nodes: &'a mut Vec<TreeNode>,
        parent_path: &[usize],
    ) -> Option<&'a mut Vec<TreeNode>> {
        if parent_path.is_empty() {
            return Some(nodes);
        }

        let parent = Self::node_by_path_mut(nodes.as_mut_slice(), parent_path)?;
        Some(&mut parent.children)
    }

    fn find_row_index_by_path(&self, path: &[usize]) -> Option<usize> {
        self.visible_rows()
            .iter()
            .position(|row| row.path.as_slice() == path)
    }

    fn move_active(&mut self, delta: isize) -> bool {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return false;
        }

        let len = rows.len() as isize;
        let current = self.active_row as isize;
        let next = (current + delta).clamp(0, len - 1) as usize;
        if next == self.active_row {
            return false;
        }
        self.active_row = next;
        true
    }

    fn toggle_expand_active(&mut self) -> bool {
        let Some(path) = self.active_path() else {
            return false;
        };
        let Some(node) = Self::node_by_path_mut(self.nodes.as_mut_slice(), &path) else {
            return false;
        };
        if !Self::is_branch(&node.kind) {
            return false;
        }
        node.expanded = !node.expanded;
        true
    }

    fn handle_left(&mut self) -> bool {
        let Some(path) = self.active_path() else {
            return false;
        };

        if let Some(node) = Self::node_by_path_mut(self.nodes.as_mut_slice(), &path)
            && Self::is_branch(&node.kind)
            && node.expanded
        {
            node.expanded = false;
            return true;
        }

        if path.len() > 1 {
            let parent_path = &path[..path.len() - 1];
            if let Some(index) = self.find_row_index_by_path(parent_path) {
                self.active_row = index;
                return true;
            }
        }

        false
    }

    fn handle_right(&mut self) -> bool {
        let Some(path) = self.active_path() else {
            return false;
        };

        let Some(node) = Self::node_by_path_mut(self.nodes.as_mut_slice(), &path) else {
            return false;
        };

        if !Self::is_branch(&node.kind) {
            return false;
        }

        if !node.expanded {
            node.expanded = true;
            return true;
        }

        if node.children.is_empty() {
            return false;
        }

        let mut child_path = path;
        child_path.push(0);
        if let Some(index) = self.find_row_index_by_path(&child_path) {
            self.active_row = index;
            return true;
        }

        false
    }

    fn add_child(&mut self) -> bool {
        let Some(path) = self.active_path() else {
            let root = self.new_leaf(Some("node".to_string()));
            self.nodes.push(root);
            self.clamp_active_row();
            return true;
        };

        let parent_kind = Self::node_by_path(self.nodes.as_slice(), &path)
            .map(|node| node.kind.clone())
            .unwrap_or(TreeNodeKind::Object);

        let new_key = match parent_kind {
            TreeNodeKind::Array => None,
            _ => Some("new_key".to_string()),
        };
        let new_node = self.new_leaf(new_key);

        let mut new_path = path.clone();

        if let Some(parent) = Self::node_by_path_mut(self.nodes.as_mut_slice(), &path) {
            if !Self::is_branch(&parent.kind) {
                parent.kind = TreeNodeKind::Object;
                parent.children.clear();
            }
            parent.expanded = true;
            parent.children.push(new_node);
            new_path.push(parent.children.len() - 1);
        } else {
            return false;
        }

        if let Some(index) = self.find_row_index_by_path(&new_path) {
            self.active_row = index;
        }
        true
    }

    fn add_sibling(&mut self) -> bool {
        let Some(path) = self.active_path() else {
            let root = self.new_leaf(Some("node".to_string()));
            self.nodes.push(root);
            self.clamp_active_row();
            return true;
        };

        let idx = *path.last().unwrap_or(&0);
        let parent_path = if path.len() > 1 {
            &path[..path.len() - 1]
        } else {
            &[]
        };

        let parent_kind = if parent_path.is_empty() {
            TreeNodeKind::Object
        } else {
            Self::node_by_path(self.nodes.as_slice(), parent_path)
                .map(|node| node.kind.clone())
                .unwrap_or(TreeNodeKind::Object)
        };

        let sibling_key = match parent_kind {
            TreeNodeKind::Array => None,
            _ => Some("new_key".to_string()),
        };
        let new_node = self.new_leaf(sibling_key);

        let Some(siblings) = Self::children_of_path_mut(&mut self.nodes, parent_path) else {
            return false;
        };

        let insert_at = (idx + 1).min(siblings.len());
        siblings.insert(insert_at, new_node);

        let mut new_path = parent_path.to_vec();
        new_path.push(insert_at);

        if let Some(index) = self.find_row_index_by_path(&new_path) {
            self.active_row = index;
        }
        true
    }

    fn remove_active(&mut self) -> bool {
        let Some(path) = self.active_path() else {
            return false;
        };

        if path.len() == 1 {
            let idx = path[0];
            if idx < self.nodes.len() {
                self.nodes.remove(idx);
                self.editor = None;
                self.clamp_active_row();
                return true;
            }
            return false;
        }

        let idx = *path.last().unwrap_or(&0);
        let parent_path = &path[..path.len() - 1];
        let Some(parent) = Self::node_by_path_mut(self.nodes.as_mut_slice(), parent_path) else {
            return false;
        };
        if idx < parent.children.len() {
            parent.children.remove(idx);
            self.editor = None;
            self.clamp_active_row();
            return true;
        }
        false
    }

    fn supports_field(node: &TreeNode, field: EditField) -> bool {
        match field {
            EditField::Key => node.key.is_some(),
            EditField::Value => matches!(node.kind, TreeNodeKind::Value(_)),
        }
    }

    fn preferred_field_for_node(node: &TreeNode) -> Option<EditField> {
        if node.key.is_some() {
            Some(EditField::Key)
        } else if matches!(node.kind, TreeNodeKind::Value(_)) {
            Some(EditField::Value)
        } else {
            None
        }
    }

    fn active_node(&self) -> Option<&TreeNode> {
        let path = self.active_path()?;
        Self::node_by_path(self.nodes.as_slice(), &path)
    }

    fn sync_editor_to_active(&mut self) {
        let Some(path) = self.active_path() else {
            self.editor = None;
            return;
        };
        let Some(node) = self.active_node() else {
            self.editor = None;
            return;
        };

        let target_field = if Self::supports_field(node, self.active_edit_field) {
            Some(self.active_edit_field)
        } else {
            Self::preferred_field_for_node(node)
        };

        let Some(target_field) = target_field else {
            self.editor = None;
            return;
        };

        if let Some(editor) = &self.editor
            && editor.path == path
            && editor.field == target_field
        {
            return;
        }

        let _ = self.commit_edit();
        self.active_edit_field = target_field;
        let _ = self.begin_edit(target_field);
    }

    fn switch_active_edit_field(&mut self) -> bool {
        let Some(node) = self.active_node() else {
            return false;
        };
        let has_key = node.key.is_some();
        let has_value = matches!(node.kind, TreeNodeKind::Value(_));
        if !(has_key && has_value) {
            return false;
        }

        let _ = self.commit_edit();
        self.active_edit_field = match self.active_edit_field {
            EditField::Key => EditField::Value,
            EditField::Value => EditField::Key,
        };
        self.begin_edit(self.active_edit_field)
    }

    fn begin_edit(&mut self, field: EditField) -> bool {
        let Some(path) = self.active_path() else {
            return false;
        };
        let Some(node) = Self::node_by_path(self.nodes.as_slice(), &path) else {
            return false;
        };

        if !Self::supports_field(node, field) {
            return false;
        }

        let buffer = match field {
            EditField::Key => node.key.clone().unwrap_or_default(),
            EditField::Value => match &node.kind {
                TreeNodeKind::Value(TreeScalar::Text(text)) => text.clone(),
                TreeNodeKind::Value(TreeScalar::Number(number)) => number.clone(),
                TreeNodeKind::Value(TreeScalar::Bool(value)) => value.to_string(),
                TreeNodeKind::Value(TreeScalar::Null) => "null".to_string(),
                TreeNodeKind::Object => String::new(),
                TreeNodeKind::Array => String::new(),
            },
        };

        self.editor = Some(TreeEditor {
            path,
            field,
            cursor: text_edit::char_count(&buffer),
            buffer,
        });

        true
    }

    fn commit_edit(&mut self) -> bool {
        let Some(editor) = self.editor.take() else {
            return false;
        };

        let Some(node) = Self::node_by_path_mut(self.nodes.as_mut_slice(), &editor.path) else {
            return false;
        };

        match editor.field {
            EditField::Key => {
                node.key = if editor.buffer.is_empty() {
                    None
                } else {
                    Some(editor.buffer)
                };
            }
            EditField::Value => {
                node.kind = TreeNodeKind::Value(Self::parse_scalar(editor.buffer));
                node.children.clear();
                node.expanded = false;
            }
        }

        true
    }

    fn handle_editor_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let Some(editor) = self.editor.as_mut() else {
            return false;
        };

        match code {
            KeyCode::Esc => {
                self.editor = None;
                true
            }
            KeyCode::Enter => self.commit_edit(),
            KeyCode::Left => {
                if editor.cursor > 0 {
                    editor.cursor -= 1;
                }
                true
            }
            KeyCode::Right => {
                let len = text_edit::char_count(&editor.buffer);
                if editor.cursor < len {
                    editor.cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                editor.cursor = 0;
                true
            }
            KeyCode::End => {
                editor.cursor = text_edit::char_count(&editor.buffer);
                true
            }
            KeyCode::Backspace => text_edit::backspace_char(&mut editor.buffer, &mut editor.cursor),
            KeyCode::Delete => text_edit::delete_char(&mut editor.buffer, &mut editor.cursor),
            KeyCode::Char(ch) => {
                if modifiers == KeyModifiers::NONE && !ch.is_control() {
                    text_edit::insert_char(&mut editor.buffer, &mut editor.cursor, ch);
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn scalar_value_text(scalar: &TreeScalar) -> String {
        match scalar {
            TreeScalar::Text(text) => text.clone(),
            TreeScalar::Number(number) => number.clone(),
            TreeScalar::Bool(value) => value.to_string(),
            TreeScalar::Null => "null".to_string(),
        }
    }

    fn parse_scalar(raw: String) -> TreeScalar {
        let trimmed = raw.trim();
        if trimmed == raw {
            if trimmed == "null" {
                return TreeScalar::Null;
            }
            if trimmed == "true" {
                return TreeScalar::Bool(true);
            }
            if trimmed == "false" {
                return TreeScalar::Bool(false);
            }
            if Self::is_number_literal(trimmed) {
                return TreeScalar::Number(trimmed.to_string());
            }
        }
        TreeScalar::Text(raw)
    }

    fn is_number_literal(input: &str) -> bool {
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

    fn node_value_preview(node: &TreeNode) -> String {
        match &node.kind {
            TreeNodeKind::Object => {
                let suffix = if node.children.len() == 1 { "" } else { "s" };
                format!("Object: {} item{}", node.children.len(), suffix)
            }
            TreeNodeKind::Array => {
                let suffix = if node.children.len() == 1 { "" } else { "s" };
                format!("Array: {} item{}", node.children.len(), suffix)
            }
            TreeNodeKind::Value(TreeScalar::Text(text)) => text.clone(),
            TreeNodeKind::Value(TreeScalar::Number(number)) => number.clone(),
            TreeNodeKind::Value(TreeScalar::Bool(value)) => value.to_string(),
            TreeNodeKind::Value(TreeScalar::Null) => "null".to_string(),
        }
    }

    fn node_icon(node: &TreeNode) -> &'static str {
        match node.kind {
            TreeNodeKind::Object | TreeNodeKind::Array => {
                if node.expanded {
                    "▾"
                } else {
                    "▸"
                }
            }
            TreeNodeKind::Value(_) => " ",
        }
    }

    fn render_line_spans(
        node: &TreeNode,
        row: &VisibleRow,
        active: bool,
        ctx: &RenderContext,
    ) -> Vec<Span> {
        let marker = if active { "> " } else { "  " };
        let indent = "  ".repeat(row.depth);
        let mut spans = vec![Span::new(format!(
            "{}{}{} ",
            marker,
            indent,
            Self::node_icon(node)
        ))];

        match &node.kind {
            TreeNodeKind::Object => {
                if let Some(key) = &node.key {
                    spans.push(Span::new(format!("[{}]: Object: ", key)));
                } else {
                    spans.push(Span::new("Object: "));
                }
                let suffix = if node.children.len() == 1 { "" } else { "s" };
                spans.push(
                    Span::new(format!("{} item{}", node.children.len(), suffix))
                        .with_style(Style::new().with_color(Color::DarkGrey)),
                );
            }
            TreeNodeKind::Array => {
                if let Some(key) = &node.key {
                    spans.push(Span::new(format!("[{}]: Array: ", key)));
                } else {
                    spans.push(Span::new("Array: "));
                }
                let suffix = if node.children.len() == 1 { "" } else { "s" };
                spans.push(
                    Span::new(format!("{} item{}", node.children.len(), suffix))
                        .with_style(Style::new().with_color(Color::DarkGrey)),
                );
            }
            TreeNodeKind::Value(_) => {
                let key = node
                    .key
                    .as_ref()
                    .map(|key| format!("[{}]", key))
                    .unwrap_or_else(|| format!("[{}]", row.index_in_parent));
                spans.push(Span::new(format!("{}: ", key)));
                spans.push(Span::new(Self::node_value_preview(node)));
            }
        }

        if active {
            for span in &mut spans {
                let merged = span.style().clone().merge(&ctx.theme().focused);
                *span = span.clone().with_style(merged);
            }
        }

        spans
    }

    fn display_cursor_offset(text: &str, cursor: usize) -> usize {
        text.chars()
            .take(cursor)
            .map(|ch| ch.to_string().width())
            .sum()
    }

    fn render_editor_line(
        &self,
        row: &VisibleRow,
        node: &TreeNode,
        editor: &TreeEditor,
        active: bool,
        ctx: &RenderContext,
    ) -> (RenderLine, usize) {
        let marker = if active { "> " } else { "  " };
        let indent = "  ".repeat(row.depth);
        let icon = Self::node_icon(node);
        let mut pre = format!("{}{}{} ", marker, indent, icon);
        let mut post = String::new();

        match editor.field {
            EditField::Key => {
                post = format!("]: {}", Self::node_value_preview(node));
                pre.push('[');
            }
            EditField::Value => {
                let key = node
                    .key
                    .as_ref()
                    .map(|key| format!("[{}]", key))
                    .unwrap_or_else(|| format!("[{}]", row.index_in_parent));
                pre.push_str(&format!("{}: ", key));
            }
        };

        let cursor_col = pre.width() + Self::display_cursor_offset(&editor.buffer, editor.cursor);

        let mut spans = vec![Span::new(pre)];
        let mut edit_span = Span::new(editor.buffer.clone());
        if active {
            edit_span = edit_span.with_style(ctx.theme().focused.clone());
        }
        spans.push(edit_span);
        if !post.is_empty() {
            spans.push(Span::new(post).with_style(Style::new().with_color(Color::DarkGrey)));
        }

        if active {
            for span in &mut spans {
                let merged = span.style().clone().merge(&ctx.theme().focused);
                *span = span.clone().with_style(merged);
            }
        }

        (RenderLine { spans }, cursor_col)
    }

    fn flatten_value(&self) -> Value {
        let mut out = Vec::new();
        for (idx, node) in self.nodes.iter().enumerate() {
            self.flatten_node(node, "", idx, &mut out);
        }
        Value::Map(out)
    }

    fn flatten_node(
        &self,
        node: &TreeNode,
        prefix: &str,
        index: usize,
        out: &mut Vec<(String, String)>,
    ) {
        let segment = match &node.key {
            Some(key) => {
                if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                }
            }
            None => {
                if prefix.is_empty() {
                    format!("[{}]", index)
                } else {
                    format!("{}[{}]", prefix, index)
                }
            }
        };

        match &node.kind {
            TreeNodeKind::Value(scalar) => {
                out.push((segment, Self::scalar_value_text(scalar)));
            }
            TreeNodeKind::Object | TreeNodeKind::Array => {
                for (child_idx, child) in node.children.iter().enumerate() {
                    self.flatten_node(child, &segment, child_idx, out);
                }
            }
        }
    }
}

impl Component for TreeViewComponent {
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
        let rows = self.visible_rows();
        if rows.is_empty() {
            return RenderOutput::from_line(RenderLine {
                spans: vec![
                    Span::new("(empty tree) ").with_style(Style::new().with_color(Color::DarkGrey)),
                    Span::new("press Ctrl+A to add node").with_style(ctx.theme().hint.clone()),
                ],
            });
        }

        let mut lines = Vec::new();
        let mut cursor = None;

        for (line_idx, row) in rows.iter().enumerate() {
            let Some(node) = Self::node_by_path(self.nodes.as_slice(), &row.path) else {
                continue;
            };

            let is_active = self.base.focused && line_idx == self.active_row;

            if let Some(editor) = &self.editor
                && editor.path == row.path
            {
                let (mut line, cursor_col) =
                    self.render_editor_line(row, node, editor, is_active, ctx);
                if is_active {
                    for span in &mut line.spans {
                        let merged = span.style().clone().merge(&ctx.theme().focused);
                        *span = span.clone().with_style(merged);
                    }
                    cursor = Some((line_idx, cursor_col));
                }
                lines.push(line);
                continue;
            }

            let mut spans = Self::render_line_spans(node, row, is_active, ctx);
            if is_active {
                let offset = spans.iter().map(|span| span.width()).sum();
                cursor = Some((line_idx, offset));
            }
            lines.push(RenderLine {
                spans: std::mem::take(&mut spans),
            });
        }

        let mut output = RenderOutput::from_lines(lines);
        if let Some((line, offset)) = cursor {
            output = output.with_cursor(line, offset);
        }
        output
    }

    fn value(&self) -> Option<Value> {
        Some(self.flatten_value())
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::Map(items) => {
                let mut nodes = Vec::new();
                for (key, value) in items {
                    nodes.push(TreeNode::text(Some(key), value));
                }
                self.set_nodes(nodes);
            }
            Value::Text(text) => {
                self.set_nodes(vec![TreeNode::text(Some("value".to_string()), text)]);
            }
            Value::List(items) => {
                let mut root = TreeNode::array(Some("items".to_string()));
                root.children = items
                    .into_iter()
                    .map(|item| TreeNode::text(None, item))
                    .collect();
                self.set_nodes(vec![root]);
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        if modifiers == KeyModifiers::CONTROL {
            let handled = match code {
                KeyCode::Char('a') => {
                    let _ = self.commit_edit();
                    let handled = self.add_child();
                    if handled {
                        self.sync_editor_to_active();
                    }
                    handled
                }
                KeyCode::Char('i') => {
                    let _ = self.commit_edit();
                    let handled = self.add_sibling();
                    if handled {
                        self.sync_editor_to_active();
                    }
                    handled
                }
                KeyCode::Char('d') => {
                    let _ = self.commit_edit();
                    let handled = self.remove_active();
                    if handled {
                        self.sync_editor_to_active();
                    }
                    handled
                }
                _ => false,
            };

            return if handled {
                self.clamp_active_row();
                ComponentResponse::handled()
            } else {
                ComponentResponse::not_handled()
            };
        }

        if modifiers != KeyModifiers::NONE {
            return ComponentResponse::not_handled();
        }

        let handled = match code {
            KeyCode::Up => {
                let _ = self.commit_edit();
                let handled = self.move_active(-1);
                if handled {
                    self.sync_editor_to_active();
                }
                handled
            }
            KeyCode::Down => {
                let _ = self.commit_edit();
                let handled = self.move_active(1);
                if handled {
                    self.sync_editor_to_active();
                }
                handled
            }
            KeyCode::Left => {
                if self.editor.is_some() && self.handle_editor_key(code, modifiers) {
                    true
                } else {
                    let _ = self.commit_edit();
                    let handled = self.handle_left();
                    if handled {
                        self.sync_editor_to_active();
                    }
                    handled
                }
            }
            KeyCode::Right => {
                if self.editor.is_some() && self.handle_editor_key(code, modifiers) {
                    true
                } else {
                    let _ = self.commit_edit();
                    let handled = self.handle_right();
                    if handled {
                        self.sync_editor_to_active();
                    }
                    handled
                }
            }
            KeyCode::Tab => self.switch_active_edit_field(),
            KeyCode::Char(' ') => {
                if self.editor.is_some() && self.handle_editor_key(code, modifiers) {
                    true
                } else {
                    let _ = self.commit_edit();
                    self.toggle_expand_active()
                }
            }
            KeyCode::Enter => {
                if self.editor.is_none() {
                    self.sync_editor_to_active();
                }
                if self.editor.is_some() {
                    let handled = self.handle_editor_key(code, modifiers);
                    if handled {
                        self.sync_editor_to_active();
                    }
                    handled
                } else {
                    false
                }
            }
            KeyCode::Esc
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Char(_) => {
                if self.editor.is_none() {
                    self.sync_editor_to_active();
                }
                if self.editor.is_some() {
                    self.handle_editor_key(code, modifiers)
                } else {
                    false
                }
            }
            _ => false,
        };

        if handled {
            self.clamp_active_row();
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        if focused {
            self.sync_editor_to_active();
        } else {
            let _ = self.commit_edit();
            self.editor = None;
        }
    }
}
