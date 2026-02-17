mod state;

use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::core::NodeId;

use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::scroll::ScrollState;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext,
};
use state::rebuild_visible;

pub trait TreeItemLabel: Send + 'static {
    fn label(&self) -> &str;
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
    /// For lazy-loading: true once children have been fetched and inserted.
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
    /// node_idx pending a lazy-load scan (shows ⟳ icon while loading).
    pub pending_expand: Option<usize>,
}

impl<T: TreeItemLabel> TreeView<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>, nodes: Vec<TreeNode<T>>) -> Self {
        let mut this = Self {
            base: WidgetBase::new(id, label),
            nodes,
            visible: Vec::new(),
            active_index: 0,
            scroll: ScrollState::new(None),
            submit_target: None,
            show_label: true,
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

    /// Returns (start, end) of the currently visible window into the visible list.
    pub fn visible_range(&self) -> (usize, usize) {
        self.scroll.visible_range(self.visible.len())
    }

    /// The active index within the visible list (not nodes[]).
    pub fn active_visible_index(&self) -> usize {
        self.active_index
    }

    /// The visible list: indices into nodes[].
    pub fn visible(&self) -> &[usize] {
        &self.visible
    }

    pub fn with_show_label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }

    pub fn set_nodes(&mut self, nodes: Vec<TreeNode<T>>) {
        self.nodes = nodes;
        self.rebuild();
    }

    pub fn active_node(&self) -> Option<&TreeNode<T>> {
        self.visible
            .get(self.active_index)
            .and_then(|&idx| self.nodes.get(idx))
    }

    /// Index into `nodes[]` of the currently active visible node.
    pub fn active_node_idx(&self) -> Option<usize> {
        self.visible.get(self.active_index).copied()
    }

    /// Direct access to the flat node list.
    pub fn nodes(&self) -> &[TreeNode<T>] {
        &self.nodes
    }

    pub fn nodes_mut(&mut self) -> &mut Vec<TreeNode<T>> {
        &mut self.nodes
    }

    /// Insert `children` right after `parent_idx`, removing any previously
    /// loaded children first. Marks the parent as expanded + loaded.
    pub fn insert_children_after(&mut self, parent_idx: usize, children: Vec<TreeNode<T>>) {
        let Some(parent) = self.nodes.get_mut(parent_idx) else {
            return;
        };
        let child_depth = parent.depth + 1;
        parent.children_loaded = true;
        parent.expanded = true;

        // Remove stale children (nodes after parent with depth >= child_depth
        // that are still within the subtree).
        let end = {
            let parent_depth = self.nodes[parent_idx].depth;
            self.nodes[parent_idx + 1..]
                .iter()
                .position(|n| n.depth <= parent_depth)
                .map(|offset| parent_idx + 1 + offset)
                .unwrap_or(self.nodes.len())
        };
        self.nodes.drain(parent_idx + 1..end);

        // Insert new children (force their depth to child_depth for safety).
        for (i, mut child) in children.into_iter().enumerate() {
            child.depth = child_depth;
            self.nodes.insert(parent_idx + 1 + i, child);
        }

        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.visible = rebuild_visible(&self.nodes);
        ScrollState::clamp_active(&mut self.active_index, self.visible.len());
        self.scroll
            .ensure_visible(self.active_index, self.visible.len());
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

        // Move to parent: find nearest ancestor (depth - 1) going backwards in nodes[]
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

            let indent: String = "  ".repeat(node.depth);
            let indent_span = if focused && active {
                Span::new(format!(" {}", indent)).no_wrap()
            } else {
                Span::styled(format!(" {}", indent), inactive_style).no_wrap()
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
            let (icon_span, label_span) = if focused && active {
                let icon_st = if loading { loading_style } else { active_style };
                (
                    Span::styled(icon, icon_st).no_wrap(),
                    Span::styled(node.item.label(), active_style).no_wrap(),
                )
            } else {
                (
                    Span::styled(icon, inactive_style).no_wrap(),
                    Span::styled(node.item.label(), inactive_style).no_wrap(),
                )
            };

            lines.push(vec![cursor_span, indent_span, icon_span, label_span]);
        }

        if let Some(text) = self.scroll.footer(total) {
            lines.push(vec![
                Span::styled(text, Style::new().color(Color::DarkGrey)).no_wrap(),
            ]);
        }

        lines
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

        lines.extend(self.render_lines(focused));
        DrawOutput { lines }
    }
}

impl<T: TreeItemLabel> Interactive for TreeView<T> {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
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
}
