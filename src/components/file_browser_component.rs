use crate::components::select_component::{SelectComponent, SelectMode, SelectOption};
use crate::core::component::{Component, ComponentBase, EventContext, FocusMode};
use crate::core::search::fuzzy;
use crate::core::value::Value;
use crate::inputs::Input;
use crate::inputs::text_input::TextInput;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, SystemTime};

pub struct FileBrowserComponent {
    base: ComponentBase,
    input: TextInput,
    select: SelectComponent,
    current_dir: PathBuf,
    view_dir: PathBuf,
    entries: Vec<FileEntry>,
    matches: Vec<fuzzy::FuzzyMatch>,
    recursive_search: bool,
    hide_hidden: bool,
    show_relative_paths: bool,
    show_info: bool,
    entry_filter: EntryFilter,
    extension_filter: Option<HashSet<String>>,
    cache: HashMap<String, SearchResult>,
    dir_cache: HashMap<String, Vec<FileEntry>>,
    in_flight: HashSet<String>,
    last_applied_key: Option<String>,
    spinner_index: usize,
    spinner_tick: u8,
    scan_tx: Sender<(String, SearchResult)>,
    scan_rx: Receiver<(String, SearchResult)>,
}

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    name_lower: String,
    ext_lower: Option<String>,
    path: PathBuf,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct SearchResult {
    entries: Vec<FileEntry>,
    options: Vec<SelectOption>,
    matches: Vec<fuzzy::FuzzyMatch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SearchMode {
    Fuzzy,
    Glob,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryFilter {
    All,
    FilesOnly,
    DirsOnly,
}

struct NewEntry {
    path: PathBuf,
    label: String,
    is_dir: bool,
}

impl FileBrowserComponent {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let input = TextInput::new(format!("{}_filter", id), "Path");
        let select =
            SelectComponent::new(format!("{}_list", id), Vec::new()).with_mode(SelectMode::List);
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let view_dir = current_dir.clone();
        let (scan_tx, scan_rx) = mpsc::channel();
        let mut component = Self {
            base: ComponentBase::new(id),
            input,
            select,
            current_dir,
            view_dir,
            entries: Vec::new(),
            matches: Vec::new(),
            recursive_search: true,
            hide_hidden: true,
            show_relative_paths: false,
            show_info: false,
            entry_filter: EntryFilter::All,
            extension_filter: None,
            cache: HashMap::new(),
            dir_cache: HashMap::new(),
            in_flight: HashSet::new(),
            last_applied_key: None,
            spinner_index: 0,
            spinner_tick: 0,
            scan_tx,
            scan_rx,
        };
        component.refresh_view();
        component
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.input.base_mut_ref().label = label.into();
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input = self.input.with_placeholder(placeholder);
        self
    }

    pub fn with_max_visible(mut self, max_visible: usize) -> Self {
        self.select.set_max_visible(max_visible);
        self
    }

    pub fn set_max_visible(&mut self, max_visible: usize) {
        self.select.set_max_visible(max_visible);
    }

    pub fn with_recursive_search(mut self, recursive: bool) -> Self {
        self.recursive_search = recursive;
        self.refresh_view();
        self
    }

    pub fn with_entry_filter(mut self, filter: EntryFilter) -> Self {
        self.entry_filter = filter;
        self.refresh_view();
        self
    }

    pub fn set_entry_filter(&mut self, filter: EntryFilter) {
        self.entry_filter = filter;
        self.refresh_view();
    }

    fn toggle_entry_filter(&mut self, filter: EntryFilter) {
        if self.entry_filter == filter {
            self.entry_filter = EntryFilter::All;
        } else {
            self.entry_filter = filter;
        }
        self.refresh_view();
    }

    pub fn with_extension_filter<I, S>(mut self, exts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.set_extension_filter(exts);
        self
    }

    pub fn set_extension_filter<I, S>(&mut self, exts: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let normalized = exts
            .into_iter()
            .map(|ext| normalize_ext(ext.as_ref()))
            .filter(|ext| !ext.is_empty())
            .collect::<HashSet<_>>();
        if normalized.is_empty() {
            self.extension_filter = None;
        } else {
            self.extension_filter = Some(normalized);
        }
        self.refresh_view();
    }

    pub fn clear_extension_filter(&mut self) {
        self.extension_filter = None;
        self.refresh_view();
    }

    fn filter_entries(&self, entries: Vec<FileEntry>) -> Vec<FileEntry> {
        filter_entries(entries, self.entry_filter, self.extension_filter.as_ref())
    }

    pub fn with_relative_paths(mut self, show_relative: bool) -> Self {
        self.show_relative_paths = show_relative;
        self.refresh_view();
        self
    }

    pub fn set_relative_paths(&mut self, show_relative: bool) {
        self.show_relative_paths = show_relative;
        self.refresh_view();
    }

    pub fn with_show_hidden(mut self, show_hidden: bool) -> Self {
        self.hide_hidden = !show_hidden;
        self.refresh_view();
        self
    }

    pub fn set_show_hidden(&mut self, show_hidden: bool) {
        self.hide_hidden = !show_hidden;
        self.refresh_view();
    }

    pub fn set_current_dir(&mut self, dir: impl Into<PathBuf>) {
        self.current_dir = dir.into();
        self.refresh_view();
    }

    fn refresh_view(&mut self) {
        self.poll_scans();
        let raw = self.input.value();
        let normalized = normalize_input(&raw, &self.current_dir);
        if normalized != raw {
            self.input.set_value(normalized.clone());
        }
        let parsed = parse_input(&normalized, &self.current_dir);

        self.view_dir = parsed.view_dir.clone();

        let (entries, options, matches) = if parsed.path_mode {
            let raw = normalized.trim();
            if let Some(query) = strip_recursive_fuzzy_segment(&parsed.segment) {
                if let Some(result) = self.search_async(
                    &parsed.view_dir,
                    true,
                    &query,
                    &parsed.view_dir,
                    SearchMode::Fuzzy,
                ) {
                    (result.entries, result.options, result.matches)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else if is_glob_query(raw) {
                if let Some((base_dir, pattern)) = split_glob_path(normalized.trim()) {
                    let base_path = resolve_path(&base_dir, &self.current_dir);
                    let recursive = is_recursive_glob(&pattern);
                    if recursive {
                        if let Some(result) = self.search_async(
                            &base_path,
                            true,
                            &pattern,
                            &base_path,
                            SearchMode::Glob,
                        ) {
                            (result.entries, result.options, result.matches)
                        } else {
                            (Vec::new(), Vec::new(), Vec::new())
                        }
                    } else {
                        let entries = self.filter_entries(list_dir(&base_path, self.hide_hidden));
                        let (entries, options) = glob_options(
                            &entries,
                            &pattern,
                            Some(&base_path),
                            self.show_relative_paths,
                            self.show_info,
                        );
                        (entries, options, Vec::new())
                    }
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else {
                let entries = self.filter_entries(list_dir(&parsed.view_dir, self.hide_hidden));
                if parsed.segment.is_empty() {
                    let options = entries
                        .iter()
                        .map(|entry| {
                            entry_option(
                                entry,
                                &[],
                                Some(&parsed.view_dir),
                                self.show_relative_paths,
                                self.show_info,
                            )
                        })
                        .collect::<Vec<_>>();
                    (entries, options, Vec::new())
                } else {
                    let (entries, options, matches) = options_from_query(
                        &entries,
                        &parsed.segment,
                        Some(&parsed.view_dir),
                        self.show_relative_paths,
                        self.show_info,
                    );
                    (entries, options, matches)
                }
            }
        } else if normalized.trim().is_empty() {
            let entries = self.filter_entries(list_dir(&self.current_dir, self.hide_hidden));
            let options = entries
                .iter()
                .map(|entry| {
                    entry_option(
                        entry,
                        &[],
                        Some(&self.current_dir),
                        self.show_relative_paths,
                        self.show_info,
                    )
                })
                .collect::<Vec<_>>();
            (entries, options, Vec::new())
        } else {
            let raw = normalized.trim();
            if let Some(query) = strip_recursive_fuzzy(raw) {
                let current_dir = self.current_dir.clone();
                let display_root = self.current_dir.clone();
                if let Some(result) = self.search_async(
                    &current_dir,
                    true,
                    &query,
                    &display_root,
                    SearchMode::Fuzzy,
                ) {
                    (result.entries, result.options, result.matches)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else if is_glob_query(raw) {
                let recursive = is_recursive_glob(raw);
                if recursive {
                    let current_dir = self.current_dir.clone();
                    let display_root = self.current_dir.clone();
                    if let Some(result) = self.search_async(
                        &current_dir,
                        true,
                        raw,
                        &display_root,
                        SearchMode::Glob,
                    ) {
                        (result.entries, result.options, result.matches)
                    } else {
                        (Vec::new(), Vec::new(), Vec::new())
                    }
                } else {
                    let entries = self.filter_entries(list_dir(&self.current_dir, self.hide_hidden));
                    let (entries, options) = glob_options(
                        &entries,
                        raw,
                        Some(&self.current_dir),
                        self.show_relative_paths,
                        self.show_info,
                    );
                    (entries, options, Vec::new())
                }
            } else if self.recursive_search {
                let query = raw.to_string();
                let current_dir = self.current_dir.clone();
                let display_root = self.current_dir.clone();
                if let Some(result) =
                    self.search_async(&current_dir, true, &query, &display_root, SearchMode::Fuzzy)
                {
                    (result.entries, result.options, result.matches)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else {
                let entries = self.filter_entries(list_dir(&self.current_dir, self.hide_hidden));
                let (entries, options, matches) = options_from_query(
                    &entries,
                    raw,
                    Some(&self.current_dir),
                    self.show_relative_paths,
                    self.show_info,
                );
                (entries, options, matches)
            }
        };

        self.entries = entries;
        self.matches = matches;
        self.select.set_options(options);
        self.select.reset_active();
    }

    fn poll_scans(&mut self) -> bool {
        let mut updated = false;
        let current_key = self.current_search_key();
        let mut to_apply: Option<String> = None;
        for (key, result) in self.scan_rx.try_iter() {
            self.in_flight.remove(&key);
            let is_current = current_key.as_deref() == Some(&key);
            self.cache.insert(key.clone(), result);
            if is_current {
                to_apply = Some(key);
                updated = true;
            }
        }
        if let Some(key) = to_apply {
            self.last_applied_key = Some(key.clone());
            if let Some(result) = self.cache.get(&key).cloned() {
                self.apply_search_result(&result);
            }
        }
        updated
    }

    fn search_async(
        &mut self,
        dir: &Path,
        recursive: bool,
        query: &str,
        display_root: &Path,
        mode: SearchMode,
    ) -> Option<SearchResult> {
        let key = cache_key(
            dir,
            recursive,
            self.hide_hidden,
            query,
            self.show_relative_paths,
            self.show_info,
            mode,
            self.entry_filter,
            self.extension_filter.as_ref(),
        );
        if let Some(result) = self.cache.get(&key) {
            self.last_applied_key = Some(key);
            return Some(result.clone());
        }

        let dir_key = dir_cache_key(dir, recursive, self.hide_hidden);
        if let Some(entries) = self.dir_cache.get(&dir_key) {
            let filtered = filter_entries(
                entries.clone(),
                self.entry_filter,
                self.extension_filter.as_ref(),
            );
            if mode == SearchMode::Glob {
                if self.in_flight.contains(&key) {
                    return None;
                }
                self.in_flight.insert(key.clone());
                let entries = filtered.clone();
                let query = query.to_string();
                let display_root = display_root.to_path_buf();
                let show_relative = self.show_relative_paths;
                let show_info = self.show_info;
                let tx = self.scan_tx.clone();
                thread::spawn(move || {
                    let (entries, options) = glob_options(
                        &entries,
                        &query,
                        Some(display_root.as_path()),
                        show_relative,
                        show_info,
                    );
                    let result = SearchResult {
                        entries,
                        options,
                        matches: Vec::new(),
                    };
                    let _ = tx.send((key, result));
                });
                return None;
            }

            let (entries, options, matches) = options_from_query(
                &filtered,
                query,
                Some(display_root),
                self.show_relative_paths,
                self.show_info,
            );
            let result = SearchResult {
                entries,
                options,
                matches,
            };
            self.cache.insert(key.clone(), result.clone());
            self.last_applied_key = Some(key);
            return Some(result);
        }

        if self.in_flight.contains(&key) {
            return None;
        }

        self.in_flight.insert(key.clone());

        let dir = dir.to_path_buf();
        let hide_hidden = self.hide_hidden;
        let query = query.to_string();
        let display_root = display_root.to_path_buf();
        let show_relative = self.show_relative_paths;
        let show_info = self.show_info;
        let mode = mode;
        let entry_filter = self.entry_filter;
        let ext_filter = self.extension_filter.clone();
        let tx = self.scan_tx.clone();
        thread::spawn(move || {
            let result = if mode == SearchMode::Glob {
                let mut entries = if recursive {
                    list_dir_recursive_glob(&dir, hide_hidden, &query)
                } else {
                    list_dir(&dir, hide_hidden)
                };
                entries = filter_entries(entries, entry_filter, ext_filter.as_ref());
                let normalized = query.replace('\\', "/");
                let segments = split_segments(&normalized);
                let name_pattern = segments
                    .last()
                    .cloned()
                    .unwrap_or_else(String::new);
                let options = build_glob_options(
                    &mut entries,
                    &name_pattern,
                    Some(display_root.as_path()),
                    show_relative,
                    show_info,
                );
                SearchResult {
                    entries,
                    options,
                    matches: Vec::new(),
                }
            } else {
                let entries = if recursive {
                    list_dir_recursive(&dir, hide_hidden)
                } else {
                    list_dir(&dir, hide_hidden)
                };
                let entries = filter_entries(entries, entry_filter, ext_filter.as_ref());
                let (entries, options, matches) = options_from_query(
                    &entries,
                    &query,
                    Some(display_root.as_path()),
                    show_relative,
                    show_info,
                );
                SearchResult {
                    entries,
                    options,
                    matches,
                }
            };
            let _ = tx.send((key, result));
        });

        None
    }

    fn apply_search_result(&mut self, result: &SearchResult) {
        self.entries = result.entries.clone();
        self.matches = result.matches.clone();
        self.select.set_options(result.options.clone());
        self.select.reset_active();
    }

    fn is_searching_current(&self) -> bool {
        let Some(key) = self.current_search_key() else {
            return false;
        };
        self.in_flight.contains(&key)
    }

    fn spinner_frame(&self) -> &'static str {
        const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        FRAMES[self.spinner_index % FRAMES.len()]
    }

    fn new_entry_candidate(&self) -> Option<NewEntry> {
        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.current_dir);
        if !parsed.path_mode {
            return None;
        }
        if parsed.segment.is_empty() {
            return None;
        }
        if parsed.segment == "~" || parsed.segment.starts_with("~/") {
            return None;
        }
        if parsed.segment.contains('*') || parsed.segment.contains('?') {
            return None;
        }
        if parsed.segment.starts_with("**") {
            return None;
        }

        let base = resolve_path(&parsed.dir_prefix, &self.current_dir);
        let candidate = base.join(&parsed.segment);
        if candidate.exists() {
            return None;
        }

        let is_dir = !parsed.segment.contains('.');
        match self.entry_filter {
            EntryFilter::FilesOnly if is_dir => return None,
            EntryFilter::DirsOnly if !is_dir => return None,
            _ => {}
        }

        if !is_dir {
            if let Some(exts) = &self.extension_filter {
                let ext = candidate
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(normalize_ext)
                    .unwrap_or_default();
                if ext.is_empty() || !exts.contains(&ext) {
                    return None;
                }
            }
        }

        Some(NewEntry {
            path: candidate,
            label: parsed.segment.clone(),
            is_dir,
        })
    }

    fn current_search_key(&self) -> Option<String> {
        if !self.recursive_search {
            return None;
        }

        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.current_dir);
        if parsed.path_mode {
            if let Some(query) = strip_recursive_fuzzy_segment(&parsed.segment) {
                return Some(cache_key(
                    &parsed.view_dir,
                    true,
                    self.hide_hidden,
                    &query,
                    self.show_relative_paths,
                    self.show_info,
                    SearchMode::Fuzzy,
                    self.entry_filter,
                    self.extension_filter.as_ref(),
                ));
            }
            if is_glob_query(raw.trim()) {
                if let Some((base_dir, pattern)) = split_glob_path(raw.trim()) {
                    if is_recursive_glob(&pattern) {
                        let base_path = resolve_path(&base_dir, &self.current_dir);
                        return Some(cache_key(
                            &base_path,
                            true,
                            self.hide_hidden,
                            &pattern,
                            self.show_relative_paths,
                            self.show_info,
                            SearchMode::Glob,
                            self.entry_filter,
                            self.extension_filter.as_ref(),
                        ));
                    }
                }
            }
        }
        if !parsed.path_mode {
            let query = raw.trim();
            if !query.is_empty() {
                if let Some(fuzzy) = strip_recursive_fuzzy(query) {
                    return Some(cache_key(
                        &self.current_dir,
                        true,
                        self.hide_hidden,
                        &fuzzy,
                        self.show_relative_paths,
                        self.show_info,
                        SearchMode::Fuzzy,
                        self.entry_filter,
                        self.extension_filter.as_ref(),
                    ));
                }
                return Some(cache_key(
                    &self.current_dir,
                    true,
                    self.hide_hidden,
                    query,
                    self.show_relative_paths,
                    self.show_info,
                    if is_glob_query(query) {
                        SearchMode::Glob
                    } else {
                        SearchMode::Fuzzy
                    },
                    self.entry_filter,
                    self.extension_filter.as_ref(),
                ));
            }
        }

        None
    }

    fn apply_cached_search_if_ready(&mut self) -> bool {
        let Some(key) = self.current_search_key() else {
            return false;
        };

        if self.last_applied_key.as_deref() == Some(&key) {
            return false;
        }

        let Some(result) = self.cache.get(&key).cloned() else {
            return false;
        };

        self.last_applied_key = Some(key);
        self.apply_search_result(&result);
        true
    }

    fn selected_entry(&self) -> Option<&FileEntry> {
        let idx = self.select.active_index();
        self.entries.get(idx)
    }

    fn enter_dir(&mut self, dir: &Path) {
        self.current_dir = dir.to_path_buf();
        self.input.set_value(path_to_string(&self.current_dir));
        self.refresh_view();
    }

    fn leave_dir(&mut self) {
        if let Some(parent) = self.view_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.input.set_value(path_to_string(&self.current_dir));
            self.refresh_view();
        }
    }

    fn apply_autocomplete(&mut self) -> bool {
        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.current_dir);
        if !parsed.path_mode {
            return false;
        }

        let entries = self.filter_entries(list_dir(&parsed.view_dir, self.hide_hidden));
        let mut candidates: Vec<FileEntry> = if parsed.segment.is_empty() {
            entries
        } else {
            entries
                .into_iter()
                .filter(|entry| entry.name.starts_with(&parsed.segment))
                .collect()
        };

        if candidates.is_empty() {
            return false;
        }

        candidates.sort_by(entry_sort);

        let exact_match = candidates.iter().any(|entry| entry.name == parsed.segment);
        if exact_match && !parsed.ends_with_slash {
            return false;
        }

        let prefix = parsed.segment.clone();
        let mut completed = if candidates.len() == 1 {
            candidates[0].name.clone()
        } else {
            longest_common_prefix(&candidates, prefix.as_str())
        };

        if completed.len() <= prefix.len() {
            return false;
        }

        if candidates.len() == 1 && candidates[0].is_dir && !completed.ends_with('/') {
            completed.push('/');
        }

        let updated = rebuild_path(&parsed, &completed);
        self.input.set_value(updated);
        self.refresh_view();
        true
    }

    fn has_autocomplete_candidates(&self) -> bool {
        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.current_dir);
        if !parsed.path_mode {
            return false;
        }

        let entries = self.filter_entries(list_dir(&parsed.view_dir, self.hide_hidden));
        if parsed.segment.is_empty() {
            return !entries.is_empty();
        }

        entries
            .iter()
            .any(|entry| entry.name.starts_with(&parsed.segment))
    }
}

impl Component for FileBrowserComponent {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine> {
        let mut lines = Vec::new();
        let inline_error = self.input.has_visible_error();
        let (spans, cursor_offset) =
            ctx.render_input_full(&self.input, inline_error, self.base.focused);
        lines.push(RenderLine {
            spans,
            cursor_offset,
        });
        lines.extend(self.select.render(ctx));

        if let Some(new_entry) = self.new_entry_candidate() {
            if self.entries.is_empty() {
                let tag = if new_entry.is_dir { "NEW DIR" } else { "NEW FILE" };
                let tag_style = Style::new().with_color(Color::Green).with_bold();
                let name_style = Style::new().with_color(Color::Yellow);
                lines.push(RenderLine {
                    spans: vec![
                        Span::new("[".to_string()),
                        Span::new(tag).with_style(tag_style),
                        Span::new("] "),
                        Span::new(new_entry.label).with_style(name_style),
                    ],
                    cursor_offset: None,
                });
            }
        }

        if self.is_searching_current() {
            let spinner = self.spinner_frame();
            let style = Style::new().with_color(Color::Cyan).with_bold();
            lines.push(RenderLine {
                spans: vec![
                    Span::new(spinner.to_string()).with_style(style),
                    Span::new(" Searching..."),
                ],
                cursor_offset: None,
            });
        }
        lines
    }

    fn value(&self) -> Option<Value> {
        self.selected_entry()
            .map(|entry| Value::Text(entry.path.to_string_lossy().to_string()))
    }

    fn set_value(&mut self, value: Value) {
        if let Value::Text(text) = value {
            self.input.set_value(text);
            self.refresh_view();
        }
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        ctx: &mut EventContext,
    ) -> bool {
        self.poll_scans();
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('h') => {
                    self.hide_hidden = !self.hide_hidden;
                    self.refresh_view();
                    ctx.handled();
                    return true;
                }
                KeyCode::Char('f') => {
                    self.toggle_entry_filter(EntryFilter::FilesOnly);
                    ctx.handled();
                    return true;
                }
                KeyCode::Char('d') => {
                    self.toggle_entry_filter(EntryFilter::DirsOnly);
                    ctx.handled();
                    return true;
                }
                KeyCode::Char('g') => {
                    self.show_info = !self.show_info;
                    self.refresh_view();
                    ctx.handled();
                    return true;
                }
                _ => {}
            }
        }
        if modifiers == KeyModifiers::NONE {
            match code {
                KeyCode::Up | KeyCode::Down | KeyCode::Char(' ') => {
                    return self.select.handle_key(code, modifiers, ctx);
                }
                KeyCode::Right => {
                    if let Some(entry) = self.selected_entry().cloned() {
                        if entry.is_dir {
                            self.enter_dir(&entry.path);
                            ctx.handled();
                            return true;
                        }
                    }
                    return false;
                }
                KeyCode::Left => {
                    self.leave_dir();
                    ctx.handled();
                    return true;
                }
                KeyCode::Enter => {
                    if self.entries.is_empty() {
                        if let Some(new_entry) = self.new_entry_candidate() {
                            if new_entry.is_dir {
                                if fs::create_dir_all(&new_entry.path).is_ok() {
                                    self.enter_dir(&new_entry.path);
                                    ctx.handled();
                                    return true;
                                }
                            } else {
                                if let Some(parent) = new_entry.path.parent() {
                                    let _ = fs::create_dir_all(parent);
                                }
                                if fs::File::create(&new_entry.path).is_ok() {
                                    ctx.produce(Value::Text(
                                        new_entry.path.to_string_lossy().to_string(),
                                    ));
                                    ctx.handled();
                                    return true;
                                }
                            }
                        }
                    }
                    if let Some(entry) = self.selected_entry().cloned() {
                        if entry.is_dir {
                            self.enter_dir(&entry.path);
                            ctx.handled();
                            return true;
                        }
                    }
                    if let Some(value) = self.value() {
                        ctx.produce(value);
                        return true;
                    }
                    return false;
                }
                KeyCode::Tab => {
                    if !self.has_autocomplete_candidates() {
                        return false;
                    }
                    let _ = self.apply_autocomplete();
                    ctx.handled();
                    return true;
                }
                _ => {}
            }
        }

        let before = self.input.value();
        let result = self.input.handle_key(code, modifiers);
        let after = self.input.value();

        if before != after {
            self.refresh_view();
            ctx.handled();
            return true;
        }

        match result {
            crate::inputs::KeyResult::Submit => {
                ctx.submit();
                true
            }
            crate::inputs::KeyResult::Handled => {
                ctx.handled();
                true
            }
            crate::inputs::KeyResult::NotHandled => false,
        }
    }

    fn poll(&mut self) -> bool {
        let updated_scans = self.poll_scans();
        let updated_cache = self.apply_cached_search_if_ready();
        let mut updated_spinner = false;
        if self.is_searching_current() {
            self.spinner_tick = self.spinner_tick.wrapping_add(1);
            if self.spinner_tick % 3 == 0 {
                self.spinner_index = self.spinner_index.wrapping_add(1);
                updated_spinner = true;
            }
        } else {
            self.spinner_tick = 0;
            self.spinner_index = 0;
        }
        updated_scans || updated_cache || updated_spinner
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        self.input.set_focused(focused);
        self.select.set_focused(focused);
    }

    fn delete_word(&mut self, ctx: &mut EventContext) -> bool {
        let before = self.input.value();
        self.input.delete_word();
        let after = self.input.value();
        if before != after {
            self.refresh_view();
            ctx.handled();
            true
        } else {
            false
        }
    }

    fn delete_word_forward(&mut self, ctx: &mut EventContext) -> bool {
        let before = self.input.value();
        self.input.delete_word_forward();
        let after = self.input.value();
        if before != after {
            self.refresh_view();
            ctx.handled();
            true
        } else {
            false
        }
    }
}

struct ParsedInput {
    path_mode: bool,
    view_dir: PathBuf,
    segment: String,
    ends_with_slash: bool,
    dir_prefix: String,
}

fn parse_input(raw: &str, current_dir: &Path) -> ParsedInput {
    let raw = raw.to_string();
    let trimmed = raw.trim();
    let path_part = trimmed;

    let path_mode = path_part.starts_with('~')
        || path_part.starts_with('/')
        || path_part.starts_with("./")
        || path_part.starts_with("../")
        || path_part.starts_with(".\\")
        || path_part.starts_with("..\\");

    let ends_with_slash = path_part.ends_with('/');
    let (dir_prefix, segment) = split_path(path_part);
    let dir_path = if path_mode {
        resolve_path(&dir_prefix, current_dir)
    } else {
        current_dir.to_path_buf()
    };

    ParsedInput {
        path_mode,
        view_dir: dir_path,
        segment,
        ends_with_slash,
        dir_prefix,
    }
}

fn normalize_input(raw: &str, current_dir: &Path) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return raw.to_string();
    }
    let path_part = trimmed;

    let path_mode = path_part.starts_with('~')
        || path_part.starts_with('/')
        || path_part.starts_with("./")
        || path_part.starts_with("../")
        || path_part.starts_with(".\\")
        || path_part.starts_with("..\\");

    if !path_mode {
        return raw.to_string();
    }

    if is_only_dot_segments(path_part) {
        return raw.to_string();
    }
    if path_part.ends_with("/.") || path_part.ends_with("\\.") {
        return raw.to_string();
    }
    if path_part.ends_with("/..") || path_part.ends_with("\\..") {
        return raw.to_string();
    }

    let normalized_path = normalize_path_part(path_part, current_dir);
    if normalized_path == path_part {
        return raw.to_string();
    }
    normalized_path
}

