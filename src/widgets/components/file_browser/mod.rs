mod cache;
mod model;
mod parser;
mod scanner;
mod search;

pub use model::EntryFilter;

/// Controls the inline browser rendering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserMode {
    #[default]
    List,
    Tree,
}

// ── FileTreeItem ──────────────────────────────────────────────────────────────

struct FileTreeItem {
    entry: model::FileEntry,
    highlights: Vec<(usize, usize)>,
    leaf_count: usize,
}

impl FileTreeItem {
    fn new(entry: model::FileEntry, highlights: Vec<(usize, usize)>) -> Self {
        Self {
            entry,
            highlights,
            leaf_count: 0,
        }
    }
}

impl TreeItemLabel for FileTreeItem {
    fn label(&self) -> &str {
        &self.entry.name
    }

    fn render_spans(
        &self,
        state: crate::widgets::components::tree_view::TreeItemRenderState,
    ) -> Vec<Span> {
        let base_style = if state.focused && state.active {
            Style::new().color(Color::Cyan).bold()
        } else if state.has_children {
            Style::new().color(Color::Blue).bold()
        } else {
            Style::default()
        };
        let highlight_style = Style::new().color(Color::Yellow).bold();
        let inactive_style = Style::new().color(Color::DarkGrey);
        let link_style = Style::new().color(Color::Green);

        let mut spans = render_text_spans(
            self.label(),
            self.highlights.as_slice(),
            base_style,
            highlight_style,
        );
        if self.entry.kind.is_symlink() {
            spans.push(Span::styled("@", link_style).no_wrap());
        }
        if self.leaf_count > 0 {
            spans.push(Span::styled(format!(" [{}]", self.leaf_count), inactive_style).no_wrap());
        }
        spans
    }
}

/// Controls how file paths are displayed in the inline list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    /// Show the full absolute path (e.g. `/home/user/projects/src/main.rs`)
    Full,
    /// Show path relative to cwd (e.g. `src/main.rs`) — default
    #[default]
    Relative,
    /// Show only the file/folder name (e.g. `main.rs`)
    Name,
}

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};

use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::highlight::render_text_spans;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::select_list::{SelectItem, SelectItemView, SelectList, SelectMode};
use crate::widgets::components::tree_view::{TreeItemLabel, TreeNode, TreeView};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult, Interactive,
    RenderContext, TextEditState, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

use cache::{CacheKey, ScanCache};
use model::{EntryFilter as EF, filter_entries, list_dir};
use parser::parse_input;
use scanner::{ScanRequest, ScannerHandle};
use search::ScanResult;

const DEBOUNCE_MS: u64 = 50;
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// A file-browser input: text field with path completion + Shift/Alt+Space inline list.
pub struct FileBrowserInput {
    base: WidgetBase,

    // Sub-widgets
    text: TextInput,
    list: SelectList,

    // Config
    cwd: PathBuf,
    recursive: bool,
    hide_hidden: bool,
    entry_filter: EF,
    ext_filter: Option<HashSet<String>>,
    display_mode: DisplayMode,
    submit_target: Option<ValueTarget>,
    validators: Vec<Validator>,

    // Async scan
    scanner: ScannerHandle,
    cache: ScanCache,
    last_scan_result: Option<ScanResult>,

    // Debounce
    debounce_deadline: Option<Instant>,

    // Inline browser state
    overlay_open: bool,
    browse_dir: PathBuf,

    // Spinner
    spinner_frame: usize,
    scanning: bool,

    // Tree mode
    browser_mode: BrowserMode,
    tree: Option<TreeView<FileTreeItem>>,
    prefer_first_real_entry: bool,
    preferred_entry_path: Option<PathBuf>,
}

