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

struct FileTreeItem(model::FileEntry);

impl TreeItemLabel for FileTreeItem {
    fn label(&self) -> &str {
        &self.0.name
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

use crate::core::value::Value;

use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::components::select_list::{SelectList, SelectMode, SelectOption};
use crate::widgets::components::tree_view::{TreeItemLabel, TreeNode, TreeView};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    CompletionState, DrawOutput, Drawable, FocusMode, InteractionResult, Interactive,
    RenderContext, TextEditState, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

use cache::{CacheKey, ScanCache};
use model::EntryFilter as EF;
use parser::parse_input;
use scanner::{ScanRequest, ScannerHandle};
use search::ScanResult;

const DEBOUNCE_MS: u64 = 150;
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// A file-browser input: text field with path completion + Ctrl+Space inline list.
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
    submit_target: Option<String>,
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
}

impl FileBrowserInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let label = label.into();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        let text = TextInput::new(format!("{id}__text"), label.clone());
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

    pub fn with_submit_target(mut self, target: impl Into<String>) -> Self {
        self.submit_target = Some(target.into());
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

        // When browser is open and not at root, prepend `..` as a navigation option
        let options = if self.overlay_open && self.browse_dir.parent().is_some() {
            let mut opts = Vec::with_capacity(result.options.len() + 1);
            opts.push(SelectOption::Styled {
                text: "..".to_string(),
                highlights: vec![],
                style: crate::ui::style::Style::new().color(crate::ui::style::Color::DarkGrey),
            });
            opts.extend(result.options.clone());
            opts
        } else {
            result.options.clone()
        };

        self.list.set_options(options);

        // In tree mode, also rebuild the TreeView from the same entries
        if self.browser_mode == BrowserMode::Tree
            && let Some(tree) = self.tree.as_mut()
        {
            let mut nodes: Vec<TreeNode<FileTreeItem>> = Vec::new();

            // Prepend `..` when not at root
            if self.overlay_open && self.browse_dir.parent().is_some() {
                use std::sync::Arc;
                let dotdot_entry = model::FileEntry {
                    name: "..".to_string(),
                    name_lower: "..".to_string(),
                    ext_lower: None,
                    path: Arc::new(self.browse_dir.parent().unwrap().to_path_buf()),
                    is_dir: true,
                };
                nodes.push(TreeNode::new(FileTreeItem(dotdot_entry), 0, false));
            }

            // Track which ancestor dirs have already been inserted as synthetic nodes
            // key = absolute path of dir
            let mut inserted_dirs: std::collections::HashSet<PathBuf> =
                std::collections::HashSet::new();

            for e in &result.entries {
                let rel = match e.path.strip_prefix(&self.browse_dir) {
                    Ok(r) => r.to_path_buf(),
                    Err(_) => {
                        nodes.push(TreeNode::new(FileTreeItem(e.clone()), 0, e.is_dir));
                        continue;
                    }
                };

                // Insert synthetic dir nodes for each ancestor not yet present
                let components: Vec<_> = rel.components().collect();
                for anc_depth in 0..components.len().saturating_sub(1) {
                    let anc_rel: PathBuf = components[..=anc_depth].iter().collect();
                    let anc_abs = self.browse_dir.join(&anc_rel);
                    if inserted_dirs.insert(anc_abs.clone()) {
                        let name = components[anc_depth]
                            .as_os_str()
                            .to_string_lossy()
                            .to_string();
                        use std::sync::Arc;
                        let dir_entry = model::FileEntry {
                            name: name.clone(),
                            name_lower: name.to_ascii_lowercase(),
                            ext_lower: None,
                            path: Arc::new(anc_abs),
                            is_dir: true,
                        };
                        let mut node = TreeNode::new(FileTreeItem(dir_entry), anc_depth, true);
                        node.expanded = true;
                        node.children_loaded = true;
                        nodes.push(node);
                    }
                }

                let depth = components.len().saturating_sub(1);
                nodes.push(TreeNode::new(FileTreeItem(e.clone()), depth, e.is_dir));
            }

            // Sort by absolute path so parents always precede their children
            // (e.g. "legacy/" < "legacy/src/" < "legacy/src/mod.rs").
            // Keep `..` pinned at the front.
            let dotdot = if nodes
                .first()
                .map(|n| n.item.0.name == "..")
                .unwrap_or(false)
            {
                Some(nodes.remove(0))
            } else {
                None
            };
            nodes.sort_by(|a, b| a.item.0.path.cmp(&b.item.0.path));
            if let Some(dd) = dotdot {
                nodes.insert(0, dd);
            }

            tree.set_nodes(nodes);
        }

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

    fn flush_debounce(&mut self) -> bool {
        let Some(deadline) = self.debounce_deadline else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.debounce_deadline = None;
        let parsed = parse_input(&self.current_input(), &self.cwd);
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
        self.browse_dir = dir.clone();
        self.list.set_options(vec![]);
        // Update text input to show the new directory path (relative to cwd if possible)
        let path_str = if let Ok(rel) = dir.strip_prefix(&self.cwd) {
            let s = rel.to_string_lossy();
            if s.is_empty() {
                String::new()
            } else {
                format!("{}/", s)
            }
        } else {
            format!("{}/", dir.to_string_lossy())
        };
        self.text.set_value(Value::Text(path_str));
        self.submit_scan(dir, String::new(), false, false);
    }

    fn open_browser(&mut self) -> InteractionResult {
        self.overlay_open = true;
        let parsed = parse_input(&self.current_input(), &self.cwd);
        self.browse_dir = parsed.view_dir.clone();
        if self.browser_mode == BrowserMode::Tree && self.tree.is_none() {
            self.tree = Some(
                TreeView::new(format!("{}_tree", self.base.id()), "", vec![])
                    .with_show_label(false)
                    .with_max_visible(12),
            );
        }
        self.submit_scan(parsed.view_dir, String::new(), false, false);
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
            KeyCode::Esc => self.close_browser(),

            KeyCode::Enter => {
                // `..` selected → go to parent
                if self.has_dotdot() && self.list.active_index() == 0 {
                    if let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) {
                        self.browse_into(parent);
                    }
                    return InteractionResult::handled();
                }
                let entry = self.active_entry().cloned();
                let Some(entry) = entry else {
                    return InteractionResult::handled();
                };
                if entry.is_dir {
                    self.browse_into((*entry.path).clone());
                } else {
                    self.text
                        .set_value(Value::Text(entry.path.to_string_lossy().to_string()));
                    return self.close_browser();
                }
                InteractionResult::handled()
            }

            KeyCode::Right => {
                // `..` selected → go to parent
                if self.has_dotdot()
                    && self.list.active_index() == 0
                    && let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf)
                {
                    self.browse_into(parent);
                    return InteractionResult::handled();
                }
                let entry = self.active_entry().cloned();
                if let Some(entry) = entry
                    && entry.is_dir
                {
                    self.browse_into((*entry.path).clone());
                    return InteractionResult::handled();
                }
                InteractionResult::ignored()
            }