fn split_path(path: &str) -> (String, String) {
    if path.is_empty() {
        return (String::new(), String::new());
    }
    if path == "~" {
        return ("~/".to_string(), String::new());
    }
    if path.ends_with('/') {
        return (path.to_string(), String::new());
    }
    if let Some(pos) = path.rfind('/') {
        let (dir, seg) = path.split_at(pos + 1);
        (dir.to_string(), seg.to_string())
    } else {
        (String::new(), path.to_string())
    }
}

fn resolve_path(path: &str, current_dir: &Path) -> PathBuf {
    let path = if path.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME") {
            let mut base = PathBuf::from(home);
            let rest = path.trim_start_matches('~').trim_start_matches('/');
            if !rest.is_empty() {
                base.push(rest);
            }
            base
        } else {
            PathBuf::from(path)
        }
    } else if path.starts_with('/') {
        PathBuf::from(path)
    } else if path.is_empty() {
        current_dir.to_path_buf()
    } else {
        current_dir.join(path)
    };
    path
}

fn normalize_path_part(path_part: &str, _current_dir: &Path) -> String {
    if path_part.is_empty() {
        return String::new();
    }

    let uses_backslash = path_part.contains('\\');
    let sep = if uses_backslash { '\\' } else { '/' };
    let path = path_part.replace('\\', "/");

    let trailing_sep = path.ends_with('/');
    let is_absolute = path.starts_with('/');
    let is_tilde = path.starts_with('~');
    let had_dot_prefix = path.starts_with("./");

    if is_tilde {
        let rest = path.trim_start_matches('~');
        let rest = rest.trim_start_matches('/');
        let normalized = normalize_relative_components(rest);
        let mut rebuilt = if normalized.is_empty() {
            "~".to_string()
        } else {
            format!("~/{normalized}")
        };
        if trailing_sep && !rebuilt.ends_with('/') {
            rebuilt.push('/');
        }
        return if sep == '/' {
            rebuilt
        } else {
            rebuilt.replace('/', &sep.to_string())
        };
    }

    let normalized = if is_absolute {
        normalize_absolute_components(&path)
    } else {
        normalize_relative_components(&path)
    };

    let mut rebuilt = if is_absolute {
        format!("/{}", normalized)
    } else if had_dot_prefix && !normalized.starts_with("..") && !normalized.is_empty() {
        format!("./{}", normalized)
    } else {
        normalized
    };

    if rebuilt.is_empty() && is_absolute {
        rebuilt.push('/');
    }

    if trailing_sep && !rebuilt.ends_with('/') {
        rebuilt.push('/');
    }

    if sep == '/' {
        rebuilt
    } else {
        rebuilt.replace('/', &sep.to_string())
    }
}