impl FileBrowserInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let label = label.into();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        let text = TextInput::new(format!("{id}__text"), label.clone())
            .with_placeholder("Type a path or pattern (Tab for completion)");
        let list = SelectList::from_strings(format!("{id}__list"), "", vec![])
            .with_mode(SelectMode::List)
            .with_show_label(false)
            .with_max_visible(12);

        Self {
            base: WidgetBase::new(id, label),
            text,
            list,
            browse_dir: cwd.clone(),
            cwd,
            recursive: false,
            hide_hidden: true,
            entry_filter: EF::All,
            ext_filter: None,
            display_mode: DisplayMode::Relative,
            submit_target: None,
            validators: Vec::new(),
            scanner: ScannerHandle::new(),
            cache: ScanCache::new(),
            last_scan_result: None,
            debounce_deadline: None,
            overlay_open: false,
            spinner_frame: 0,
            scanning: false,
            browser_mode: BrowserMode::List,
            tree: None,
            prefer_first_real_entry: false,
            preferred_entry_path: None,
        }
    }

    // ── Builder ──────────────────────────────────────────────────────────────

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        let p = cwd.into();
        self.browse_dir = p.clone();
        self.cwd = p;
        self
    }

    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    pub fn with_hide_hidden(mut self, hide: bool) -> Self {
        self.hide_hidden = hide;
        self
    }

    pub fn with_entry_filter(mut self, filter: EF) -> Self {
        self.entry_filter = filter;
        self
    }

    pub fn with_ext_filter(mut self, exts: &[&str]) -> Self {
        self.ext_filter = Some(
            exts.iter()
                .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
                .collect(),
        );
        self
    }

    pub fn with_display_mode(mut self, mode: DisplayMode) -> Self {
        self.display_mode = mode;
        self
    }

    pub fn with_max_visible(mut self, n: usize) -> Self {
        self.list.set_max_visible(n);
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

    pub fn with_browser_mode(mut self, mode: BrowserMode) -> Self {
        self.browser_mode = mode;
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn current_input(&self) -> String {
        self.text
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_default()
    }

    fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()]
    }

    fn make_key(&self, dir: &Path, query: &str) -> CacheKey {
        // `**` in a glob pattern implies recursive even if self.recursive is false
        let recursive = self.recursive || query.contains("**");
        CacheKey {
            dir: dir.to_path_buf(),
            query: query.to_string(),
            recursive,
            hide_hidden: self.hide_hidden,
            entry_filter: self.entry_filter,
        }
    }

    fn submit_scan(&mut self, dir: PathBuf, query: String, is_glob: bool, recursive: bool) {
        // `**` in a glob pattern implies recursive traversal
        let recursive = recursive || (is_glob && query.contains("**"));
        let key = self.make_key(&dir, &query);
        if let Some(result) = self.cache.get(&key).cloned() {
            self.apply_result(result);
            return;
        }
        if self.cache.is_in_flight(&key) {
            return;
        }
        self.scanning = true;
        self.cache.mark_in_flight(key.clone());
        self.scanner.submit(ScanRequest {
            key,
            dir,
            query,
            recursive,
            hide_hidden: self.hide_hidden,
            entry_filter: self.entry_filter,
            ext_filter: self.ext_filter.clone(),
            is_glob,
            display_mode: self.display_mode,
        });
    }

    fn apply_result(&mut self, result: ScanResult) {
        self.scanning = false;
        self.text
            .set_completion_items(result.completion_items.clone());

        let options = if self.overlay_open && self.browse_dir.parent().is_some() {
            let mut opts = Vec::with_capacity(result.options.len() + 1);
            opts.push(
                SelectItem::new(
                    Value::Text("..".to_string()),
                    SelectItemView::Styled {
                        text: "..".to_string(),
                        highlights: vec![],
                        style: crate::ui::style::Style::new()
                            .color(crate::ui::style::Color::DarkGrey),
                    },
                )
                .with_search_text(".."),
            );
            opts.extend(result.options.clone());
            opts
        } else {
            result.options.clone()
        };
        let has_dotdot_option = options
            .first()
            .and_then(|item| item.value.to_text_scalar())
            .is_some_and(|v| v == "..");
        self.list.set_options(options);
        let mut list_active_set = false;
        if let Some(pref_path) = self.preferred_entry_path.as_ref()
            && let Some(pos) = result
                .entries
                .iter()
                .position(|entry| entry.path.as_ref() == pref_path)
        {
            let offset = if has_dotdot_option { 1 } else { 0 };
            self.list.set_active_index(pos + offset);
            list_active_set = true;
        }
        let prefer_first_real_entry = self.prefer_first_real_entry && has_dotdot_option;
        if !list_active_set && prefer_first_real_entry {
            self.list.set_active_index(1);
        }

        let tree_nodes = self.build_tree_nodes(&result);
        let has_dotdot_tree = tree_nodes
            .first()
            .is_some_and(|node| node.item.entry.name == "..");
        if let Some(tree) = self.tree.as_mut() {
            tree.set_nodes(tree_nodes);
            let mut tree_active_set = false;
            if let Some(pref_path) = self.preferred_entry_path.as_ref()
                && let Some(pos) = tree
                    .visible()
                    .iter()
                    .position(|&idx| tree.nodes()[idx].item.entry.path.as_ref() == pref_path)
            {
                tree.set_active_visible_index(pos);
                tree_active_set = true;
            }
            if !tree_active_set && self.prefer_first_real_entry && has_dotdot_tree {
                tree.set_active_visible_index(1);
            }
        }
        if self.prefer_first_real_entry {
            self.prefer_first_real_entry = false;
        }
        self.preferred_entry_path = None;

        self.last_scan_result = Some(result);
    }

    fn poll_scanner(&mut self) -> bool {
        let results = self.scanner.try_recv_all();
        if results.is_empty() {
            return false;
        }
        let current_key = {
            let parsed = parse_input(&self.current_input(), &self.cwd);
            self.make_key(&parsed.view_dir, &parsed.query)
        };
        let browse_key = self.make_key(&self.browse_dir.clone(), "");
        let mut changed = false;
        for (key, result) in results {
            self.cache.insert(key.clone(), result.clone());
            if key == current_key || (self.overlay_open && key == browse_key) {
                self.apply_result(result);
                changed = true;
            }
        }
        changed
    }

    fn sync_completion_items_for_dir(&mut self, dir: &Path) {
        let key = self.make_key(dir, "");
        if let Some(result) = self.cache.get(&key) {
            self.text
                .set_completion_items(result.completion_items.clone());
            return;
        }

        // Provide immediate completion candidates without waiting for async scan.
        let items = filter_entries(
            list_dir(dir, self.hide_hidden),
            self.entry_filter,
            self.ext_filter.as_ref(),
        )
        .into_iter()
        .map(|entry| {
            if entry.kind.is_dir() {
                format!("{}/", entry.name)
            } else {
                entry.name
            }
        })
        .collect::<Vec<_>>();
        self.text.set_completion_items(items);

        self.submit_scan(dir.to_path_buf(), String::new(), false, false);
    }

    fn build_tree_nodes(&self, result: &ScanResult) -> Vec<TreeNode<FileTreeItem>> {
        let mut nodes = Vec::<TreeNode<FileTreeItem>>::new();

        if self.overlay_open
            && let Some(parent) = self.browse_dir.parent()
        {
            use std::sync::Arc;
            let dotdot_entry = model::FileEntry {
                name: "..".to_string(),
                name_lower: "..".to_string(),
                ext_lower: None,
                path: Arc::new(parent.to_path_buf()),
                kind: model::EntryKind::Dir,
            };
            nodes.push(TreeNode::new(
                FileTreeItem::new(dotdot_entry, Vec::new()),
                0,
                false,
            ));
        }

        let mut inserted_dirs = std::collections::HashSet::<PathBuf>::new();
        for (entry_idx, entry) in result.entries.iter().enumerate() {
            let highlights = result
                .highlights
                .get(entry_idx)
                .cloned()
                .unwrap_or_default();
            let rel = match entry.path.strip_prefix(&self.browse_dir) {
                Ok(path) => path.to_path_buf(),
                Err(_) => {
                    nodes.push(TreeNode::new(
                        FileTreeItem::new(entry.clone(), highlights),
                        0,
                        entry.kind.is_dir(),
                    ));
                    continue;
                }
            };

            let components: Vec<_> = rel.components().collect();
            for anc_depth in 0..components.len().saturating_sub(1) {
                let anc_rel: PathBuf = components[..=anc_depth].iter().collect();
                let anc_abs = self.browse_dir.join(&anc_rel);
                if inserted_dirs.insert(anc_abs.clone()) {
                    use std::sync::Arc;
                    let name = components[anc_depth]
                        .as_os_str()
                        .to_string_lossy()
                        .to_string();
                    let dir_entry = model::FileEntry {
                        name: name.clone(),
                        name_lower: name.to_ascii_lowercase(),
                        ext_lower: None,
                        path: Arc::new(anc_abs),
                        kind: model::EntryKind::Dir,
                    };
                    let mut node =
                        TreeNode::new(FileTreeItem::new(dir_entry, Vec::new()), anc_depth, true);
                    // Keep synthetic folders open so filtered matches are visible immediately.
                    node.expanded = true;
                    node.children_loaded = true;
                    nodes.push(node);
                }
            }

            let depth = components.len().saturating_sub(1);
            nodes.push(TreeNode::new(
                FileTreeItem::new(entry.clone(), highlights),
                depth,
                entry.kind.is_dir(),
            ));
        }

        let dotdot = if nodes
            .first()
            .map(|n| n.item.entry.name == "..")
            .unwrap_or(false)
        {
            Some(nodes.remove(0))
        } else {
            None
        };
        nodes.sort_by(|a, b| {
            fn sort_key(entry: &model::FileEntry) -> String {
                let mut s = entry.path.to_string_lossy().to_ascii_lowercase();
                if entry.kind.is_dir() {
                    s.push('/');
                }
                s
            }
            sort_key(&a.item.entry).cmp(&sort_key(&b.item.entry))
        });
        if let Some(dd) = dotdot {
            nodes.insert(0, dd);
        }

        for i in 0..nodes.len() {
            if !nodes[i].has_children {
                continue;
            }
            let parent_depth = nodes[i].depth;
            let count = nodes[i + 1..]
                .iter()
                .take_while(|n| n.depth > parent_depth)
                .filter(|n| !n.has_children)
                .count();
            nodes[i].item.leaf_count = count;
        }

        nodes
    }

    fn flush_debounce(&mut self) -> bool {
        let Some(deadline) = self.debounce_deadline else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.debounce_deadline = None;
        let parsed = parse_input(&self.current_input(), &self.cwd);
        self.browse_dir = parsed.view_dir.clone();
        self.submit_scan(
            parsed.view_dir,
            parsed.query,
            parsed.is_glob,
            self.recursive,
        );
        true
    }

    fn schedule_scan(&mut self) {
        self.debounce_deadline = Some(Instant::now() + Duration::from_millis(DEBOUNCE_MS));
    }

    fn browse_into(&mut self, dir: PathBuf) {
        self.debounce_deadline = None;
        self.overlay_open = true;
        self.prefer_first_real_entry = self.preferred_entry_path.is_none();
        self.browse_dir = dir.clone();
        // Update text input to show the new directory path (relative to cwd if possible)
        let path_str = if let Ok(rel) = dir.strip_prefix(&self.cwd) {
            let s = rel.to_string_lossy();
            if s.is_empty() {
                String::new()
            } else {
                format!("{}/", s)
            }
        } else {
            let abs = dir.to_string_lossy();
            if abs == "/" {
                "/".to_string()
            } else if abs.ends_with('/') {
                abs.to_string()
            } else {
                format!("{abs}/")
            }
        };
        self.text.set_value(Value::Text(path_str));
        self.submit_scan(dir, String::new(), false, false);
    }

    fn open_browser(&mut self) -> InteractionResult {
        self.debounce_deadline = None;
        self.overlay_open = true;
        self.prefer_first_real_entry = true;
        let parsed = parse_input(&self.current_input(), &self.cwd);
        self.browse_dir = parsed.view_dir.clone();
        self.sync_completion_items_for_dir(parsed.view_dir.as_path());
        if self.browser_mode == BrowserMode::Tree && self.tree.is_none() {
            self.tree = Some(
                TreeView::new(format!("{}_tree", self.base.id()), "", vec![])
                    .with_show_label(false)
                    .with_indent_guides(true)
                    .with_max_visible(12),
            );
        }
        self.submit_scan(
            parsed.view_dir,
            parsed.query,
            parsed.is_glob,
            self.recursive,
        );
        InteractionResult::handled()
    }

    fn close_browser(&mut self) -> InteractionResult {
        self.overlay_open = false;
        InteractionResult::handled()
    }

    /// Returns true when `..` is prepended to the list (browser open, not at root).
    fn has_dotdot(&self) -> bool {
        self.overlay_open && self.browse_dir.parent().is_some()
    }

    /// Returns the `FileEntry` at the current active list index, if any.
    /// Returns `None` when `..` is selected (handled separately as parent nav).
    fn active_entry(&self) -> Option<&model::FileEntry> {
        let idx = self.list.active_index();
        let entries = &self.last_scan_result.as_ref()?.entries;
        let offset = if self.has_dotdot() { 1 } else { 0 };
        idx.checked_sub(offset).and_then(|i| entries.get(i))
    }

    fn active_tree_entry(&self) -> Option<(&model::FileEntry, bool)> {
        let node = self.tree.as_ref()?.active_node()?;
        let is_dotdot = node.item.entry.name == "..";
        Some((&node.item.entry, is_dotdot))
    }

    fn path_value_for_submit(&self, path: &Path) -> String {
        if let Ok(rel) = path.strip_prefix(&self.cwd) {
            rel.to_string_lossy().to_string()
        } else {
            path.to_string_lossy().to_string()
        }
    }

    fn handle_browser_key(&mut self, key: KeyEvent) -> InteractionResult {
        // Ctrl+T — toggle List ↔ Tree
        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.browser_mode = match self.browser_mode {
                BrowserMode::List => BrowserMode::Tree,
                BrowserMode::Tree => BrowserMode::List,
            };
            if self.browser_mode == BrowserMode::Tree && self.tree.is_none() {
                self.tree = Some(
                    TreeView::new(format!("{}_tree", self.base.id()), "", vec![])
                        .with_show_label(false)
                        .with_indent_guides(true)
                        .with_max_visible(12),
                );
                // Rebuild tree from existing scan result
                if let Some(result) = self.last_scan_result.clone() {
                    self.apply_result(result);
                }
            }
            return InteractionResult::handled();
        }

        // In tree mode, ↑/↓/→/←/Enter go to TreeView
        if self.browser_mode == BrowserMode::Tree {
            return self.handle_tree_key(key);
        }

        match key.code {
            KeyCode::Esc => {
                let parsed = parse_input(&self.current_input(), &self.cwd);
                if !parsed.query.trim().is_empty() {
                    self.browse_into(self.browse_dir.clone());
                    return InteractionResult::handled();
                }
                self.close_browser()
            }

            KeyCode::Enter => {
                // `..` selected → go to parent
                if self.has_dotdot() && self.list.active_index() == 0 {
                    if let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) {
                        self.preferred_entry_path = Some(self.browse_dir.clone());
                        self.browse_into(parent);
                    }
                    return InteractionResult::handled();
                }
                let entry = self.active_entry().cloned();
                let Some(entry) = entry else {
                    return InteractionResult::handled();
                };
                if entry.kind.is_dir() {
                    self.browse_into((*entry.path).clone());
                } else {
                    self.text
                        .set_value(Value::Text(self.path_value_for_submit(entry.path.as_ref())));
                    return self.close_browser();
                }
                InteractionResult::handled()
            }

            KeyCode::Right => {
                // Ignore on `..`
                if self.has_dotdot() && self.list.active_index() == 0 {
                    return InteractionResult::handled();
                }
                let entry = self.active_entry().cloned();
                if let Some(entry) = entry
                    && entry.kind.is_dir()
                {
                    self.browse_into((*entry.path).clone());
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }

            KeyCode::Left => {
                if let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) {
                    self.preferred_entry_path = Some(self.browse_dir.clone());
                    self.browse_into(parent);
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }

            KeyCode::Up | KeyCode::Down => self.list.on_key(key),

            // All other keys (chars, backspace, delete, left/right cursor) → text input
            _ => {
                let prev = self.current_input();
                let result = self.text.on_key(key);
                if self.current_input() != prev {
                    self.schedule_scan();
                }
                result
            }
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Esc => {
                let parsed = parse_input(&self.current_input(), &self.cwd);
                if !parsed.query.trim().is_empty() {
                    self.browse_into(self.browse_dir.clone());
                    return InteractionResult::handled();
                }
                self.close_browser()
            }

            KeyCode::Up => {
                if self
                    .tree
                    .as_mut()
                    .map(|t| t.move_active(-1))
                    .unwrap_or(false)
                {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Down => {
                if self
                    .tree
                    .as_mut()
                    .map(|t| t.move_active(1))
                    .unwrap_or(false)
                {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }

            KeyCode::Right => {
                let Some((entry, is_dotdot)) = self.active_tree_entry() else {
                    return InteractionResult::handled();
                };
                if is_dotdot {
                    return InteractionResult::handled();
                }
                if entry.kind.is_dir() {
                    let dir = (*entry.path).clone();
                    self.browse_into(dir);
                }
                InteractionResult::handled()
            }

            KeyCode::Enter => {
                let Some((entry, is_dotdot)) = self.active_tree_entry() else {
                    return InteractionResult::handled();
                };

                if is_dotdot || entry.kind.is_dir() {
                    let dir = (*entry.path).clone();
                    self.browse_into(dir);
                    return InteractionResult::handled();
                }

                self.text
                    .set_value(Value::Text(self.path_value_for_submit(entry.path.as_ref())));
                return self.close_browser();
            }

            KeyCode::Char(' ') => {
                let is_dotdot = self
                    .active_tree_entry()
                    .is_some_and(|(_, is_dotdot)| is_dotdot);
                if is_dotdot {
                    if let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) {
                        self.preferred_entry_path = Some(self.browse_dir.clone());
                        self.browse_into(parent);
                    }
                    return InteractionResult::handled();
                }

                let active = self.tree.as_ref().and_then(|tree| {
                    let node = tree.active_node()?;
                    let idx = tree.active_node_idx()?;
                    Some((
                        idx,
                        node.has_children,
                        node.children_loaded,
                        node.expanded,
                        node.item.entry.path.clone(),
                    ))
                });
                let Some((node_idx, has_children, children_loaded, expanded, path)) = active else {
                    return InteractionResult::handled();
                };

                if !has_children {
                    return InteractionResult::handled();
                }

                if expanded {
                    if let Some(tree) = self.tree.as_mut() {
                        tree.collapse_active();
                    }
                    return InteractionResult::handled();
                }

                if !children_loaded {
                    let child_entries = filter_entries(
                        list_dir(path.as_ref(), self.hide_hidden),
                        self.entry_filter,
                        self.ext_filter.as_ref(),
                    );
                    let children = child_entries
                        .into_iter()
                        .map(|entry| {
                            let is_dir = entry.kind.is_dir();
                            TreeNode::new(FileTreeItem::new(entry, Vec::new()), 0, is_dir)
                        })
                        .collect::<Vec<_>>();
                    if let Some(tree) = self.tree.as_mut() {
                        tree.insert_children_after(node_idx, children);
                    }
                } else if let Some(tree) = self.tree.as_mut() {
                    tree.expand_active();
                }
                InteractionResult::handled()
            }

            KeyCode::Left => {
                if let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) {
                    self.preferred_entry_path = Some(self.browse_dir.clone());
                    self.browse_into(parent);
                }
                InteractionResult::handled()
            }

            _ => {
                let prev = self.current_input();
                let result = self.text.on_key(key);
                if self.current_input() != prev {
                    self.schedule_scan();
                }
                result
            }
        }
    }

    fn child_ctx(&self, ctx: &RenderContext, focused_id: Option<String>) -> RenderContext {
        // Remap completion menu keyed under our own id to the inner text widget's id,
        // so TextInput::draw() finds it when rendering ghost text.
        let mut completion_menus = ctx.completion_menus.clone();
        if let Some(menu) = completion_menus.remove(self.base.id()) {
            completion_menus.insert(self.text.id().to_string(), menu);
        }
        RenderContext {
            focused_id,
            terminal_size: ctx.terminal_size,
            visible_errors: ctx.visible_errors.clone(),
            invalid_hidden: ctx.invalid_hidden.clone(),
            completion_menus,
        }
    }
}

