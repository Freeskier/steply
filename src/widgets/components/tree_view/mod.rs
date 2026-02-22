mod state;

use std::borrow::Cow;

use crate::core::NodeId;
use crate::core::search::fuzzy::match_text;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};

use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::highlight::render_text_spans;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::scroll::ScrollState;
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, RenderContext, TextAction,
};
use state::{rebuild_visible, rebuild_visible_filtered};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItemRenderState {
    pub focused: bool,
    pub active: bool,
    pub has_children: bool,
    pub expanded: bool,
    pub loading: bool,
    pub highlights: Vec<(usize, usize)>,
}

pub trait TreeItemLabel: Send + 'static {
    fn label(&self) -> &str;

    fn search_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.label())
    }

    fn render_spans(&self, state: TreeItemRenderState) -> Vec<Span> {
        let style = if state.focused && state.active {
            Style::new().color(Color::Cyan).bold()
        } else if state.has_children {
            Style::new().color(Color::Blue).bold()
        } else {
            Style::default()
        };
        render_text_spans(
            self.label(),
            state.highlights.as_slice(),
            style,
            Style::new().color(Color::Yellow).bold(),
        )
    }
}

impl TreeItemLabel for String {
    fn label(&self) -> &str {
        self.as_str()
    }
}

impl TreeItemLabel for &'static str {
    fn label(&self) -> &str {
        self
    }
}

pub struct TreeNode<T> {
    pub item: T,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,

    pub children_loaded: bool,
}

impl<T: TreeItemLabel> TreeNode<T> {
    pub fn new(item: T, depth: usize, has_children: bool) -> Self {
        Self {
            item,
            depth,
            has_children,
            expanded: false,
            children_loaded: false,
        }
    }

    pub fn expanded(mut self) -> Self {
        self.expanded = true;
        self
    }
}

pub struct TreeView<T: TreeItemLabel> {
    base: WidgetBase,
    nodes: Vec<TreeNode<T>>,
    visible: Vec<usize>,
    active_index: usize,
    scroll: ScrollState,
    submit_target: Option<ValueTarget>,
    show_label: bool,
    show_indent_guides: bool,
    filter: TextInput,
    filter_visible: bool,
    filter_focus: bool,
    filter_query: String,

    pub pending_expand: Option<usize>,
}

