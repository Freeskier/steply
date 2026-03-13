mod async_utils;
mod cache;
mod interaction;
mod model;
mod overlay_interaction;
mod parser;
mod query;
mod scanner;
mod tree_builder;
mod tree_scanner;

pub use model::EntryFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserMode {
    #[default]
    List,
    Tree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Single,
    Multi,
}

#[derive(Clone)]
struct FileTreeItem {
    entry: model::FileEntry,
    highlights: Vec<(usize, usize)>,
    leaf_count: usize,
    selected: bool,
}

impl FileTreeItem {
    fn new(entry: model::FileEntry, highlights: Vec<(usize, usize)>, selected: bool) -> Self {
        Self {
            entry,
            highlights,
            leaf_count: 0,
            selected,
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
        } else if state.selected || self.selected {
            Style::new().color(Color::Yellow).bold()
        } else if self.entry.kind.is_dir() {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    Full,

    #[default]
    Relative,

    Name,
}

use crate::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::value::Value;

use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::highlight::render_text_spans;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::ui::text::text_display_width;
use crate::widgets::base::WidgetBase;
use crate::widgets::components::select_list::{
    SelectList, SelectMode, default_render_option_lines,
};
use crate::widgets::components::tree_view::{TreeItemLabel, TreeNode, TreeView};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::LeafComponent;
use crate::widgets::shared::keymap;
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, RenderContext, TextEditState, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

use cache::{CacheKey, ScanCache};
use model::{EntryFilter as EF, completion_item_label, filter_entries, list_dir};
use parser::parse_input;
use query::ScanResult;
use scanner::{ScanRequest, ScannerHandle};
use tree_scanner::TreeScannerHandle;

const DEBOUNCE_MS: u64 = 120;
const SPINNER_INTERVAL_MS: u64 = 80;
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub struct FileBrowserComponent {
    base: WidgetBase,

    // Kept as a dedicated input intentionally: it is both the widget value owner
    // and completion prefix owner for path/query editing.
    text: TextInput,
    list: SelectList,

    cwd: PathBuf,
    recursive: bool,
    hide_hidden: bool,
    entry_filter: EF,
    ext_filter: Option<HashSet<String>>,
    display_mode: DisplayMode,
    value_mode: DisplayMode,
    selection_mode: SelectionMode,
    validators: Vec<Validator>,

    scanner: ScannerHandle,
    tree_scanner: TreeScannerHandle,
    cache: ScanCache,
    last_scan_result: Option<Arc<ScanResult>>,

    debounce_deadline: Option<Instant>,

    overlay_open: bool,
    browse_dir: PathBuf,

    spinner_frame: usize,
    spinner_last_tick: Instant,
    scanning: bool,
    tree_building: bool,

    browser_mode: BrowserMode,
    tree: Option<TreeView<FileTreeItem>>,
    list_overlay_items: Vec<overlay_interaction::ActiveOverlayItem>,
    pending_tree_nodes: Option<(u64, Vec<TreeNode<FileTreeItem>>)>,
    tree_build_seq: u64,
    pending_focus_restore: Option<FocusRestore>,
    focus_history: HashMap<PathBuf, FocusMemory>,
    selected_paths: Vec<PathBuf>,
    pending_selection_tokens: Option<Vec<String>>,
}

pub type FileBrowserInput = FileBrowserComponent;

const MULTI_VALUE_SEPARATOR: &str = ", ";

struct MultiInputState {
    active_query: String,
    query_start_chars: usize,
}

#[derive(Clone)]
pub(super) struct FocusMemory {
    pub index: usize,
    pub path: Option<PathBuf>,
}

#[derive(Clone)]
pub(super) enum FocusRestore {
    History(FocusMemory),
    FirstRealEntry,
}

impl FileBrowserComponent {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let label = label.into();
        let cwd = crate::host::cwd();

        let text = TextInput::new(format!("{id}__text"), label.clone())
            .with_placeholder("Type a path or pattern (Tab for completion)");
        let list = SelectList::from_strings(format!("{id}__list"), "", vec![])
            .with_mode(SelectMode::List)
            .with_show_label(false)
            .with_max_visible(12);

        let mut widget = Self {
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
            value_mode: DisplayMode::Relative,
            selection_mode: SelectionMode::Single,
            validators: Vec::new(),
            scanner: ScannerHandle::new(),
            tree_scanner: TreeScannerHandle::new(),
            cache: ScanCache::new(),
            last_scan_result: None,
            debounce_deadline: None,
            overlay_open: true,
            spinner_frame: 0,
            spinner_last_tick: Instant::now(),
            scanning: false,
            tree_building: false,
            browser_mode: BrowserMode::List,
            tree: None,
            list_overlay_items: Vec::new(),
            pending_tree_nodes: None,
            tree_build_seq: 0,
            pending_focus_restore: None,
            focus_history: HashMap::new(),
            selected_paths: Vec::new(),
            pending_selection_tokens: None,
        };
        widget.list.set_option_renderer(|item, mut state| {
            if state.selected && !(state.focused && state.active) {
                state.base_style = Style::new().color(Color::Yellow).bold();
            }
            default_render_option_lines(item, state)
        });
        widget
    }

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

    pub fn with_value_mode(mut self, mode: DisplayMode) -> Self {
        self.value_mode = mode;
        self
    }

    pub fn with_selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    pub fn with_max_visible(mut self, n: usize) -> Self {
        self.list.set_max_visible(n);
        self
    }

    pub fn with_browser_mode(mut self, mode: BrowserMode) -> Self {
        self.browser_mode = mode;
        if self.browser_mode == BrowserMode::Tree {
            self.ensure_tree_widget();
        }
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    fn current_input(&self) -> String {
        self.text
            .value()
            .and_then(|v| v.to_text_scalar())
            .unwrap_or_default()
    }

    fn query_input(&self) -> String {
        if !self.is_multi_select() {
            return self.current_input();
        }
        split_multi_input(self.current_input().as_str()).active_query
    }

    fn query_start_char_index(&self) -> usize {
        if !self.is_multi_select() {
            return 0;
        }
        split_multi_input(self.current_input().as_str()).query_start_chars
    }

    fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()]
    }