// ── Component ────────────────────────────────────────────────────────────────

impl Component for FileBrowserInput {
    fn children(&self) -> &[Node] {
        &[]
    }
    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

// ── Drawable ─────────────────────────────────────────────────────────────────

impl Drawable for FileBrowserInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = ctx
            .focused_id
            .as_deref()
            .is_some_and(|id| id == self.base.id());

        // Always pass focus through to inner TextInput so completion ghost text renders
        let text_ctx = self.child_ctx(
            ctx,
            if focused {
                Some(self.text.id().to_string())
            } else {
                None
            },
        );
        let mut lines = self.text.draw(&text_ctx).lines;

        if focused {
            if self.overlay_open {
                if self.browser_mode == BrowserMode::Tree {
                    if let Some(tree) = &self.tree {
                        lines.extend(tree.render_lines(true));
                    }
                    let hint = if self.scanning {
                        format!("  {} scanning…", self.spinner_char())
                    } else {
                        "  ↑↓ nav  Space expand/collapse  ← → dirs  Enter select  Ctrl+T list  Esc close".to_string()
                    };
                    lines.push(vec![
                        Span::styled(hint, Style::new().color(Color::DarkGrey)).no_wrap(),
                    ]);
                } else {
                    // Inline list — pass list's own ID as focused so the ❯ cursor renders
                    let list_id = self.list.id().to_string();
                    let list_ctx = self.child_ctx(ctx, Some(list_id));
                    lines.extend(self.list.draw(&list_ctx).lines);

                    // Show truncation notice if results were cut off
                    if let Some(result) = &self.last_scan_result {
                        let shown = result.entries.len();
                        let total = result.total_matches;
                        if total > shown {
                            lines.push(vec![
                                Span::styled(
                                    format!(
                                        "  … {} more (refine query to narrow down)",
                                        total - shown
                                    ),
                                    Style::new().color(Color::DarkGrey),
                                )
                                .no_wrap(),
                            ]);
                        }
                    }

                    let hint = if self.scanning {
                        format!("  {} scanning…", self.spinner_char())
                    } else {
                        "  ← → dirs  Enter select  Ctrl+T tree  Esc close".to_string()
                    };
                    lines.push(vec![
                        Span::styled(hint, Style::new().color(Color::DarkGrey)).no_wrap(),
                    ]);
                }
            } else {
                let hint = if self.scanning {
                    format!("  {} scanning…", self.spinner_char())
                } else {
                    "  Shift+Space (or Alt+Space) to browse".to_string()
                };
                lines.push(vec![
                    Span::styled(hint, Style::new().color(Color::DarkGrey)).no_wrap(),
                ]);
            }
        }