impl<T: TreeItemLabel> TreeView<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>, nodes: Vec<TreeNode<T>>) -> Self {
        let id = id.into();
        let mut this = Self {
            base: WidgetBase::new(id.clone(), label),
            nodes,
            visible: Vec::new(),
            active_index: 0,
            scroll: ScrollState::new(None),
            submit_target: None,
            show_label: true,
            show_indent_guides: false,
            filter: TextInput::new(format!("{id}__filter"), ""),
            filter_visible: false,
            filter_focus: false,
            filter_query: String::new(),
            pending_expand: None,
        };
        this.rebuild();
        this
    }

    pub fn with_max_visible(mut self, max_visible: usize) -> Self {
        self.scroll.max_visible = if max_visible == 0 {
            None
        } else {
            Some(max_visible)
        };
        self.rebuild();
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<NodeId>) -> Self {
        self.submit_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_submit_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.submit_target = Some(ValueTarget::path(root, path));
        self
    }

    pub fn visible_range(&self) -> (usize, usize) {
        self.scroll.visible_range(self.visible.len())
    }

    pub fn active_visible_index(&self) -> usize {
        self.active_index
    }

    pub fn set_active_visible_index(&mut self, index: usize) {
        if self.visible.is_empty() {
            self.active_index = 0;
            self.scroll.offset = 0;
            return;
        }
        self.active_index = index.min(self.visible.len() - 1);
        self.scroll
            .ensure_visible(self.active_index, self.visible.len());
    }

    pub fn visible(&self) -> &[usize] {
        &self.visible
    }

    pub fn with_show_label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }

    pub fn with_indent_guides(mut self, show: bool) -> Self {
        self.show_indent_guides = show;
        self
    }

    pub fn set_indent_guides(&mut self, show: bool) {
        self.show_indent_guides = show;
    }

    pub fn set_nodes(&mut self, nodes: Vec<TreeNode<T>>) {
        self.nodes = nodes;
        self.rebuild();
    }

    pub fn set_filter_query(&mut self, query: impl Into<String>) {
        let next = query.into();
        if self.filter_query != next && !next.trim().is_empty() {
            self.expand_ancestors_for_filter(next.as_str());
        }
        self.filter_query = next;
        self.rebuild();
    }

    pub fn clear_filter(&mut self) {
        self.filter.set_value(Value::Text(String::new()));
        self.filter_query.clear();
        self.rebuild();
    }

    pub fn filter_query(&self) -> &str {
        self.filter_query.as_str()
    }

    fn filter_text(&self) -> String {
        self.filter
            .value()
            .and_then(|value| value.to_text_scalar())
            .unwrap_or_default()
    }

    fn sync_filter_from_input(&mut self) {
        self.filter_query = self.filter_text();
        self.rebuild();
    }

    fn toggle_filter_visibility(&mut self) {
        self.filter_visible = !self.filter_visible;
        if self.filter_visible {
            self.filter_focus = true;
            return;
        }
        self.filter_focus = false;
        self.clear_filter();
    }

    fn child_context(&self, ctx: &RenderContext, focused_id: Option<String>) -> RenderContext {
        RenderContext {
            focused_id,
            terminal_size: ctx.terminal_size,
            visible_errors: ctx.visible_errors.clone(),
            invalid_hidden: ctx.invalid_hidden.clone(),
            completion_menus: ctx.completion_menus.clone(),
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }
        match key.code {
            KeyCode::Esc => {
                self.filter_focus = false;
                InteractionResult::handled()
            }
            KeyCode::Enter | KeyCode::Down => {
                self.filter_focus = false;
                InteractionResult::handled()
            }
            _ => {
                let before = self.filter_text();
                let mut result = self.filter.on_key(key);
                result.actions.retain(|action| {
                    !matches!(action, crate::runtime::event::WidgetAction::InputDone)
                });
                if self.filter_text() != before {
                    self.sync_filter_from_input();
                    return InteractionResult::handled();
                }
                result
            }
        }
    }

    pub fn active_node(&self) -> Option<&TreeNode<T>> {
        self.visible
            .get(self.active_index)
            .and_then(|&idx| self.nodes.get(idx))
    }

    pub fn active_node_idx(&self) -> Option<usize> {
        self.visible.get(self.active_index).copied()
    }

    pub fn nodes(&self) -> &[TreeNode<T>] {
        &self.nodes
    }

    pub fn nodes_mut(&mut self) -> &mut Vec<TreeNode<T>> {
        &mut self.nodes
    }

    pub fn insert_children_after(&mut self, parent_idx: usize, children: Vec<TreeNode<T>>) {
        let Some(parent) = self.nodes.get_mut(parent_idx) else {
            return;
        };
        let child_depth = parent.depth + 1;
        parent.children_loaded = true;
        parent.expanded = true;

        let end = {
            let parent_depth = self.nodes[parent_idx].depth;
            self.nodes[parent_idx + 1..]
                .iter()
                .position(|n| n.depth <= parent_depth)
                .map(|offset| parent_idx + 1 + offset)
                .unwrap_or(self.nodes.len())
        };
        self.nodes.drain(parent_idx + 1..end);

        for (i, mut child) in children.into_iter().enumerate() {
            child.depth = child_depth;
            self.nodes.insert(parent_idx + 1 + i, child);
        }

        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.visible = if self.filter_query.trim().is_empty() {
            rebuild_visible(&self.nodes)
        } else {
            rebuild_visible_filtered(&self.nodes, self.filter_query.as_str())
        };
        ScrollState::clamp_active(&mut self.active_index, self.visible.len());
        self.scroll
            .ensure_visible(self.active_index, self.visible.len());
    }

    fn expand_ancestors_for_filter(&mut self, query: &str) {
        let q = query.trim();
        if q.is_empty() {
            return;
        }

        let mut parents = Vec::<Option<usize>>::with_capacity(self.nodes.len());
        let mut stack = Vec::<usize>::new();
        for (idx, node) in self.nodes.iter().enumerate() {
            stack.truncate(node.depth);
            parents.push(stack.last().copied());
            stack.push(idx);
        }

        for idx in 0..self.nodes.len() {
            let search = self.nodes[idx].item.search_text();
            let matched = match_text(q, search.as_ref()).is_some();
            if !matched {
                continue;
            }
            let mut cur = parents[idx];
            while let Some(parent) = cur {
                if self.nodes[parent].has_children {
                    self.nodes[parent].expanded = true;
                }
                cur = parents[parent];
            }
        }
    }

    pub fn move_active(&mut self, delta: isize) -> bool {
        let len = self.visible.len();
        if len == 0 {
            return false;
        }
        let current = self.active_index as isize;
        let next = ((current + delta + len as isize) % len as isize) as usize;
        if next == self.active_index {
            return false;
        }
        self.active_index = next;
        self.scroll
            .ensure_visible(self.active_index, self.visible.len());
        true
    }

    pub fn expand_active(&mut self) -> bool {
        let Some(&node_idx) = self.visible.get(self.active_index) else {
            return false;
        };
        let node = &self.nodes[node_idx];
        if node.has_children && !node.expanded {
            self.nodes[node_idx].expanded = true;
            self.rebuild();
            true
        } else {
            false
        }
    }

    pub fn collapse_active(&mut self) -> bool {
        let Some(&node_idx) = self.visible.get(self.active_index) else {
            return false;
        };
        let node = &self.nodes[node_idx];

        if node.has_children && node.expanded {
            self.nodes[node_idx].expanded = false;
            self.rebuild();
            return true;
        }

        if node.depth == 0 {
            return false;
        }
        let target_depth = node.depth - 1;
        let parent_node_idx = (0..node_idx)
            .rev()
            .find(|&i| self.nodes[i].depth == target_depth);

        if let Some(parent_node_idx) = parent_node_idx {
            if let Some(pos) = self.visible.iter().position(|&i| i == parent_node_idx) {
                self.active_index = pos;
                self.scroll
                    .ensure_visible(self.active_index, self.visible.len());
                return true;
            }
        }

        false
    }

    pub fn render_lines(&self, focused: bool) -> Vec<Vec<Span>> {
        let mut lines = Vec::new();
        let total = self.visible.len();
        let (start, end) = self.scroll.visible_range(total);

        let inactive_style = Style::new().color(Color::DarkGrey);
        let cursor_style = Style::new().color(Color::Yellow);
        let active_style = Style::new().color(Color::Cyan).bold();
        let loading_style = Style::new().color(Color::Yellow);

        for vis_pos in start..end {
            let node_idx = self.visible[vis_pos];
            let node = &self.nodes[node_idx];

            let active = vis_pos == self.active_index;
            let loading = self.pending_expand == Some(node_idx);

            let cursor = if focused && active { "❯" } else { " " };
            let cursor_span = if focused && active {
                Span::styled(cursor, cursor_style).no_wrap()
            } else {
                Span::styled(cursor, inactive_style).no_wrap()
            };

            let icon = if node.has_children {
                if loading {
                    "⟳ "
                } else if node.expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            let icon_span = if focused && active {
                let icon_st = if loading { loading_style } else { active_style };
                Span::styled(icon, icon_st).no_wrap()
            } else {
                Span::styled(icon, inactive_style).no_wrap()
            };

            let mut line = vec![cursor_span];
            line.extend(self.render_indent_spans(node_idx, node.depth, focused, active));
            line.push(icon_span);
            let highlights = if self.filter_query.trim().is_empty() {
                Vec::new()
            } else {
                match_text(self.filter_query.as_str(), node.item.label())
                    .map(|(_, ranges)| ranges)
                    .unwrap_or_default()
            };
            line.extend(node.item.render_spans(TreeItemRenderState {
                focused,
                active,
                has_children: node.has_children,
                expanded: node.expanded,
                loading,
                highlights,
            }));
            lines.push(line);
        }

        if let Some(text) = self.scroll.footer(total) {
            lines.push(vec![
                Span::styled(text, Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        lines
    }

    fn render_indent_spans(
        &self,
        node_idx: usize,
        depth: usize,
        focused: bool,
        active: bool,
    ) -> Vec<Span> {
        let inactive_style = Style::new().color(Color::DarkGrey);
        let mut spans = Vec::with_capacity(depth + 1);
        if focused && active {
            spans.push(Span::new(" ").no_wrap());
        } else {
            spans.push(Span::styled(" ", inactive_style).no_wrap());
        }

        if depth == 0 {
            return spans;
        }
        if !self.show_indent_guides {
            let indent = "  ".repeat(depth);
            if focused && active {
                spans.push(Span::new(indent).no_wrap());
            } else {
                spans.push(Span::styled(indent, inactive_style).no_wrap());
            }
            return spans;
        }

        let guides = self.ancestor_guides(node_idx, depth);
        for show in guides {
            let segment = if show { "│ " } else { "  " };
            spans.push(Span::styled(segment, inactive_style).no_wrap());
        }
        spans
    }

    fn ancestor_guides(&self, mut node_idx: usize, depth: usize) -> Vec<bool> {
        let mut guides = vec![false; depth];
        let mut cur_depth = depth;
        while cur_depth > 0 {
            let target_depth = cur_depth - 1;
            let Some(parent_idx) = (0..node_idx)
                .rev()
                .find(|&idx| self.nodes[idx].depth == target_depth)
            else {
                break;
            };
            let parent = &self.nodes[parent_idx];
            guides[target_depth] = parent.has_children && parent.expanded;
            node_idx = parent_idx;
            cur_depth = target_depth;
        }
        guides
    }
}

impl<T: TreeItemLabel> Component for TreeView<T> {
    fn children(&self) -> &[Node] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

impl<T: TreeItemLabel> Drawable for TreeView<T> {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let mut lines = Vec::new();

        if self.show_label && !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        if self.filter_visible {
            let filter_ctx = self.child_context(
                ctx,
                if focused && self.filter_focus {
                    Some(self.filter.id().to_string())
                } else {
                    None
                },
            );
            let mut filter_line =
                vec![Span::styled("Filter: ", Style::new().color(Color::DarkGrey)).no_wrap()];
            filter_line.extend(
                self.filter
                    .draw(&filter_ctx)
                    .lines
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| vec![Span::new("").no_wrap()]),
            );
            lines.push(filter_line);
        }

        lines.extend(self.render_lines(focused));
        DrawOutput { lines }
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }
        let mut hints = vec![
            HintItem::new("↑ ↓", "move", HintGroup::Navigation).with_priority(10),
            HintItem::new("→", "expand", HintGroup::Navigation).with_priority(11),
            HintItem::new("←", "collapse / parent", HintGroup::Navigation).with_priority(12),
            HintItem::new("Enter", "select", HintGroup::Action).with_priority(20),
            HintItem::new("Ctrl+F", "toggle filter", HintGroup::View).with_priority(30),
        ];
        if self.filter_focus {
            hints.push(HintItem::new("Esc", "leave filter", HintGroup::View).with_priority(31));
        }
        hints
    }
}

impl<T: TreeItemLabel> Interactive for TreeView<T> {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            self.toggle_filter_visibility();
            return InteractionResult::handled();
        }

        if self.filter_focus {
            return self.handle_filter_key(key);
        }

        if key.modifiers != KeyModifiers::NONE {
            return InteractionResult::ignored();
        }

        match key.code {
            KeyCode::Up => {
                if self.move_active(-1) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Down => {
                if self.move_active(1) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Right => {
                if self.expand_active() {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Left => {
                if self.collapse_active() {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Enter => {
                let Some(value) = self.value() else {
                    return InteractionResult::input_done();
                };
                InteractionResult::submit_or_produce(self.submit_target.as_ref(), value)
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        self.active_node()
            .map(|node| Value::Text(node.item.label().to_string()))
    }

    fn set_value(&mut self, value: Value) {
        let Some(text) = value.to_text_scalar() else {
            return;
        };
        if let Some(pos) = self
            .visible
            .iter()
            .position(|&idx| self.nodes[idx].item.label() == text.as_str())
        {
            self.active_index = pos;
            self.scroll
                .ensure_visible(self.active_index, self.visible.len());
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if !self.filter_focus {
            return InteractionResult::ignored();
        }
        let before = self.filter_text();
        let mut result = self.filter.on_text_action(action);
        result
            .actions
            .retain(|a| !matches!(a, crate::runtime::event::WidgetAction::InputDone));
        if self.filter_text() != before {
            self.sync_filter_from_input();
            return InteractionResult::handled();
        }
        result
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if !self.filter_focus {
            return None;
        }
        self.filter.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if !self.filter_focus {
            return None;
        }
        let local = self.filter.cursor_pos()?;
        let mut row: u16 = 0;
        if self.show_label && !self.base.label().is_empty() {
            row = row.saturating_add(1);
        }
        Some(CursorPos {
            col: local.col.saturating_add(8),
            row,
        })
    }
}