fn normalize_absolute_components(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        match part {
            "." => {}
            ".." => {
                stack.pop();
            }
            _ => stack.push(part),
        }
    }
    stack.join("/")
}

fn normalize_relative_components(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        match part {
            "." => {}
            ".." => {
                if let Some(last) = stack.last() {
                    if *last != ".." {
                        stack.pop();
                    } else {
                        stack.push("..");
                    }
                } else {
                    stack.push("..");
                }
            }
            _ => stack.push(part),
        }
    }
    stack.join("/")
}

fn is_only_dot_segments(path: &str) -> bool {
    let trimmed = path.trim_matches(|c| c == '/' || c == '\\');
    if trimmed.is_empty() {
        return true;
    }
    for part in trimmed.split(|c| c == '/' || c == '\\') {
        if part.is_empty() {
            continue;
        }
        if part != "." && part != ".." {
            return false;
        }
    }
    true
}

fn list_dir(dir: &Path, hide_hidden: bool) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_type = entry.file_type().ok();
            let is_dir = file_type.map(|t| t.is_dir()).unwrap_or(false);
            let name = entry.file_name().to_string_lossy().to_string();
            if hide_hidden && name.starts_with('.') {
                continue;
            }
            let metadata = entry.metadata().ok();
            entries.push(build_entry(name, path, is_dir, metadata));
        }
    }
    entries.sort_by(entry_sort);
    entries
}