        DrawOutput { lines }
    }
}

// ── Interactive ───────────────────────────────────────────────────────────────

impl Interactive for FileBrowserInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        // Shift+Space (or Alt+Space) → open inline browser
        if key.code == KeyCode::Char(' ')
            && (key.modifiers == KeyModifiers::SHIFT || key.modifiers == KeyModifiers::ALT)
        {
            return self.open_browser();
        }

        // Browser open: route to browser key handler
        if self.overlay_open {
            return self.handle_browser_key(key);
        }

        // Enter → submit
        if key.code == KeyCode::Enter {
            return InteractionResult::submit_or_produce(
                self.submit_target.as_ref(),
                Value::Text(self.current_input()),
            );
        }

        // Normal text input
        let prev = self.current_input();
        let result = self.text.on_key(key);
        if self.current_input() != prev {
            self.schedule_scan();
        }
        result
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        // Delegate so Ctrl+W / Alt+D work on the inner text field
        self.text.text_editing()
    }

    fn on_text_edited(&mut self) {
        // Called after Ctrl+W / Alt+D mutates the inner text — trigger a scan
        self.schedule_scan();
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        // Completion candidates are always loaded from the same scan/cache
        // pipeline as query filtering; only the presentation differs by mode.
        let parsed = parse_input(&self.current_input(), &self.cwd);
        self.sync_completion_items_for_dir(parsed.view_dir.as_path());

        let mut state = self.text.completion()?;
        // Path-aware completion: token starts after the last '/'
        let chars: Vec<char> = state.value.chars().collect();
        let pos = (*state.cursor).min(chars.len());
        let byte_end = state
            .value
            .char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(state.value.len());
        let start = state.value[..byte_end]
            .rfind('/')
            .map(|i| state.value[..=i].chars().count())
            .unwrap_or(0);
        state.prefix_start = Some(start);
        Some(state)
    }

    fn on_tick(&mut self) -> InteractionResult {
        if self.scanning {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
        let scanner_changed = self.poll_scanner();
        let debounce_fired = self.flush_debounce();
        if scanner_changed || debounce_fired || self.scanning {
            InteractionResult::handled()
        } else {
            InteractionResult::ignored()
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.current_input()))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.to_text_scalar() {
            self.text.set_value(Value::Text(text));
            self.schedule_scan();
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, &Value::Text(self.current_input()))
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        self.text.cursor_pos()
    }
}
