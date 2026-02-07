use crate::components::select_component::{SelectComponent, SelectMode};
use crate::core::component::{Component, ComponentResponse};
use crate::core::search::fuzzy;
use crate::core::value::Value;
use crate::inputs::Input;
use crate::inputs::text_input::TextInput;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine, RenderOutput};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use super::cache::SearchKey;
use super::model::{
    EntryFilter, FileEntry, NewEntry, SearchMode, SearchResult, entry_sort, filter_entries,
    list_dir, normalize_ext,
};
use super::parser::{
    is_glob_query, is_recursive_glob, normalize_input, parse_input, path_to_string, rebuild_path,
    resolve_path, split_glob_path, strip_recursive_fuzzy, strip_recursive_fuzzy_segment,
};
use super::scanner::{ScanRequest, ScannerHandle};
use super::search::{
    build_options, compute_max_name_width, glob_options, longest_common_prefix, options_from_query,
};
use super::search_state::SearchState;

struct NavigationState {
    current_dir: PathBuf,
    view_dir: PathBuf,
    entries: Vec<FileEntry>,
    matches: Vec<fuzzy::FuzzyMatch>,
}

pub struct FileBrowserState {
    input: TextInput,
    select: SelectComponent,
    nav: NavigationState,
    search: SearchState,
    scanner: ScannerHandle,
    scan_rx: Receiver<(SearchKey, SearchResult)>,
}