fn list_dir_recursive(dir: &Path, hide_hidden: bool) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    list_dir_recursive_inner(dir, &mut entries, hide_hidden);
    entries.sort_by(entry_sort);
    entries
}

fn list_dir_recursive_glob(dir: &Path, hide_hidden: bool, pattern: &str) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    let normalized = pattern.replace('\\', "/");
    let pattern_segments = split_segments(&normalized);
    let name_pattern = pattern_segments.last().map(|s| s.as_str()).unwrap_or("");
    let literal = longest_literal_chunk(name_pattern);
    let prefix_len = glob_prefix_len(&pattern_segments);
    let has_double_star = pattern_segments.iter().any(|s| s == "**");
    let mut rel_segments = Vec::new();
    list_dir_recursive_glob_inner(
        dir,
        &pattern_segments,
        hide_hidden,
        literal.as_deref(),
        prefix_len,
        has_double_star,
        &mut rel_segments,
        &mut entries,
    );
    entries.sort_by(entry_sort);
    entries
}

fn glob_prefix_len(pattern_segments: &[String]) -> usize {
    for (idx, segment) in pattern_segments.iter().enumerate() {
        if segment == "**" {
            return idx;
        }
    }
    pattern_segments.len()
}

fn list_dir_recursive_inner(dir: &Path, entries: &mut Vec<FileEntry>, hide_hidden: bool) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_type = entry.file_type().ok();
        let is_dir = file_type.map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        if hide_hidden && name.starts_with('.') {
            continue;
        }
        let metadata = entry.metadata().ok();
        entries.push(build_entry(name, path.clone(), is_dir, metadata));
        if is_dir {
            list_dir_recursive_inner(&path, entries, hide_hidden);
        }
    }
}

