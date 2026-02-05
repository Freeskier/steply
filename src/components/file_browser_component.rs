use crate::components::select_component::{SelectComponent, SelectMode, SelectOption};
use crate::core::component::{Component, ComponentBase, EventContext, FocusMode};
use crate::core::search::fuzzy;
use crate::core::value::Value;
use crate::inputs::Input;
use crate::inputs::text_input::TextInput;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine};
use crate::ui::style::{Color, Style};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

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
    cache: HashMap<String, SearchResult>,
    scan_tx: Sender<(String, SearchResult)>,
    scan_rx: Receiver<(String, SearchResult)>,
}

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

#[derive(Debug, Clone)]
struct SearchResult {
    entries: Vec<FileEntry>,
    options: Vec<SelectOption>,
    matches: Vec<fuzzy::FuzzyMatch>,
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
            cache: HashMap::new(),
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
        let parsed = parse_input(&raw, &self.current_dir);

        self.view_dir = parsed.view_dir.clone();

        let (entries, options, matches) = if let Some(query) = parsed.query {
            if self.recursive_search {
                if let Some(result) = self.search_async(&parsed.view_dir, true, &query) {
                    (result.entries, result.options, result.matches)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else {
                let entries = list_dir(&parsed.view_dir, self.hide_hidden);
                let (options, matches) = fuzzy_options(&entries, &query);
                (entries, options, matches)
            }
        } else if parsed.path_mode {
            let mut entries = list_dir(&parsed.view_dir, self.hide_hidden);
            if !parsed.segment.is_empty() {
                entries.retain(|entry| entry.name.starts_with(&parsed.segment));
            }
            let options = entries
                .iter()
                .map(|entry| entry_option(entry, &[]))
                .collect::<Vec<_>>();
            (entries, options, Vec::new())
        } else if raw.trim().is_empty() {
            let entries = list_dir(&self.current_dir, self.hide_hidden);
            let options = entries
                .iter()
                .map(|entry| entry_option(entry, &[]))
                .collect::<Vec<_>>();
            (entries, options, Vec::new())
        } else {
            let query = raw.trim().to_string();
            if self.recursive_search {
                let current_dir = self.current_dir.clone();
                if let Some(result) = self.search_async(&current_dir, true, &query) {
                    (result.entries, result.options, result.matches)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else {
                let entries = list_dir(&self.current_dir, self.hide_hidden);
                let (options, matches) = fuzzy_options(&entries, &query);
                (entries, options, matches)
            }
        };

        self.entries = entries;
        self.matches = matches;
        self.select.set_options(options);
        self.select.reset_active();
    }

    fn poll_scans(&mut self) {
        for (key, result) in self.scan_rx.try_iter() {
            self.cache.insert(key, result);
        }
    }

    fn search_async(&mut self, dir: &Path, recursive: bool, query: &str) -> Option<SearchResult> {
        let key = cache_key(dir, recursive, self.hide_hidden, query);
        if let Some(result) = self.cache.get(&key) {
            return Some(result.clone());
        }

        let dir = dir.to_path_buf();
        let hide_hidden = self.hide_hidden;
        let query = query.to_string();
        let tx = self.scan_tx.clone();
        thread::spawn(move || {
            let entries = if recursive {
                list_dir_recursive(&dir, hide_hidden)
            } else {
                list_dir(&dir, hide_hidden)
            };
            let (options, matches) = fuzzy_options(&entries, &query);
            let result = SearchResult {
                entries,
                options,
                matches,
            };
            let _ = tx.send((key, result));
        });

        None
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
        if !parsed.path_mode || parsed.query.is_some() {
            return false;
        }

        let entries = list_dir(&parsed.view_dir, self.hide_hidden);
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
        if !parsed.path_mode || parsed.query.is_some() {
            return false;
        }

        let entries = list_dir(&parsed.view_dir, self.hide_hidden);
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
    query: Option<String>,
    ends_with_slash: bool,
    dir_prefix: String,
}

fn parse_input(raw: &str, current_dir: &Path) -> ParsedInput {
    let raw = raw.to_string();
    let trimmed = raw.trim();
    let mut query = None;
    let mut path_part = trimmed;

    if let Some(idx) = trimmed.find(':') {
        path_part = &trimmed[..idx];
        query = Some(trimmed[idx + 1..].to_string());
    }

    let path_mode =
        path_part.starts_with('~') || path_part.starts_with('/') || path_part.starts_with('.');

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
        query,
        ends_with_slash,
        dir_prefix,
    }
}

fn split_path(path: &str) -> (String, String) {
    if path.is_empty() {
        return (String::new(), String::new());
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
            entries.push(FileEntry { name, path, is_dir });
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
        entries.push(FileEntry {
            name: name.clone(),
            path: path.clone(),
            is_dir,
        });
        if is_dir {
            list_dir_recursive_inner(&path, entries, hide_hidden);
        }
    }
}

fn entry_sort(a: &FileEntry, b: &FileEntry) -> Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    }
}

fn fuzzy_options(
    entries: &[FileEntry],
    query: &str,
) -> (Vec<SelectOption>, Vec<fuzzy::FuzzyMatch>) {
    let names = entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect::<Vec<_>>();
    let mut matches = fuzzy::match_candidates(query, &names);

    matches.sort_by(|a, b| {
        let a_entry = entries.get(a.index);
        let b_entry = entries.get(b.index);
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

    let mut options = Vec::with_capacity(matches.len());
    for m in &matches {
        if let Some(entry) = entries.get(m.index) {
            options.push(entry_option(entry, &m.ranges));
        }
    }

    (options, matches)
}

fn entry_option(entry: &FileEntry, highlights: &[(usize, usize)]) -> SelectOption {
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

fn rebuild_path(parsed: &ParsedInput, segment: &str) -> String {
    let mut base = parsed.dir_prefix.clone();
    base.push_str(segment);
    if let Some(query) = &parsed.query {
        base.push(':');
        base.push_str(query);
    }
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

fn cache_key(dir: &Path, recursive: bool, hide_hidden: bool, query: &str) -> String {
    format!(
        "{}|r:{}|h:{}|q:{}",
        dir.to_string_lossy(),
        recursive,
        hide_hidden,
        query
    )
}