            KeyCode::Left => {
                if let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) {
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
            KeyCode::Esc => self.close_browser(),

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

            KeyCode::Right | KeyCode::Enter => {
                let active = self.tree.as_ref().and_then(|t| t.active_node()).map(|n| {
                    let is_dotdot = n.item.0.name == "..";
                    (
                        is_dotdot,
                        n.item.0.is_dir,
                        n.item.0.path.clone(),
                        n.has_children,
                    )
                });
                let Some((is_dotdot, is_dir, path, has_children)) = active else {
                    return InteractionResult::handled();
                };

                if is_dotdot
                    || (key.code == KeyCode::Right && is_dir)
                    || (key.code == KeyCode::Enter && is_dir)
                {
                    let dir = (*path).clone();
                    self.browse_into(dir);
                    // Reset tree for new directory
                    if let Some(tree) = self.tree.as_mut() {
                        tree.set_nodes(vec![]);
                    }
                    return InteractionResult::handled();
                }

                if key.code == KeyCode::Enter && !is_dir {
                    let path_str = if let Ok(rel) = path.strip_prefix(&self.cwd) {
                        rel.to_string_lossy().to_string()
                    } else {
                        path.to_string_lossy().to_string()
                    };
                    self.text.set_value(Value::Text(path_str));
                    return self.close_browser();
                }

                // Dir node without Enter: expand/collapse
                if has_children && let Some(tree) = self.tree.as_mut() {
                    tree.expand_active();
                }
                InteractionResult::handled()
            }

            KeyCode::Char(' ') => {
                if let Some(tree) = self.tree.as_mut()
                    && !tree.expand_active()
                {
                    tree.collapse_active();
                }
                InteractionResult::handled()
            }

            KeyCode::Left => {
                if let Some(tree) = self.tree.as_mut()
                    && tree.collapse_active()
                {
                    return InteractionResult::handled();
                }
                // Go to parent directory
                if let Some(parent) = self.browse_dir.parent().map(Path::to_path_buf) {
                    self.browse_into(parent);
                    if let Some(tree) = self.tree.as_mut() {
                        tree.set_nodes(vec![]);
                    }
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
                        "  ↑↓ nav  → expand  ← collapse  Enter select  Ctrl+T list  Esc close"
                            .to_string()
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
                    "  Ctrl+Space to browse".to_string()
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
        // Ctrl+Space → open inline browser
        if key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.open_browser();
        }

        // Browser open: route to browser key handler
        if self.overlay_open {
            return self.handle_browser_key(key);
        }

        // Enter → submit
        if key.code == KeyCode::Enter {
            return InteractionResult::submit_or_produce(
                self.submit_target.as_deref(),
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
        // Delegate so Tab-completion uses inner TextInput's value + completion_items
        self.text.completion()
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