fn list_dir_recursive_glob_inner(
    dir: &Path,
    pattern_segments: &[String],
    hide_hidden: bool,
    literal: Option<&str>,
    prefix_len: usize,
    has_double_star: bool,
    rel_segments: &mut Vec<String>,
    entries: &mut Vec<FileEntry>,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_type = entry.file_type().ok();
        let is_dir = file_type.map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        if hide_hidden && name.starts_with('.') {
            continue;
        }
        let metadata = entry.metadata().ok();

        rel_segments.push(name.clone());

        if !has_double_star && rel_segments.len() > pattern_segments.len() {
            rel_segments.pop();
            continue;
        }

        if rel_segments.len() <= prefix_len
            && !glob_prefix_matches(pattern_segments, rel_segments, prefix_len)
        {
            rel_segments.pop();
            continue;
        }

        let literal_ok = literal.map(|lit| name.contains(lit)).unwrap_or(true);
        if literal_ok && glob_match_segments(pattern_segments, rel_segments) {
            entries.push(build_entry(name, path.clone(), is_dir, metadata));
        }

        if is_dir {
            list_dir_recursive_glob_inner(
                &path,
                pattern_segments,
                hide_hidden,
                literal,
                prefix_len,
                has_double_star,
                rel_segments,
                entries,
            );
        }

        rel_segments.pop();
    }
}

