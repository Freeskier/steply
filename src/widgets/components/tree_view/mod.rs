mod state;

use std::borrow::Cow;

use crate::core::NodeId;
use crate::core::search::fuzzy::match_text;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::runtime::event::WidgetAction;

use crate::terminal::{
    CursorPos, KeyCode, KeyEvent, PointerButton, PointerEvent, PointerKind, PointerSemantic,
};
use crate::ui::highlight::render_text_spans;
use crate::ui::layout::Layout;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::scroll::ScrollState;
use crate::widgets::node::LeafComponent;
use crate::widgets::shared::cursor_anchor;
use crate::widgets::shared::filter;
use crate::widgets::shared::keymap;
use crate::widgets::shared::list_core;
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, PointerRowMap, RenderContext, TextAction,
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
    filter: filter::FilterController,
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
            filter: filter::FilterController::new(format!("{id}__filter")),
            filter_query: String::new(),
            pending_expand: None,
        };
        this.rebuild();
        this
    }

    pub fn with_max_visible(mut self, max_visible: usize) -> Self {
        self.scroll.set_max_visible(max_visible);
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
        self.scroll
            .set_active_clamped(&mut self.active_index, self.visible.len(), index);
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
        self.filter.clear();
        self.filter_query.clear();
        self.rebuild();
    }

    pub fn filter_query(&self) -> &str {
        self.filter_query.as_str()
    }

    fn sync_filter_from_input(&mut self) {
        self.filter_query = self.filter.query();
        self.rebuild();
    }

    fn toggle_filter_visibility(&mut self) {
        if !list_core::toggle_filter_visibility(&mut self.filter, false) {
            self.clear_filter();
        }
    }

    fn handled_with_focus(&self) -> InteractionResult {
        let mut result = InteractionResult::handled();
        result.actions.push(WidgetAction::RequestFocus {
            target: self.base.id().to_string().into(),
        });
        result
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> InteractionResult {
        match self
            .filter
            .handle_key_with_change(key, filter::FilterEscBehavior::Blur)
        {
            filter::FilterKeyOutcome::Ignored => InteractionResult::ignored(),
            filter::FilterKeyOutcome::Hide | filter::FilterKeyOutcome::Blur => {
                self.filter.set_focused(false);
                InteractionResult::handled()
            }
            filter::FilterKeyOutcome::Edited(outcome) => {
                outcome.refresh_if_changed(|| self.sync_filter_from_input())
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
        self.scroll
            .clamp_and_ensure(&mut self.active_index, self.visible.len());
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
        self.scroll
            .move_active_wrapped(&mut self.active_index, self.visible.len(), delta)
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

        if let Some(parent_node_idx) = parent_node_idx
            && let Some(pos) = self.visible.iter().position(|&i| i == parent_node_idx)
        {
            self.scroll
                .set_active_clamped(&mut self.active_index, self.visible.len(), pos);
            return true;
        }

        false
    }

    fn render_visible_line(&self, vis_pos: usize, focused: bool) -> Vec<Span> {
        let inactive_style = Style::new().color(Color::DarkGrey);
        let cursor_style = Style::new().color(Color::Yellow);
        let active_style = Style::new().color(Color::Cyan).bold();
        let loading_style = Style::new().color(Color::Yellow);

        let Some(node_idx) = self.visible.get(vis_pos).copied() else {
            return Vec::new();
        };
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
        line
    }

    pub fn render_lines(&self, focused: bool) -> Vec<Vec<Span>> {
        let mut lines = Vec::new();
        let total = self.visible.len();
        let (start, end) = self.scroll.visible_range(total);
        for vis_pos in start..end {
            lines.push(self.render_visible_line(vis_pos, focused));
        }

        let placeholders = self.scroll.placeholder_count(total);
        for _ in 0..placeholders {
            lines.push(vec![Span::new(" ").no_wrap()]);
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

    fn marker_cursor_pos(&self) -> Option<CursorPos> {
        if self.visible.is_empty() {
            return None;
        }
        let (start, end) = self.scroll.visible_range(self.visible.len());
        let mut row = 0usize;
        if self.show_label && !self.base.label().is_empty() {
            row += 1;
        }
        if self.filter.is_visible() {
            row += 1;
        }
        cursor_anchor::visible_row_cursor(self.active_index, start, end, row, 0)
    }

    fn pointer_rows_for_draw(&self, wrap_width: u16) -> Vec<PointerRowMap> {
        let mut rows = Vec::<PointerRowMap>::new();
        let mut rendered_row = 0u16;

        if self.show_label && !self.base.label().is_empty() {
            rendered_row = rendered_row.saturating_add(1);
        }
        if self.filter.is_visible() {
            rows.push(PointerRowMap::new(rendered_row, 0).with_semantic(PointerSemantic::Filter));
            rendered_row = rendered_row.saturating_add(1);
        }

        let total = self.visible.len();
        let (start, end) = self.scroll.visible_range(total);
        for vis_pos in start..end {
            let line = self.render_visible_line(vis_pos, false);
            let wrapped = Layout::compose(std::slice::from_ref(&line), wrap_width)
                .len()
                .max(1);
            let base_row = vis_pos.min((u16::MAX - 1) as usize) as u16;
            for wrapped_idx in 0..wrapped {
                let semantic = if wrapped_idx == 0 {
                    PointerSemantic::None
                } else {
                    PointerSemantic::WrappedContinuation
                };
                rows.push(PointerRowMap::new(rendered_row, base_row).with_semantic(semantic));
                rendered_row = rendered_row.saturating_add(1);
            }
        }

        rows
    }

    fn icon_col_range(depth: usize) -> (u16, u16) {
        let start = (depth.saturating_mul(2).saturating_add(2)).min(u16::MAX as usize) as u16;
        (start, start.saturating_add(2))
    }

    fn handle_pointer_left_down(&mut self, event: PointerEvent) -> InteractionResult {
        if event.semantic == PointerSemantic::Filter {
            self.filter.set_focused(true);
            return self.handled_with_focus();
        }

        self.filter.set_focused(false);
        let continuation = event.semantic == PointerSemantic::WrappedContinuation;
        let vis_pos = event.row as usize;
        let Some(node_idx) = self.visible.get(vis_pos).copied() else {
            return InteractionResult::ignored();
        };
        self.scroll
            .set_active_clamped(&mut self.active_index, self.visible.len(), vis_pos);

        let depth = self.nodes.get(node_idx).map(|node| node.depth).unwrap_or(0);
        let has_children = self
            .nodes
            .get(node_idx)
            .is_some_and(|node| node.has_children);
        if has_children {
            let (icon_start, icon_end) = Self::icon_col_range(depth);
            if !continuation && event.col >= icon_start && event.col < icon_end {
                if let Some(node) = self.nodes.get_mut(node_idx) {
                    node.expanded = !node.expanded;
                }
                self.rebuild();
                if let Some(pos) = self.visible.iter().position(|idx| *idx == node_idx) {
                    self.scroll
                        .set_active_clamped(&mut self.active_index, self.visible.len(), pos);
                }
            }
        }

        self.handled_with_focus()
    }
}

impl<T: TreeItemLabel> LeafComponent for TreeView<T> {}

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

        if self.filter.is_visible() {
            lines.push(filter::render_filter_line(&self.filter, ctx, focused));
        }

        lines.extend(self.render_lines(focused));
        DrawOutput::with_lines(lines)
    }

    fn pointer_rows(&self, ctx: &RenderContext) -> Vec<PointerRowMap> {
        self.pointer_rows_for_draw(ctx.terminal_size.width.max(1))
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
        if self.filter.is_focused() {
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
        if keymap::is_ctrl_char(key, 'f') {
            self.toggle_filter_visibility();
            return InteractionResult::handled();
        }

        if self.filter.is_focused() {
            return self.handle_filter_key(key);
        }

        if !keymap::has_no_modifiers(key) {
            return InteractionResult::ignored();
        }

        match key.code {
            KeyCode::Up => InteractionResult::handled_if(self.move_active(-1)),
            KeyCode::Down => InteractionResult::handled_if(self.move_active(1)),
            KeyCode::Right => InteractionResult::handled_if(self.expand_active()),
            KeyCode::Left => InteractionResult::handled_if(self.collapse_active()),
            KeyCode::Enter => {
                let Some(value) = self.value() else {
                    return InteractionResult::input_done();
                };
                InteractionResult::submit_or_produce(self.submit_target.as_ref(), value)
            }
            _ => InteractionResult::ignored(),
        }
    }

    fn on_pointer(&mut self, event: PointerEvent) -> InteractionResult {
        match event.kind {
            PointerKind::Down(PointerButton::Left) => self.handle_pointer_left_down(event),
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
            self.scroll
                .set_active_clamped(&mut self.active_index, self.visible.len(), pos);
        }
    }

    fn on_text_action(&mut self, action: TextAction) -> InteractionResult {
        if !self.filter.is_focused() {
            return InteractionResult::ignored();
        }
        self.filter
            .handle_text_action_with_change(action)
            .refresh_if_changed(|| self.sync_filter_from_input())
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        if !self.filter.is_focused() {
            return None;
        }
        self.filter.completion()
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        if self.filter.is_focused() {
            let local = self.filter.cursor_pos()?;
            let mut row: u16 = 0;
            if self.show_label && !self.base.label().is_empty() {
                row = row.saturating_add(1);
            }
            return Some(CursorPos {
                col: local.col.saturating_add(8),
                row,
            });
        }
        self.marker_cursor_pos()
    }

    fn cursor_visible(&self) -> bool {
        self.filter.is_focused()
    }
}