impl FileBrowserState {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let input = TextInput::new(format!("{}_filter", id), "Path");
        let select =
            SelectComponent::new(format!("{}_list", id), Vec::new()).with_mode(SelectMode::List);
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (scan_tx, scan_rx) = mpsc::channel();
        let scanner = ScannerHandle::new(scan_tx);
        let mut component = Self {
            input,
            select,
            nav: NavigationState {
                current_dir: current_dir.clone(),
                view_dir: current_dir,
                entries: Vec::new(),
                matches: Vec::new(),
            },
            search: SearchState::new(),
            scanner,
            scan_rx,
        };
        component.refresh_view();
        component
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.input.base_mut_ref().label = label.into();
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        let placeholder = placeholder.into();
        self.input.base_mut_ref().placeholder = Some(placeholder);
    }

    pub fn set_max_visible(&mut self, max_visible: usize) {
        self.select.set_max_visible(max_visible);
    }

    pub fn set_recursive_search(&mut self, recursive: bool) {
        self.search.recursive_search = recursive;
        self.refresh_view();
    }

    pub fn set_entry_filter(&mut self, filter: EntryFilter) {
        self.search.entry_filter = filter;
        self.refresh_view();
    }

    fn toggle_entry_filter(&mut self, filter: EntryFilter) {
        if self.search.entry_filter == filter {
            self.search.entry_filter = EntryFilter::All;
        } else {
            self.search.entry_filter = filter;
        }
        self.refresh_view();
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
            self.search.extension_filter = None;
        } else {
            self.search.extension_filter = Some(normalized);
        }
        self.refresh_view();
    }

    pub fn clear_extension_filter(&mut self) {
        self.search.extension_filter = None;
        self.refresh_view();
    }

    fn filter_entries(&self, entries: Vec<FileEntry>) -> Vec<FileEntry> {
        filter_entries(
            entries,
            self.search.entry_filter,
            self.search.extension_filter.as_ref(),
        )
    }

    pub fn set_relative_paths(&mut self, show_relative: bool) {
        self.search.show_relative_paths = show_relative;
        self.refresh_view();
    }

    pub fn set_show_hidden(&mut self, show_hidden: bool) {
        self.search.hide_hidden = !show_hidden;
        self.refresh_view();
    }

    pub fn set_show_info(&mut self, show_info: bool) {
        self.search.show_info = show_info;
        self.refresh_view();
    }

    pub fn set_current_dir(&mut self, dir: impl Into<PathBuf>) {
        self.nav.current_dir = dir.into();
        self.refresh_view();
    }

    fn refresh_view(&mut self) {
        self.poll_scans();
        let raw = self.input.value();
        let normalized = normalize_input(&raw, &self.nav.current_dir);
        if normalized != raw {
            self.input.set_value(normalized.clone());
        }
        let parsed = parse_input(&normalized, &self.nav.current_dir);
        self.nav.view_dir = parsed.view_dir.clone();

        // Handle path mode
        if parsed.path_mode {
            let raw = normalized.trim();
            if let Some(query) = strip_recursive_fuzzy_segment(&parsed.segment) {
                if let Some(result) = self.search_async(
                    &parsed.view_dir,
                    true,
                    &query,
                    &parsed.view_dir,
                    SearchMode::Fuzzy,
                ) {
                    self.apply_search_result(&result);
                    return;
                }
                self.set_empty_results();
                return;
            }

            if is_glob_query(raw) {
                if let Some((base_dir, pattern)) = split_glob_path(normalized.trim()) {
                    let base_path = resolve_path(&base_dir, &self.nav.current_dir);
                    let recursive = is_recursive_glob(&pattern);
                    if recursive {
                        if let Some(result) = self.search_async(
                            &base_path,
                            true,
                            &pattern,
                            &base_path,
                            SearchMode::Glob,
                        ) {
                            self.apply_search_result(&result);
                            return;
                        }
                        self.set_empty_results();
                        return;
                    }

                    let entries =
                        self.filter_entries(list_dir(&base_path, self.search.hide_hidden));
                    let (entries, _) = glob_options(
                        &entries,
                        &pattern,
                        Some(&base_path),
                        self.search.show_relative_paths,
                        self.search.show_info,
                    );
                    self.set_results_no_matches(entries, Some(&base_path));
                    return;
                }
                self.set_empty_results();
                return;
            }

            let entries = self.filter_entries(list_dir(&parsed.view_dir, self.search.hide_hidden));
            if parsed.segment.is_empty() {
                self.set_results_no_matches(entries, Some(&parsed.view_dir));
                return;
            }

            let (entries, _, matches) = options_from_query(
                &entries,
                &parsed.segment,
                Some(&parsed.view_dir),
                self.search.show_relative_paths,
                self.search.show_info,
            );
            self.set_results_with_matches(entries, matches, Some(&parsed.view_dir));
            return;
        }

        // Handle non-path mode
        if normalized.trim().is_empty() {
            let current_dir = self.nav.current_dir.clone();
            let entries = self.filter_entries(list_dir(&current_dir, self.search.hide_hidden));
            self.set_results_no_matches(entries, Some(&current_dir));
            return;
        }

        let raw = normalized.trim();
        if let Some(query) = strip_recursive_fuzzy(raw) {
            let current_dir = self.nav.current_dir.clone();
            if let Some(result) =
                self.search_async(&current_dir, true, &query, &current_dir, SearchMode::Fuzzy)
            {
                self.apply_search_result(&result);
                return;
            }
            self.set_empty_results();
            return;
        }

        if is_glob_query(raw) {
            let recursive = is_recursive_glob(raw);
            if recursive {
                let current_dir = self.nav.current_dir.clone();
                if let Some(result) =
                    self.search_async(&current_dir, true, raw, &current_dir, SearchMode::Glob)
                {
                    self.apply_search_result(&result);
                    return;
                }
                self.set_empty_results();
                return;
            }

            let current_dir = self.nav.current_dir.clone();
            let entries = self.filter_entries(list_dir(&current_dir, self.search.hide_hidden));
            let (entries, _) = glob_options(
                &entries,
                raw,
                Some(&current_dir),
                self.search.show_relative_paths,
                self.search.show_info,
            );
            self.set_results_no_matches(entries, Some(&current_dir));
            return;
        }

        if self.search.recursive_search {
            let current_dir = self.nav.current_dir.clone();
            if let Some(result) =
                self.search_async(&current_dir, true, raw, &current_dir, SearchMode::Fuzzy)
            {
                self.apply_search_result(&result);
                return;
            }
            self.set_empty_results();
            return;
        }

        let current_dir = self.nav.current_dir.clone();
        let entries = self.filter_entries(list_dir(&current_dir, self.search.hide_hidden));
        let (entries, _, matches) = options_from_query(
            &entries,
            raw,
            Some(&current_dir),
            self.search.show_relative_paths,
            self.search.show_info,
        );
        self.set_results_with_matches(entries, matches, Some(&current_dir));
    }

    fn set_empty_results(&mut self) {
        self.nav.entries = Vec::new();
        self.nav.matches = Vec::new();
        self.select.set_options(Vec::new());
        self.select.reset_active();
    }

    fn set_results_no_matches(&mut self, entries: Vec<FileEntry>, display_root: Option<&Path>) {
        self.set_results(
            entries,
            Vec::new(),
            display_root,
            self.search.show_relative_paths,
            self.search.show_info,
        );
    }

    fn set_results_with_matches(
        &mut self,
        entries: Vec<FileEntry>,
        matches: Vec<fuzzy::FuzzyMatch>,
        display_root: Option<&Path>,
    ) {
        self.set_results(
            entries,
            matches,
            display_root,
            self.search.show_relative_paths,
            self.search.show_info,
        );
    }

    fn poll_scans(&mut self) -> bool {
        let mut updated = false;
        let current_key = self.current_search_key();

        let mut to_apply: Option<SearchKey> = None;
        for (key, result) in self.scan_rx.try_iter() {
            self.search.cache.clear_in_flight(&key);
            let is_current = current_key.as_ref() == Some(&key);
            self.search.cache.insert(key.clone(), result);
            if is_current {
                to_apply = Some(key);
                updated = true;
            }
        }
        if let Some(key) = to_apply {
            self.search.cache.set_last_applied(key.clone());
            if let Some(result) = self.search.cache.get(&key) {
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
        let key = SearchKey::new(
            dir,
            recursive,
            self.search.hide_hidden,
            query,
            self.search.show_relative_paths,
            self.search.show_info,
            mode,
            self.search.entry_filter,
            self.search.extension_filter.as_ref(),
        );
        if let Some(result) = self.search.cache.get(&key) {
            self.search.cache.set_last_applied(key);
            return Some(result);
        }

        if self.search.cache.is_in_flight(&key) {
            return None;
        }

        self.search.cache.mark_in_flight(key.clone());

        let request = ScanRequest {
            key: key.clone(),
            dir: dir.to_path_buf(),
            recursive,
            query: query.to_string(),
            display_root: display_root.to_path_buf(),
            hide_hidden: self.search.hide_hidden,
            show_relative: self.search.show_relative_paths,
            show_info: self.search.show_info,
            mode,
            entry_filter: self.search.entry_filter,
            ext_filter: self.search.extension_filter.clone(),
        };
        self.scanner.submit(request);

        None
    }

    fn apply_search_result(&mut self, result: &SearchResult) {
        self.set_results(
            result.entries.clone(),
            result.matches.clone(),
            result.display_root.as_deref(),
            result.show_relative,
            result.show_info,
        );
    }

    fn set_results(
        &mut self,
        entries: Vec<FileEntry>,
        matches: Vec<fuzzy::FuzzyMatch>,
        display_root: Option<&Path>,
        show_relative: bool,
        show_info: bool,
    ) {
        let options = build_options(
            &entries,
            if matches.is_empty() {
                None
            } else {
                Some(&matches)
            },
            display_root,
            show_relative,
            show_info,
        );
        self.nav.entries = entries;
        self.nav.matches = matches;
        self.select.set_options(options);
        self.select.reset_active();
    }

    fn is_searching_current(&self) -> bool {
        let Some(key) = self.current_search_key() else {
            return false;
        };
        self.search.cache.is_in_flight(&key)
    }

    fn spinner_frame(&self) -> &'static str {
        const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        FRAMES[self.search.spinner_index % FRAMES.len()]
    }

    fn new_entry_candidate(&self) -> Option<NewEntry> {
        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.nav.current_dir);
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

        let base = resolve_path(&parsed.dir_prefix, &self.nav.current_dir);
        let candidate = base.join(&parsed.segment);
        if candidate.exists() {
            return None;
        }

        let is_dir = !parsed.segment.contains('.');
        match self.search.entry_filter {
            EntryFilter::FilesOnly if is_dir => return None,
            EntryFilter::DirsOnly if !is_dir => return None,
            _ => {}
        }

        if !is_dir {
            if let Some(exts) = &self.search.extension_filter {
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

    fn current_search_key(&self) -> Option<SearchKey> {
        if !self.search.recursive_search {
            return None;
        }

        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.nav.current_dir);
        if parsed.path_mode {
            if let Some(query) = strip_recursive_fuzzy_segment(&parsed.segment) {
                return Some(SearchKey::new(
                    &parsed.view_dir,
                    true,
                    self.search.hide_hidden,
                    &query,
                    self.search.show_relative_paths,
                    self.search.show_info,
                    SearchMode::Fuzzy,
                    self.search.entry_filter,
                    self.search.extension_filter.as_ref(),
                ));
            }
            if is_glob_query(raw.trim()) {
                if let Some((base_dir, pattern)) = split_glob_path(raw.trim()) {
                    if is_recursive_glob(&pattern) {
                        let base_path = resolve_path(&base_dir, &self.nav.current_dir);
                        return Some(SearchKey::new(
                            &base_path,
                            true,
                            self.search.hide_hidden,
                            &pattern,
                            self.search.show_relative_paths,
                            self.search.show_info,
                            SearchMode::Glob,
                            self.search.entry_filter,
                            self.search.extension_filter.as_ref(),
                        ));
                    }
                }
            }
        }
        if !parsed.path_mode {
            let query = raw.trim();
            if !query.is_empty() {
                if let Some(fuzzy) = strip_recursive_fuzzy(query) {
                    return Some(SearchKey::new(
                        &self.nav.current_dir,
                        true,
                        self.search.hide_hidden,
                        &fuzzy,
                        self.search.show_relative_paths,
                        self.search.show_info,
                        SearchMode::Fuzzy,
                        self.search.entry_filter,
                        self.search.extension_filter.as_ref(),
                    ));
                }
                return Some(SearchKey::new(
                    &self.nav.current_dir,
                    true,
                    self.search.hide_hidden,
                    query,
                    self.search.show_relative_paths,
                    self.search.show_info,
                    if is_glob_query(query) {
                        SearchMode::Glob
                    } else {
                        SearchMode::Fuzzy
                    },
                    self.search.entry_filter,
                    self.search.extension_filter.as_ref(),
                ));
            }
        }

        None
    }

    fn apply_cached_search_if_ready(&mut self) -> bool {
        let Some(key) = self.current_search_key() else {
            return false;
        };

        if self.search.cache.last_applied() == Some(&key) {
            return false;
        }

        let Some(result) = self.search.cache.get(&key) else {
            return false;
        };

        self.search.cache.set_last_applied(key);
        self.apply_search_result(&result);
        true
    }

    fn selected_entry(&self) -> Option<&FileEntry> {
        let idx = self.select.active_index();
        self.nav.entries.get(idx)
    }

    fn enter_dir(&mut self, dir: &Path) {
        self.nav.current_dir = dir.to_path_buf();
        self.input.set_value(path_to_string(&self.nav.current_dir));
        self.refresh_view();
    }

    fn leave_dir(&mut self) {
        if let Some(parent) = self.nav.view_dir.parent() {
            self.nav.current_dir = parent.to_path_buf();
            self.input.set_value(path_to_string(&self.nav.current_dir));
            self.refresh_view();
        }
    }

    fn apply_autocomplete(&mut self) -> bool {
        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.nav.current_dir);
        if !parsed.path_mode {
            return false;
        }

        let entries = self.filter_entries(list_dir(&parsed.view_dir, self.search.hide_hidden));
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

    fn mark_input_changed(&mut self) {
        self.search.mark_input_changed();
    }

    fn has_autocomplete_candidates(&self) -> bool {
        let raw = self.input.value();
        let parsed = parse_input(&raw, &self.nav.current_dir);
        if !parsed.path_mode {
            return false;
        }

        let entries = self.filter_entries(list_dir(&parsed.view_dir, self.search.hide_hidden));
        if parsed.segment.is_empty() {
            return !entries.is_empty();
        }

        entries
            .iter()
            .any(|entry| entry.name.starts_with(&parsed.segment))
    }
}

impl FileBrowserState {
    pub fn input_id(&self) -> &str {
        &self.input.base_ref().id
    }

    pub fn list_id(&self) -> &str {
        self.select.id()
    }

    pub fn input(&self) -> &TextInput {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut TextInput {
        &mut self.input
    }

    pub fn select(&self) -> &SelectComponent {
        &self.select
    }

    pub fn select_mut(&mut self) -> &mut SelectComponent {
        &mut self.select
    }

    pub fn render_input_line(&self, ctx: &RenderContext, focused: bool) -> RenderOutput {
        let inline_error = self.input.has_visible_error();
        ctx.render_input_full(&self.input, inline_error, focused)
    }

    pub fn render_list_lines(&mut self, ctx: &RenderContext, focused: bool) -> RenderOutput {
        let mut lines = Vec::new();
        // Add column headers if showing info
        if self.search.show_info && !self.nav.entries.is_empty() {
            let max_name_width = compute_max_name_width(&self.nav.entries, true);

            let header_style = Style::new().with_color(Color::DarkGrey).with_dim();
            // NAME is 4 chars, so padding is max_name_width - 4
            let padding_after_name = if max_name_width > 4 {
                " ".repeat(max_name_width - 4)
            } else {
                String::new()
            };
            lines.push(RenderLine {
                spans: vec![
                    Span::new(format!(
                        "  NAME{}    {:>5}  {:>8}  {:>7}",
                        padding_after_name, "TYPE", "SIZE", "MODIFIED"
                    ))
                    .with_style(header_style),
                ],
            });
        }

        let prev_focus = self.select.is_focused();
        self.select.set_focused(focused);
        let select_lines = self.select.render(ctx);
        self.select.set_focused(prev_focus);
        let options_len = self.select.options().len();
        let max_visible = self.select.max_visible_value();
        lines.extend(select_lines.lines);
        let mut padding = 0usize;
        if let Some(max_visible) = max_visible {
            if options_len < max_visible {
                padding = max_visible - options_len;
            }
            let footer_present = options_len > max_visible;
            if !footer_present {
                // Reserve space for the select footer line to keep height stable.
                padding += 1;
            }
        }

        // Show [NEW DIR] / [NEW FILE] only when not searching
        if !self.is_searching_current() {
            if let Some(new_entry) = self.new_entry_candidate() {
                if self.nav.entries.is_empty() {
                    let tag = if new_entry.is_dir {
                        "NEW DIR"
                    } else {
                        "NEW FILE"
                    };
                    let tag_style = Style::new().with_color(Color::Green).with_bold();
                    let name_style = Style::new().with_color(Color::Yellow);
                    lines.push(RenderLine {
                        spans: vec![
                            Span::new("[".to_string()),
                            Span::new(tag).with_style(tag_style),
                            Span::new("] "),
                            Span::new(new_entry.label).with_style(name_style),
                        ],
                    });
                }
            }
        }

        // Keep height stable: spinner consumes one padding row when available.
        let show_spinner = self.is_searching_current() && padding > 0;
        if show_spinner {
            padding = padding.saturating_sub(1);
        }
        for _ in 0..padding {
            lines.push(RenderLine {
                spans: vec![Span::new(" ").with_wrap(crate::ui::span::Wrap::No)],
            });
        }
        if show_spinner {
            let spinner = self.spinner_frame();
            let spinner_style = Style::new().with_color(Color::Cyan).with_bold();
            lines.push(RenderLine {
                spans: vec![
                    Span::new(spinner).with_style(spinner_style),
                    Span::new(" Searching..."),
                ],
            });
        }

        RenderOutput::from_lines(lines)
    }

    pub fn selected_value(&self) -> Option<Value> {
        self.selected_entry()
            .map(|entry| Value::Text(entry.path.to_string_lossy().to_string()))
    }

    pub fn set_value(&mut self, value: Value) {
        if let Value::Text(text) = value {
            self.input.set_value(text);
            self.refresh_view();
        }
    }

    pub fn handle_list_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        self.poll_scans();
        if modifiers != KeyModifiers::NONE {
            return ComponentResponse::not_handled();
        }

        match code {
            KeyCode::Up | KeyCode::Down | KeyCode::Char(' ') => {
                return self.select.handle_key(code, modifiers);
            }
            KeyCode::Right => {
                if let Some(entry) = self.selected_entry().cloned() {
                    if entry.is_dir {
                        self.enter_dir(&entry.path);
                        return ComponentResponse::handled();
                    }
                }
                return ComponentResponse::not_handled();
            }
            KeyCode::Left => {
                self.leave_dir();
                return ComponentResponse::handled();
            }
            KeyCode::Enter => {
                if self.nav.entries.is_empty() {
                    if let Some(new_entry) = self.new_entry_candidate() {
                        if new_entry.is_dir {
                            if fs::create_dir_all(&new_entry.path).is_ok() {
                                self.enter_dir(&new_entry.path);
                                return ComponentResponse::handled();
                            }
                        } else {
                            if let Some(parent) = new_entry.path.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            if fs::File::create(&new_entry.path).is_ok() {
                                return ComponentResponse::produced(Value::Text(
                                    new_entry.path.to_string_lossy().to_string(),
                                ));
                            }
                        }
                    }
                }
                if let Some(entry) = self.selected_entry().cloned() {
                    if entry.is_dir {
                        self.enter_dir(&entry.path);
                        return ComponentResponse::handled();
                    }
                }
                if let Some(value) = self.selected_value() {
                    return ComponentResponse::produced(value);
                }
                ComponentResponse::not_handled()
            }
            _ => ComponentResponse::not_handled(),
        }
    }

    pub fn handle_input_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> ComponentResponse {
        self.poll_scans();
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('h') => {
                    self.search.hide_hidden = !self.search.hide_hidden;
                    self.refresh_view();
                    return ComponentResponse::handled();
                }
                KeyCode::Char('f') => {
                    self.toggle_entry_filter(EntryFilter::FilesOnly);
                    return ComponentResponse::handled();
                }
                KeyCode::Char('d') => {
                    self.toggle_entry_filter(EntryFilter::DirsOnly);
                    return ComponentResponse::handled();
                }
                KeyCode::Char('g') => {
                    self.search.show_info = !self.search.show_info;
                    self.refresh_view();
                    return ComponentResponse::handled();
                }
                _ => {}
            }
        }
        if modifiers == KeyModifiers::NONE {
            match code {
                KeyCode::Tab => {
                    if !self.has_autocomplete_candidates() {
                        return ComponentResponse::not_handled();
                    }
                    let _ = self.apply_autocomplete();
                    return ComponentResponse::handled();
                }
                _ => {}
            }
        }

        let before = self.input.value();
        let result = self.input.handle_key(code, modifiers);
        let after = self.input.value();

        if before != after {
            // Set debounce timer instead of immediate refresh
            self.mark_input_changed();
            return ComponentResponse::handled();
        }

        match result {
            crate::inputs::KeyResult::Submit => ComponentResponse::submit_requested(),
            crate::inputs::KeyResult::Handled => ComponentResponse::handled(),
            crate::inputs::KeyResult::NotHandled => ComponentResponse::not_handled(),
        }
    }

    pub fn handle_combined_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> ComponentResponse {
        let list_response = self.handle_list_key(code, modifiers);
        if list_response.handled {
            return list_response;
        }
        self.handle_input_key(code, modifiers)
    }

    pub fn poll(&mut self) -> bool {
        let updated_scans = self.poll_scans();
        let updated_cache = self.apply_cached_search_if_ready();

        // Handle debounced input
        let mut debounce_triggered = false;
        if self
            .search
            .take_debounce_if_elapsed(Duration::from_millis(50))
        {
            self.refresh_view();
            debounce_triggered = true;
        }

        let updated_spinner = self.search.tick_spinner(self.is_searching_current());

        // Return true if debounce is pending to keep polling
        let debounce_pending = self.search.debounce_pending();
        updated_scans || updated_cache || updated_spinner || debounce_triggered || debounce_pending
    }

    pub fn delete_word(&mut self) -> ComponentResponse {
        let before = self.input.value();
        self.input.delete_word();
        let after = self.input.value();
        if before != after {
            self.mark_input_changed();
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }

    pub fn delete_word_forward(&mut self) -> ComponentResponse {
        let before = self.input.value();
        self.input.delete_word_forward();
        let after = self.input.value();
        if before != after {
            self.mark_input_changed();
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }
}