fn entry_sort(a: &FileEntry, b: &FileEntry) -> Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name_lower.cmp(&b.name_lower),
    }
}

const MAX_MATCHES: usize = 2000;

fn options_from_query(
    entries: &[FileEntry],
    query: &str,
    display_root: Option<&Path>,
    show_relative: bool,
    show_info: bool,
) -> (Vec<FileEntry>, Vec<SelectOption>, Vec<fuzzy::FuzzyMatch>) {
    let query = query.trim();
    if query.is_empty() {
        let options = entries
            .iter()
            .map(|entry| entry_option(entry, &[], display_root, show_relative, show_info))
            .collect::<Vec<_>>();
        return (entries.to_vec(), options, Vec::new());
    }

    let mut indices: Vec<usize> = (0..entries.len()).collect();
    if let Some(filtered) = prefilter_entries(entries, query) {
        indices = filtered;
    }

    let names = indices
        .iter()
        .map(|idx| entries[*idx].name.clone())
        .collect::<Vec<_>>();
    let mut matches = if names.len() > MAX_MATCHES * 4 {
        fuzzy::match_candidates_top(query, &names, MAX_MATCHES)
    } else {
        fuzzy::match_candidates(query, &names)
    };

    matches.sort_by(|a, b| {
        let a_entry = indices
            .get(a.index)
            .and_then(|idx| entries.get(*idx));
        let b_entry = indices
            .get(b.index)
            .and_then(|idx| entries.get(*idx));
        match (a_entry, b_entry) {
            (Some(ae), Some(be)) => {
                let dir_order = match (ae.is_dir, be.is_dir) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => Ordering::Equal,
                };
                if dir_order != Ordering::Equal {
                    return dir_order;
                }
            }
            _ => {}
        }
        b.score.cmp(&a.score)
    });

    let mut matched_entries = Vec::with_capacity(matches.len());
    let mut options = Vec::with_capacity(matches.len());
    let mut adjusted = Vec::with_capacity(matches.len());
    for (pos, m) in matches.into_iter().enumerate() {
        if let Some(entry_idx) = indices.get(m.index).copied() {
            if let Some(entry) = entries.get(entry_idx) {
                matched_entries.push(entry.clone());
                options.push(entry_option(
                    entry,
                    &m.ranges,
                    display_root,
                    show_relative,
                    show_info,
                ));
                adjusted.push(fuzzy::FuzzyMatch {
                    index: pos,
                    score: m.score,
                    matched_indices: m.matched_indices,
                    ranges: m.ranges,
                });
            }
        }
    }

    (matched_entries, options, adjusted)
}

fn glob_options(
    entries: &[FileEntry],
    pattern: &str,
    display_root: Option<&Path>,
    show_relative: bool,
    show_info: bool,
) -> (Vec<FileEntry>, Vec<SelectOption>) {
    let normalized = pattern.replace('\\', "/");
    let pattern_segments = split_segments(&normalized);
    let use_path = normalized.contains('/');
    let name_pattern = pattern_segments.last().map(|s| s.as_str()).unwrap_or("");
    let literal = longest_literal_chunk(name_pattern);

    let mut matched_entries = Vec::new();
    for entry in entries {
        if let Some(lit) = &literal {
            if !entry.name.contains(lit) {
                continue;
            }
        }
        let target = if use_path {
            relative_path_for_match(entry, display_root)
        } else {
            entry.name.clone()
        };
        if glob_match_path_segments(&pattern_segments, &target) {
            matched_entries.push(entry.clone());
        }
    }
    let options = build_glob_options(
        &mut matched_entries,
        name_pattern,
        display_root,
        show_relative,
        show_info,
    );
    (matched_entries, options)
}

fn relative_path_for_match(entry: &FileEntry, display_root: Option<&Path>) -> String {
    let path = if let Some(root) = display_root {
        entry.path.strip_prefix(root).unwrap_or(&entry.path)
    } else {
        &entry.path
    };
    path.to_string_lossy().replace('\\', "/")
}

fn build_glob_options(
    entries: &mut Vec<FileEntry>,
    name_pattern: &str,
    display_root: Option<&Path>,
    show_relative: bool,
    show_info: bool,
) -> Vec<SelectOption> {
    if entries.is_empty() {
        return Vec::new();
    }
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    indices.sort_by(|&a, &b| {
        let ea = &entries[a];
        let eb = &entries[b];
        let depth_a = match glob_depth(ea, display_root) {
            Some(depth) => depth,
            None => usize::MAX,
        };
        let depth_b = match glob_depth(eb, display_root) {
            Some(depth) => depth,
            None => usize::MAX,
        };
        depth_a
            .cmp(&depth_b)
            .then_with(|| match (ea.is_dir, eb.is_dir) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => Ordering::Equal,
            })
            .then_with(|| ea.name.to_lowercase().cmp(&eb.name.to_lowercase()))
    });

    let mut sorted_entries = Vec::with_capacity(entries.len());
    let mut sorted_options = Vec::with_capacity(entries.len());
    for idx in indices {
        let entry = entries[idx].clone();
        let highlights = glob_highlights(name_pattern, &entry.name);
        sorted_entries.push(entry.clone());
        sorted_options.push(entry_option(
            &entry,
            &highlights,
            display_root,
            show_relative,
            show_info,
        ));
    }
    *entries = sorted_entries;
    sorted_options
}

