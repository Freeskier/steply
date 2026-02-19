mod async_utils;
mod cache;
mod model;
mod nav;
mod overlay;
mod parser;
mod scanner;
mod search;
mod tree_builder;
mod tree_scanner;

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
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};

use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::highlight::render_text_spans;
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::select_list::{SelectList, SelectMode};
use crate::widgets::components::tree_view::{TreeItemLabel, TreeNode, TreeView};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, HintContext, HintGroup, HintItem,
    InteractionResult, Interactive, RenderContext, TextEditState, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

use cache::{CacheKey, ScanCache};
use model::{EntryFilter as EF, completion_item_label, filter_entries, list_dir};
use parser::parse_input;
use scanner::{ScanRequest, ScannerHandle};
use search::ScanResult;
use tree_scanner::TreeScannerHandle;

const DEBOUNCE_MS: u64 = 120;
const SPINNER_INTERVAL_MS: u64 = 80;
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
    tree_scanner: TreeScannerHandle,
    cache: ScanCache,
    last_scan_result: Option<Arc<ScanResult>>,

    // Debounce
    debounce_deadline: Option<Instant>,

    // Inline browser state
    overlay_open: bool,
    browse_dir: PathBuf,

    // Spinner
    spinner_frame: usize,
    spinner_last_tick: Instant,
    scanning: bool,
    tree_building: bool,

    // Tree mode
    browser_mode: BrowserMode,
    tree: Option<TreeView<FileTreeItem>>,
    pending_tree_nodes: Option<(u64, Vec<TreeNode<FileTreeItem>>)>,
    tree_build_seq: u64,
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
            tree_scanner: TreeScannerHandle::new(),
            cache: ScanCache::new(),
            last_scan_result: None,
            debounce_deadline: None,
            overlay_open: false,
            spinner_frame: 0,
            spinner_last_tick: Instant::now(),
            scanning: false,
            tree_building: false,
            browser_mode: BrowserMode::List,
            tree: None,
            pending_tree_nodes: None,
            tree_build_seq: 0,
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
        // `**` in a glob pattern implies recursive traversal
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
            let parsed = parse_input(&self.current_input(), &self.cwd);
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

        // Provide immediate completion candidates without waiting for async scan.
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
        let parsed = parse_input(&self.current_input(), &self.cwd);
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
        if self.browser_mode == BrowserMode::Tree {
            self.ensure_tree_widget();
        }
        let recursive = parsed.mode.recursive(self.recursive, parsed.query.as_str());
        let is_glob = parsed.mode.is_glob();
        self.submit_scan(parsed.view_dir, parsed.query, is_glob, recursive);
        InteractionResult::handled()
    }

    fn close_browser(&mut self) -> InteractionResult {
        self.overlay_open = false;
        self.tree_building = false;
        self.pending_tree_nodes = None;
        InteractionResult::handled()
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
                    let hint = if self.scanning || self.tree_building {
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

                    let hint = if self.scanning || self.tree_building {
                        format!("  {} scanning…", self.spinner_char())
                    } else {
                        "  ← → dirs  Enter select  Ctrl+T tree  Esc close".to_string()
                    };
                    lines.push(vec![
                        Span::styled(hint, Style::new().color(Color::DarkGrey)).no_wrap(),
                    ]);
                }
            } else {
                let hint = if self.scanning || self.tree_building {
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

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        if !ctx.focused {
            return Vec::new();
        }

        let mut hints = vec![
            HintItem::new("Tab", "completion", HintGroup::Completion).with_priority(10),
            HintItem::new("Ctrl+Space", "toggle completion", HintGroup::Completion)
                .with_priority(11),
            HintItem::new("Shift+Space / Alt+Space", "open browser", HintGroup::View)
                .with_priority(20),
            HintItem::new("Enter", "select / submit", HintGroup::Action).with_priority(30),
        ];

        if self.overlay_open {
            hints.push(HintItem::new("Esc", "close browser", HintGroup::View).with_priority(21));
            hints.push(
                HintItem::new("← →", "navigate dirs", HintGroup::Navigation).with_priority(12),
            );
            if self.browser_mode == BrowserMode::Tree {
                hints.push(
                    HintItem::new("↑ ↓", "move in tree", HintGroup::Navigation).with_priority(13),
                );
                hints.push(
                    HintItem::new("Space", "expand/collapse", HintGroup::Navigation)
                        .with_priority(14),
                );
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
        self.handle_text_key_with_rescan(key)
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

fn should_skip_expensive_typing_scan(overlay_open: bool, recursive: bool, query: &str) -> bool {
    if overlay_open || !recursive || !query.contains("**") {
        return false;
    }
    let literal_chars = query.chars().filter(|ch| ch.is_alphanumeric()).count();
    literal_chars < 3
}