    fn ensure_tree_widget(&mut self) {
        if self.tree.is_none() {
            self.tree = Some(
                TreeView::new(format!("{}_tree", self.base.id()), "", vec![])
                    .with_show_label(false)
                    .with_indent_guides(true)
                    .with_max_visible(12),
            );
        }
    }

    fn handle_text_key_with_rescan(&mut self, key: KeyEvent) -> InteractionResult {
        let prev = self.current_input();
        let result = self.text.on_key(key);
        if self.current_input() != prev {
            self.schedule_scan();
        }
        result
    }

    fn make_key(&self, dir: &Path, query: &str, recursive: bool) -> CacheKey {
        CacheKey {
            dir: dir.to_path_buf(),
            query: query.to_string(),
            recursive,
            hide_hidden: self.hide_hidden,
            entry_filter: self.entry_filter,
        }
    }

    fn submit_scan(&mut self, dir: PathBuf, query: String, is_glob: bool, recursive: bool) {
        let recursive = recursive || (is_glob && query.contains("**"));
        if should_skip_expensive_typing_scan(self.overlay_open, recursive, query.as_str()) {
            self.scanning = false;
            return;
        }
        let key = self.make_key(&dir, &query, recursive);
        if let Some(result) = self.cache.get(&key).cloned() {
            self.apply_result(result);
            return;
        }
        if self.cache.is_in_flight(&key) {
            return;
        }
        self.scanning = true;
        self.spinner_last_tick = Instant::now();
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

    fn poll_scanner(&mut self) -> bool {
        let results = self.scanner.try_recv_all();
        if results.is_empty() {
            return false;
        }
        let current_key = {
            let parsed = parse_input(&self.query_input(), &self.cwd);
            self.make_key(
                &parsed.view_dir,
                &parsed.query,
                parsed.mode.recursive(self.recursive, parsed.query.as_str()),
            )
        };
        let browse_key = self.make_key(&self.browse_dir.clone(), "", false);
        let mut changed = false;
        for (key, result) in results {
            self.cache.insert(key.clone(), Arc::clone(&result));
            if key == current_key || (self.overlay_open && key == browse_key) {
                self.apply_result(result);
                changed = true;
            }
        }
        changed
    }

    fn sync_completion_items_for_dir(&mut self, dir: &Path) {
        let key = self.make_key(dir, "", false);
        if let Some(result) = self.cache.get(&key) {
            self.text
                .set_completion_items(result.completion_items.clone());
            return;
        }

        let items = filter_entries(
            list_dir(dir, self.hide_hidden),
            self.entry_filter,
            self.ext_filter.as_ref(),
        )
        .into_iter()
        .map(|entry| completion_item_label(&entry))
        .collect::<Vec<_>>();
        self.text.set_completion_items(items);

        self.submit_scan(dir.to_path_buf(), String::new(), false, false);
    }

    fn flush_debounce(&mut self) -> bool {
        let Some(deadline) = self.debounce_deadline else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.debounce_deadline = None;
        let parsed = parse_input(&self.query_input(), &self.cwd);
        self.browse_dir = parsed.view_dir.clone();
        let recursive = parsed.mode.recursive(self.recursive, parsed.query.as_str());
        let is_glob = parsed.mode.is_glob();
        self.submit_scan(parsed.view_dir, parsed.query, is_glob, recursive);
        true
    }

    fn schedule_scan(&mut self) {
        self.debounce_deadline = Some(Instant::now() + Duration::from_millis(DEBOUNCE_MS));
    }

    fn browse_into(&mut self, dir: PathBuf) {
        self.browse_into_with_restore(dir, None);
    }

    fn browse_into_with_restore(&mut self, dir: PathBuf, fallback: Option<FocusRestore>) {
        self.debounce_deadline = None;
        self.overlay_open = true;
        self.pending_focus_restore = self
            .focus_history
            .get(&dir)
            .cloned()
            .map(FocusRestore::History)
            .or(fallback)
            .or(Some(FocusRestore::FirstRealEntry));
        self.browse_dir = dir.clone();

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
        if self.is_multi_select() {
            self.set_active_query(path_str);
        } else {
            self.text.set_value(Value::Text(path_str));
        }
        self.submit_scan(dir, String::new(), false, false);
    }

    fn open_browser(&mut self) -> InteractionResult {
        self.debounce_deadline = None;
        self.overlay_open = true;
        let parsed = parse_input(&self.query_input(), &self.cwd);
        self.browse_dir = parsed.view_dir.clone();
        self.pending_focus_restore = self
            .focus_history
            .get(&parsed.view_dir)
            .cloned()
            .map(FocusRestore::History)
            .or(Some(FocusRestore::FirstRealEntry));
        self.sync_completion_items_for_dir(parsed.view_dir.as_path());
        if self.browser_mode == BrowserMode::Tree {
            self.ensure_tree_widget();
        }
        let recursive = parsed.mode.recursive(self.recursive, parsed.query.as_str());
        let is_glob = parsed.mode.is_glob();
        self.submit_scan(parsed.view_dir, parsed.query, is_glob, recursive);
        InteractionResult::handled()
    }

    pub fn initialize_open(&mut self) {
        let _ = self.open_browser();
    }

    fn close_browser(&mut self) -> InteractionResult {
        self.overlay_open = false;
        self.tree_building = false;
        self.pending_tree_nodes = None;
        InteractionResult::handled()
    }

    fn child_ctx(&self, ctx: &RenderContext, focused_id: Option<String>) -> RenderContext {
        ctx.with_focus(focused_id)
            .with_completion_owner(self.base.id(), Some(self.text.id()))
    }

    fn is_multi_select(&self) -> bool {
        self.selection_mode == SelectionMode::Multi
    }

    fn selected_output_values(&self) -> Vec<Value> {
        self.selected_paths
            .iter()
            .map(|path| Value::Text(self.path_value_for_submit(path.as_path())))
            .collect()
    }

    fn is_selected_path(&self, path: &Path) -> bool {
        self.selected_paths.iter().any(|selected| selected == path)
    }

    fn set_selected_path(&mut self, path: PathBuf, selected: bool) {
        if selected {
            if !self.selected_paths.iter().any(|existing| existing == &path) {
                self.selected_paths.push(path);
            }
        } else {
            self.selected_paths.retain(|existing| existing != &path);
        }
    }

    fn toggle_selected_path(&mut self, path: PathBuf) -> bool {
        let was_selected = self.is_selected_path(path.as_path());
        self.set_selected_path(path, !was_selected);
        !was_selected
    }

    fn sync_list_selection(&mut self) {
        let values = Value::List(
            self.selected_paths
                .iter()
                .map(|path| Value::Text(path.to_string_lossy().to_string()))
                .collect(),
        );
        self.list.set_value(values);
    }

    fn sync_tree_selection(&mut self) {
        let selected = self.selected_paths.clone();
        if let Some(tree) = self.tree.as_mut() {
            for node in tree.nodes_mut() {
                node.item.selected = selected
                    .iter()
                    .any(|path| path == node.item.entry.path.as_ref());
            }
        }
    }

    fn expanded_tree_paths(&self) -> HashSet<PathBuf> {
        self.tree
            .as_ref()
            .map(|tree| {
                tree.nodes()
                    .iter()
                    .filter(|node| {
                        node.has_children && node.expanded && node.item.entry.name != ".."
                    })
                    .map(|node| (*node.item.entry.path).clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn expanded_tree_subtrees(&self) -> HashMap<PathBuf, Vec<TreeNode<FileTreeItem>>> {
        let Some(tree) = self.tree.as_ref() else {
            return HashMap::new();
        };

        let nodes = tree.nodes();
        let mut out = HashMap::new();
        for (index, node) in nodes.iter().enumerate() {
            if !(node.has_children
                && node.expanded
                && node.children_loaded
                && node.item.entry.name != "..")
            {
                continue;
            }

            let end = nodes[index + 1..]
                .iter()
                .position(|child| child.depth <= node.depth)
                .map(|offset| index + 1 + offset)
                .unwrap_or(nodes.len());

            if end > index + 1 {
                out.insert(
                    (*node.item.entry.path).clone(),
                    nodes[index + 1..end].to_vec(),
                );
            }
        }
        out
    }

    fn resolve_tokens_against_result(
        &self,
        tokens: &[String],
        result: Option<&ScanResult>,
    ) -> Vec<PathBuf> {
        match self.value_mode {
            DisplayMode::Full => tokens.iter().map(PathBuf::from).collect(),
            DisplayMode::Relative => tokens.iter().map(|token| self.cwd.join(token)).collect(),
            DisplayMode::Name => {
                let Some(result) = result else {
                    return Vec::new();
                };
                let mut selected = Vec::new();
                for token in tokens {
                    for entry in &result.entries {
                        if entry.name == *token
                            && !selected.iter().any(|path| path == entry.path.as_ref())
                        {
                            selected.push((*entry.path).clone());
                        }
                    }
                }
                selected
            }
        }
    }

    fn set_selected_paths_from_tokens(&mut self, tokens: Vec<String>) {
        let resolved =
            self.resolve_tokens_against_result(tokens.as_slice(), self.last_scan_result.as_deref());
        self.pending_selection_tokens =
            if self.value_mode == DisplayMode::Name && resolved.is_empty() && !tokens.is_empty() {
                Some(tokens)
            } else {
                None
            };
        self.selected_paths = resolved;
        self.sync_list_selection();
        self.sync_tree_selection();
        self.sync_multi_input_text(true);
    }

    fn sync_multi_input_text(&mut self, preserve_query: bool) {
        if !self.is_multi_select() {
            return;
        }
        let active_query = if preserve_query {
            split_multi_input(self.current_input().as_str()).active_query
        } else {
            String::new()
        };
        let mut text = self
            .selected_output_values()
            .into_iter()
            .filter_map(|value| value.to_text_scalar())
            .collect::<Vec<_>>()
            .join(MULTI_VALUE_SEPARATOR);
        if !text.is_empty() {
            if !active_query.is_empty() {
                text.push_str(MULTI_VALUE_SEPARATOR);
                text.push_str(active_query.as_str());
            } else {
                text.push_str(MULTI_VALUE_SEPARATOR);
            }
        } else if !active_query.is_empty() {
            text = active_query;
        }
        self.text.set_value(Value::Text(text));
    }

    fn set_active_query(&mut self, active_query: String) {
        if !self.is_multi_select() {
            self.text.set_value(Value::Text(active_query));
            return;
        }

        let mut text = self
            .selected_output_values()
            .into_iter()
            .filter_map(|value| value.to_text_scalar())
            .collect::<Vec<_>>()
            .join(MULTI_VALUE_SEPARATOR);
        if !text.is_empty() {
            text.push_str(MULTI_VALUE_SEPARATOR);
        }
        text.push_str(active_query.as_str());
        self.text.set_value(Value::Text(text));
    }
}

impl LeafComponent for FileBrowserComponent {}

impl Drawable for FileBrowserComponent {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = ctx
            .focused_id
            .as_deref()
            .is_some_and(|id| id == self.base.id());

        let text_ctx = self.child_ctx(
            ctx,
            if focused {
                Some(self.text.id().to_string())
            } else {
                None
            },
        );
        let mut lines = self.text.draw(&text_ctx).lines;
        if self.overlay_open && (self.scanning || self.tree_building) {
            let status = format!("{} scanning...", self.spinner_char());
            if let Some(input_line) = lines.first_mut() {
                append_right_status(input_line, status.as_str(), ctx.terminal_size.width);
            }
        }

        if self.overlay_open {
            if self.browser_mode == BrowserMode::Tree {
                if let Some(tree) = &self.tree {
                    lines.extend(tree.render_lines(true));
                }
            } else {
                let list_id = self.list.id().to_string();
                let list_ctx = self.child_ctx(ctx, Some(list_id));
                lines.extend(self.list.draw(&list_ctx).lines);

                if let Some(result) = &self.last_scan_result {
                    let shown = result.entries.len();
                    let total = result.total_matches;
                    if total > shown {
                        lines.push(vec![
                            Span::styled(
                                format!("  … {} more (refine query to narrow down)", total - shown),
                                Style::new().color(Color::DarkGrey),
                            )
                            .no_wrap(),
                        ]);
                    }
                }
            }
        }

        DrawOutput::with_lines(lines)
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }

        let mut hints = crate::widgets::traits::focused_static_hints(
            ctx,
            crate::widgets::static_hints::FILE_BROWSER_DOC_HINTS,
        );
        hints.retain(|hint| {
            hint.key == "Tab"
                || hint.key == "Ctrl+Space"
                || hint.key == "Shift+Space / Alt+Space"
                || hint.key == "Enter"
        });

        if self.overlay_open {
            hints.push(HintItem::new("Esc", "close browser", HintGroup::View).with_priority(21));
            hints.push(
                HintItem::new("← →", "navigate dirs", HintGroup::Navigation).with_priority(12),
            );
            if self.is_multi_select() {
                hints.push(
                    HintItem::new("Enter", "accept selection", HintGroup::Action).with_priority(24),
                );
                hints.push(
                    HintItem::new("Space", "toggle file", HintGroup::Action).with_priority(25),
                );
            }
            if self.browser_mode == BrowserMode::Tree {
                hints.push(
                    HintItem::new("↑ ↓", "move in tree", HintGroup::Navigation).with_priority(13),
                );
                if self.is_multi_select() {
                    hints.push(
                        HintItem::new("Ctrl+Space", "toggle dir", HintGroup::Action)
                            .with_priority(26),
                    );
                    hints.push(
                        HintItem::new("Space", "expand/collapse", HintGroup::Navigation)
                            .with_priority(14),
                    );
                } else {
                    hints.push(
                        HintItem::new("Space", "expand/collapse", HintGroup::Navigation)
                            .with_priority(14),
                    );
                }
                hints.push(
                    HintItem::new("Ctrl+T", "switch to list", HintGroup::View).with_priority(22),
                );
            } else {
                hints.push(
                    HintItem::new("↑ ↓", "move in list", HintGroup::Navigation).with_priority(13),
                );
                hints.push(
                    HintItem::new("Ctrl+T", "switch to tree", HintGroup::View).with_priority(22),
                );
            }
        }

        hints
    }
}

impl Interactive for FileBrowserComponent {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn store_sync_policy(&self) -> crate::widgets::traits::StoreSyncPolicy {
        crate::widgets::traits::StoreSyncPolicy::PreserveLocalStateWhileFocused
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        if key.code == KeyCode::Char(' ')
            && (keymap::has_exact_modifiers(key, KeyModifiers::SHIFT)
                || keymap::has_exact_modifiers(key, KeyModifiers::ALT))
        {
            return self.open_browser();
        }

        if self.overlay_open {
            return self.handle_browser_key(key);
        }

        if key.code == KeyCode::Enter {
            return InteractionResult::input_done();
        }

        self.handle_text_key_with_rescan(key)
    }

    fn text_editing(&mut self) -> Option<TextEditState<'_>> {
        self.text.text_editing()
    }

    fn on_text_edited(&mut self) {
        self.schedule_scan();
    }

    fn completion(&mut self) -> Option<CompletionState<'_>> {
        let parsed = parse_input(&self.query_input(), &self.cwd);
        self.sync_completion_items_for_dir(parsed.view_dir.as_path());
        let query_start = self.query_start_char_index();

        let mut state = self.text.completion()?;

        let chars: Vec<char> = state.value.chars().collect();
        let pos = (*state.cursor).min(chars.len());
        let byte_end = state
            .value
            .char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(state.value.len());
        let query_start_byte = state
            .value
            .char_indices()
            .nth(query_start)
            .map(|(i, _)| i)
            .unwrap_or(state.value.len());
        let start = state.value[query_start_byte..byte_end]
            .rfind('/')
            .map(|relative| {
                query_start
                    + state.value[query_start_byte..query_start_byte + relative + 1]
                        .chars()
                        .count()
            })
            .unwrap_or(query_start);
        state.prefix_start = Some(start);
        Some(state)
    }

    fn on_tick(&mut self) -> InteractionResult {
        let mut spinner_advanced = false;
        if (self.scanning || self.tree_building) && self.overlay_open {
            let now = Instant::now();
            if now.duration_since(self.spinner_last_tick)
                >= Duration::from_millis(SPINNER_INTERVAL_MS)
            {
                self.spinner_last_tick = now;
                self.spinner_frame = self.spinner_frame.wrapping_add(1);
                spinner_advanced = true;
            }
        }
        let scanner_changed = self.poll_scanner();
        let tree_changed = self.poll_tree_build_results();
        let debounce_fired = self.flush_debounce();
        if scanner_changed || tree_changed || debounce_fired || spinner_advanced {
            InteractionResult::handled()
        } else {
            InteractionResult::ignored()
        }
    }

    fn value(&self) -> Option<Value> {
        if self.is_multi_select() {
            return Some(Value::List(self.selected_output_values()));
        }
        Some(Value::Text(self.current_input()))
    }

    fn set_value(&mut self, value: Value) {
        if self.is_multi_select() {
            if let Some(items) = value.to_text_list() {
                self.set_selected_paths_from_tokens(items);
                return;
            }
            if let Some(text) = value.to_text_scalar() {
                self.set_selected_paths_from_tokens(vec![text]);
                return;
            }
        }
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

fn should_skip_expensive_typing_scan(overlay_open: bool, recursive: bool, query: &str) -> bool {
    if overlay_open || !recursive || !query.contains("**") {
        return false;
    }
    let literal_chars = query.chars().filter(|ch| ch.is_alphanumeric()).count();
    literal_chars < 3
}

fn append_right_status(line: &mut Vec<Span>, status: &str, terminal_width: u16) {
    let available = terminal_width as usize;
    if available == 0 {
        return;
    }

    let status_width = text_display_width(status);
    let used = line
        .iter()
        .map(|span| text_display_width(span.text.as_str()))
        .sum::<usize>();

    let gap = available.saturating_sub(used.saturating_add(status_width));
    if gap == 0 {
        return;
    }

    line.push(Span::new(" ".repeat(gap)).no_wrap());
    line.push(Span::styled(status.to_string(), Style::new().color(Color::DarkGrey)).no_wrap());
}

fn split_multi_input(raw: &str) -> MultiInputState {
    let Some((prefix, tail)) = raw.rsplit_once(',') else {
        return MultiInputState {
            active_query: raw.to_string(),
            query_start_chars: 0,
        };
    };

    let _ = prefix;
    let active_query = tail.trim_start().to_string();
    let query_start_chars = raw.chars().count() - tail.chars().count()
        + tail.chars().take_while(|ch| ch.is_whitespace()).count();

    MultiInputState {
        active_query,
        query_start_chars,
    }
}