fn glob_depth(entry: &FileEntry, display_root: Option<&Path>) -> Option<usize> {
    let rel = relative_path_for_match(entry, display_root);
    let segments = split_segments(&rel);
    if segments.is_empty() {
        None
    } else {
        Some(segments.len())
    }
}

fn split_segments(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

fn glob_match_path_segments(pattern_segments: &[String], target: &str) -> bool {
    let target_segments = split_segments(target);
    glob_match_segments(pattern_segments, &target_segments)
}


fn glob_match_segments(pattern: &[String], target: &[String]) -> bool {
    if pattern.is_empty() {
        return target.is_empty();
    }

    let head = &pattern[0];
    if head == "**" {
        if glob_match_segments(&pattern[1..], target) {
            return true;
        }
        if !target.is_empty() {
            return glob_match_segments(pattern, &target[1..]);
        }
        return false;
    }

    if target.is_empty() {
        return false;
    }
    if !glob_match_segment(head, &target[0]) {
        return false;
    }
    glob_match_segments(&pattern[1..], &target[1..])
}

fn glob_prefix_matches(pattern: &[String], target: &[String], prefix_len: usize) -> bool {
    let len = target.len().min(prefix_len);
    for idx in 0..len {
        if !glob_match_segment(&pattern[idx], &target[idx]) {
            return false;
        }
    }
    true
}

fn glob_match_segment(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut pi = 0usize;
    let mut ti = 0usize;
    let mut star_idx: Option<usize> = None;
    let mut match_idx = 0usize;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_idx = Some(pi);
            match_idx = ti;
            pi += 1;
        } else if let Some(star) = star_idx {
            pi = star + 1;
            match_idx += 1;
            ti = match_idx;
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }

    pi == p.len()
}

fn glob_highlights(pattern: &str, name: &str) -> Vec<(usize, usize)> {
    let literals = glob_literal_chunks(pattern);
    if literals.is_empty() {
        return Vec::new();
    }
    let mut best = String::new();
    for lit in literals {
        if lit.len() > best.len() {
            best = lit;
        }
    }
    if best.is_empty() {
        return Vec::new();
    }
    let name_chars: Vec<char> = name.chars().collect();
    let best_chars: Vec<char> = best.chars().collect();
    if best_chars.len() > name_chars.len() {
        return Vec::new();
    }
    for start in 0..=name_chars.len() - best_chars.len() {
        if name_chars[start..start + best_chars.len()] == best_chars[..] {
            return vec![(start, start + best_chars.len())];
        }
    }
    Vec::new()
}

fn glob_literal_chunks(pattern: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in pattern.chars() {
        if ch == '*' || ch == '?' {
            if !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn longest_literal_chunk(pattern: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for chunk in glob_literal_chunks(pattern) {
        if best.as_ref().map(|b| chunk.len() > b.len()).unwrap_or(true) {
            best = Some(chunk);
        }
    }
    best
}

fn prefilter_entries(entries: &[FileEntry], query: &str) -> Option<Vec<usize>> {
    if query.contains('/') || query.contains('\\') {
        return None;
    }
    if !query.starts_with('.') {
        let q = query.to_ascii_lowercase();
        if q.len() < 2 {
            return None;
        }
        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                if entry.name_lower.contains(&q) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        if filtered.is_empty() {
            return None;
        }
        return Some(filtered);
    }
    let needle = query.to_ascii_lowercase();
    let filtered: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            if entry.name_lower.ends_with(&needle) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if filtered.is_empty() {
        None
    } else {
        Some(filtered)
    }
}

fn entry_option(
    entry: &FileEntry,
    highlights: &[(usize, usize)],
    display_root: Option<&Path>,
    show_relative: bool,
    show_info: bool,
) -> SelectOption {
    let suffix = if show_info {
        entry_info_suffix(entry).map(|info| format!("  {}", info))
    } else {
        None
    };
    if show_relative {
        if let Some(root) = display_root {
            if let Some(prefix) = relative_prefix(&entry.path, root) {
                let name = entry.name.clone();
                let suffix_text = suffix.unwrap_or_default();
                let text = format!("{}{}{}", prefix, name, suffix_text);
                let name_start = prefix.chars().count();
                let suffix_start = name_start + name.chars().count();
                let prefix_style = Style::new().with_color(Color::DarkGrey).with_dim();
                let name_style = if entry.is_dir {
                    Style::new().with_color(Color::Blue).with_bold()
                } else {
                    Style::new()
                };
                if suffix_text.is_empty() {
                    return SelectOption::Split {
                        text,
                        name_start,
                        highlights: highlights.to_vec(),
                        prefix_style,
                        name_style,
                    };
                }
                let suffix_style = Style::new().with_color(Color::DarkGrey).with_dim();
                return SelectOption::SplitSuffix {
                    text,
                    name_start,
                    suffix_start,
                    highlights: highlights.to_vec(),
                    prefix_style,
                    name_style,
                    suffix_style,
                };
            }
        }
    }

    if let Some(suffix_text) = suffix {
        let suffix_start = entry.name.chars().count();
        let text = format!("{}{}", entry.name, suffix_text);
        let suffix_style = Style::new().with_color(Color::DarkGrey).with_dim();
        let style = if entry.is_dir {
            Style::new().with_color(Color::Blue).with_bold()
        } else {
            Style::new()
        };
        return SelectOption::Suffix {
            text,
            highlights: highlights.to_vec(),
            suffix_start,
            style,
            suffix_style,
        };
    }

    if entry.is_dir {
        SelectOption::Styled {
            text: entry.name.clone(),
            highlights: highlights.to_vec(),
            style: Style::new().with_color(Color::Blue).with_bold(),
        }
    } else if highlights.is_empty() {
        SelectOption::Plain(entry.name.clone())
    } else {
        SelectOption::Highlighted {
            text: entry.name.clone(),
            highlights: highlights.to_vec(),
        }
    }
}

fn entry_info_suffix(entry: &FileEntry) -> Option<String> {
    let mut parts = Vec::new();
    if !entry.is_dir {
        if let Some(size) = entry.size {
            parts.push(format_size(size));
        }
    }
    if let Some(modified) = entry.modified {
        parts.push(format_age(modified));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut value = size as f64;
    let mut unit = UNITS[0];
    for next in UNITS.iter().skip(1) {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    if unit == "B" {
        format!("{}B", size)
    } else if value >= 10.0 {
        format!("{:.0}{}", value, unit)
    } else {
        format!("{:.1}{}", value, unit)
    }
}

fn format_age(modified: SystemTime) -> String {
    let delta = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_else(|_| Duration::ZERO);
    let secs = delta.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 60 * 60 {
        format!("{}m", secs / 60)
    } else if secs < 60 * 60 * 24 {
        format!("{}h", secs / 3600)
    } else if secs < 60 * 60 * 24 * 30 {
        format!("{}d", secs / (60 * 60 * 24))
    } else if secs < 60 * 60 * 24 * 365 {
        format!("{}mo", secs / (60 * 60 * 24 * 30))
    } else {
        format!("{}y", secs / (60 * 60 * 24 * 365))
    }
}

fn rebuild_path(parsed: &ParsedInput, segment: &str) -> String {
    let mut base = parsed.dir_prefix.clone();
    base.push_str(segment);
    base
}

fn path_to_string(path: &Path) -> String {
    let mut text = path.to_string_lossy().to_string();
    if !text.ends_with('/') {
        text.push('/');
    }
    text
}

fn longest_common_prefix(entries: &[FileEntry], prefix: &str) -> String {
    let mut common = prefix.to_string();
    if entries.is_empty() {
        return common;
    }
    let mut chars = entries[0].name.chars().collect::<Vec<_>>();
    for entry in entries.iter().skip(1) {
        let other = entry.name.chars().collect::<Vec<_>>();
        let len = chars.len().min(other.len());
        let mut i = 0;
        while i < len && chars[i] == other[i] {
            i += 1;
        }
        chars.truncate(i);
        if chars.is_empty() {
            break;
        }
    }
    common.clear();
    common.extend(chars);
    if common.len() < prefix.len() {
        prefix.to_string()
    } else {
        common
    }
}

fn cache_key(
    dir: &Path,
    recursive: bool,
    hide_hidden: bool,
    query: &str,
    show_relative: bool,
    show_info: bool,
    mode: SearchMode,
    entry_filter: EntryFilter,
    ext_filter: Option<&HashSet<String>>,
) -> String {
    let display = if show_relative { "rel" } else { "name" };
    let mode = match mode {
        SearchMode::Fuzzy => "f",
        SearchMode::Glob => "g",
    };
    let filter = match entry_filter {
        EntryFilter::All => "a",
        EntryFilter::FilesOnly => "f",
        EntryFilter::DirsOnly => "d",
    };
    let ext_tag = if let Some(exts) = ext_filter {
        let mut list = exts.iter().cloned().collect::<Vec<_>>();
        list.sort();
        list.join(",")
    } else {
        String::new()
    };
    format!(
        "{}|r:{}|h:{}|d:{}|i:{}|m:{}|f:{}|e:{}|q:{}",
        dir.to_string_lossy(),
        recursive,
        hide_hidden,
        display,
        show_info,
        mode,
        filter,
        ext_tag,
        query
    )
}

fn dir_cache_key(dir: &Path, recursive: bool, hide_hidden: bool) -> String {
    format!(
        "{}|r:{}|h:{}",
        dir.to_string_lossy(),
        recursive,
        hide_hidden
    )
}

fn is_glob_query(query: &str) -> bool {
    query.contains('*') || query.contains('?')
}

fn is_recursive_glob(pattern: &str) -> bool {
    pattern.contains("**") || pattern.contains('/')
}

fn split_glob_path(path_part: &str) -> Option<(String, String)> {
    if !is_glob_query(path_part) {
        return None;
    }
    let normalized = path_part.replace('\\', "/");
    let first_glob = normalized.find(|ch| matches!(ch, '*' | '?'))?;
    let before = &normalized[..first_glob];
    if let Some(last_slash) = before.rfind('/') {
        let base_dir = normalized[..=last_slash].to_string();
        let pattern = normalized[last_slash + 1..].to_string();
        Some((base_dir, pattern))
    } else {
        Some((String::new(), normalized))
    }
}

fn strip_recursive_fuzzy(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if !trimmed.starts_with("**") {
        return None;
    }
    let rest = trimmed.trim_start_matches("**");
    if rest.starts_with('/') || rest.starts_with('\\') {
        return None;
    }
    let rest = rest.trim();
    if rest.contains('*') || rest.contains('?') {
        return None;
    }
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn filter_entries(
    mut entries: Vec<FileEntry>,
    entry_filter: EntryFilter,
    ext_filter: Option<&HashSet<String>>,
) -> Vec<FileEntry> {
    entries.retain(|entry| match entry_filter {
        EntryFilter::All => true,
        EntryFilter::FilesOnly => !entry.is_dir,
        EntryFilter::DirsOnly => entry.is_dir,
    });

    if let Some(exts) = ext_filter {
        entries.retain(|entry| {
            if entry.is_dir {
                true
            } else {
                entry
                    .ext_lower
                    .as_ref()
                    .map(|ext| exts.contains(ext))
                    .unwrap_or(false)
            }
        });
    }

    entries
}

fn normalize_ext(ext: &str) -> String {
    ext.trim_start_matches('.').to_ascii_lowercase()
}

fn build_entry(name: String, path: PathBuf, is_dir: bool, metadata: Option<fs::Metadata>) -> FileEntry {
    let name_lower = name.to_ascii_lowercase();
    let ext_lower = if is_dir {
        None
    } else {
        name.rsplit_once('.')
            .map(|(_, ext)| normalize_ext(ext))
            .filter(|ext| !ext.is_empty())
    };
    let size = metadata
        .as_ref()
        .and_then(|meta| if is_dir { None } else { Some(meta.len()) });
    let modified = metadata.and_then(|meta| meta.modified().ok());
    FileEntry {
        name,
        name_lower,
        ext_lower,
        path,
        is_dir,
        size,
        modified,
    }
}

fn strip_recursive_fuzzy_segment(segment: &str) -> Option<String> {
    let trimmed = segment.trim();
    if !trimmed.starts_with("**") {
        return None;
    }
    let rest = trimmed.trim_start_matches("**").trim();
    if rest.contains('*') || rest.contains('?') {
        return None;
    }
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn relative_prefix(path: &Path, root: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parent = rel.parent();
    let Some(parent) = parent else {
        return Some(String::new());
    };
    let prefix = parent.to_string_lossy().to_string();
    if prefix.is_empty() || prefix == "." {
        return Some(String::new());
    }
    let mut display = prefix.replace('\\', "/");
    if !display.ends_with('/') {
        display.push('/');
    }
    Some(elide_middle(&display, RELATIVE_PREFIX_MAX))
}

const RELATIVE_PREFIX_MAX: usize = 24;

fn elide_middle(text: &str, max_len: usize) -> String {
    let len = text.chars().count();
    if len <= max_len {
        return text.to_string();
    }
    if max_len <= 3 {
        return "...".to_string();
    }
    let keep = max_len - 3;
    let head_len = keep / 2;
    let tail_len = keep - head_len;
    let head: String = text.chars().take(head_len).collect();
    let tail: String = text.chars().rev().take(tail_len).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{}...{}", head, tail)
}
