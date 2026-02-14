#![allow(dead_code)]

use std::path::Path;
use std::time::{Duration, SystemTime};

use globset::{Glob, GlobSetBuilder};

use crate::core::search::fuzzy;
use crate::ui::style::{Color, Style};
use crate::widgets::components::select_list::SelectOption;

use super::DisplayMode;
use super::model::{FileEntry, build_entry, entry_sort};

const MAX_MATCHES: usize = 200;
const RELATIVE_PREFIX_MAX: usize = 24;

/// Processed result ready to feed into the SelectList and completion items.
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub entries: Vec<FileEntry>,
    pub options: Vec<SelectOption>,
    /// Plain names / relative paths for TextInput completion.
    pub completion_items: Vec<String>,
    /// Total matches before truncation (> entries.len() means results were cut off).
    pub total_matches: usize,
}

// ── Public search entry points ───────────────────────────────────────────────

/// Fuzzy-match `query` against `entries`, returning a `ScanResult`.
pub fn fuzzy_search(
    entries: &[FileEntry],
    query: &str,
    root: &Path,
    mode: DisplayMode,
) -> ScanResult {
    let query = query.trim();
    if query.is_empty() {
        return plain_result(entries, root, mode);
    }

    let indices = prefilter(entries, query).unwrap_or_else(|| (0..entries.len()).collect());
    if indices.is_empty() {
        return ScanResult {
            entries: Vec::new(),
            options: Vec::new(),
            completion_items: Vec::new(),
            total_matches: 0,
        };
    }

    let candidate_names: Vec<String> = indices.iter().map(|&i| entries[i].name.clone()).collect();

    let mut matches = fuzzy::ranked_matches(query, &candidate_names);
    let total_matches = matches.len();
    matches.truncate(MAX_MATCHES);

    let mut matched_entries: Vec<FileEntry> = Vec::with_capacity(matches.len());
    let mut matched_ranges: Vec<Vec<(usize, usize)>> = Vec::with_capacity(matches.len());

    for m in &matches {
        if let Some(&ei) = indices.get(m.index) {
            if let Some(entry) = entries.get(ei) {
                matched_entries.push(entry.clone());
                matched_ranges.push(m.ranges.clone());
            }
        }
    }

    build_result(matched_entries, matched_ranges, root, mode, total_matches)
}

/// Glob-match `pattern` against `entries`, returning a `ScanResult`.
pub fn glob_search(
    entries: &[FileEntry],
    pattern: &str,
    root: &Path,
    mode: DisplayMode,
) -> ScanResult {
    let matcher = build_glob_matcher(pattern);
    let use_path = pattern.contains('/');

    let mut matched_entries: Vec<FileEntry> = entries
        .iter()
        .filter(|entry| {
            let target = if use_path {
                entry
                    .path
                    .strip_prefix(root)
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| entry.name.clone())
            } else {
                entry.name.clone()
            };
            matcher.as_ref().is_some_and(|gs| gs.is_match(&target))
        })
        .cloned()
        .collect();

    let total_matches = matched_entries.len();
    matched_entries.truncate(MAX_MATCHES);
    matched_entries.sort_by(entry_sort);

    let literal = glob_literal_chunk(pattern);
    let ranges: Vec<Vec<(usize, usize)>> = matched_entries
        .iter()
        .map(|e| literal_highlights(&literal, &e.name))
        .collect();

    build_result(matched_entries, ranges, root, mode, total_matches)
}

/// Plain listing with no filter.
pub fn plain_result(entries: &[FileEntry], root: &Path, mode: DisplayMode) -> ScanResult {
    let total = entries.len();
    let truncated: Vec<FileEntry> = entries.iter().take(MAX_MATCHES).cloned().collect();
    let n = truncated.len();
    build_result(truncated, vec![vec![]; n], root, mode, total)
}

// ── Recursive glob scan ──────────────────────────────────────────────────────

/// Walk `dir` recursively, collecting entries whose relative path matches `pattern`.
pub fn list_dir_recursive_glob(dir: &Path, hide_hidden: bool, pattern: &str) -> Vec<FileEntry> {
    // Normalize `**.ext` → `**/*.ext`
    let normalized =
        if pattern.starts_with("**") && !pattern.starts_with("**/") && pattern.len() > 2 {
            format!("**/*{}", &pattern[2..])
        } else {
            pattern.to_string()
        };

    let matcher = build_glob_matcher(&normalized);
    let mut entries = Vec::new();
    walk_dir_recursive(dir, dir, hide_hidden, &matcher, &mut entries);
    entries.sort_by(entry_sort);
    entries
}

fn walk_dir_recursive(
    root: &Path,
    dir: &Path,
    hide_hidden: bool,
    matcher: &Option<globset::GlobSet>,
    entries: &mut Vec<FileEntry>,
) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if hide_hidden && name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let meta = entry.metadata().ok();

        // Compute relative path from root for matching
        let rel = path
            .strip_prefix(root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| name.clone());

        let matches = match matcher {
            Some(gs) => gs.is_match(&rel),
            None => false,
        };

        if matches {
            entries.push(build_entry(name.clone(), path.clone(), is_dir, meta));
        }

        if is_dir {
            walk_dir_recursive(root, &path, hide_hidden, matcher, entries);
        }
    }
}

// ── Internal build helpers ───────────────────────────────────────────────────

fn build_result(
    entries: Vec<FileEntry>,
    ranges: Vec<Vec<(usize, usize)>>,
    root: &Path,
    mode: DisplayMode,
    total_matches: usize,
) -> ScanResult {
    let options = entries
        .iter()
        .zip(ranges.iter())
        .map(|(entry, hl)| entry_option(entry, hl, root, mode))
        .collect();

    let completion_items = entries
        .iter()
        .map(|e| display_text(e, root, mode))
        .collect();

    ScanResult {
        total_matches,
        entries,
        options,
        completion_items,
    }
}

fn entry_option(
    entry: &FileEntry,
    highlights: &[(usize, usize)],
    root: &Path,
    mode: DisplayMode,
) -> SelectOption {
    let dir_style = Style::new().color(Color::Blue).bold();
    let prefix_style = Style::new().color(Color::DarkGrey);

    match mode {
        DisplayMode::Relative => {
            if let Some(prefix) = relative_prefix(&entry.path, root) {
                let name = entry.name.clone();
                let name_start = prefix.chars().count();
                let text = format!("{}{}", prefix, name);
                let name_style = if entry.is_dir {
                    dir_style
                } else {
                    Style::default()
                };
                return SelectOption::Split {
                    text,
                    name_start,
                    highlights: highlights.to_vec(),
                    prefix_style,
                    name_style,
                };
            }
        }
        DisplayMode::Full => {
            let full = entry.path.to_string_lossy().to_string();
            let name_start = full.len().saturating_sub(entry.name.len());
            let name_style = if entry.is_dir {
                dir_style
            } else {
                Style::default()
            };
            return SelectOption::Split {
                text: full,
                name_start,
                highlights: highlights.to_vec(),
                prefix_style,
                name_style,
            };
        }
        DisplayMode::Name => {}
    }

    // Name mode (or Relative with no prefix — file is at root level)
    if entry.is_dir {
        SelectOption::Styled {
            text: entry.name.clone(),
            highlights: highlights.to_vec(),
            style: dir_style,
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

/// The string used for completion items, matching what's shown in the list.
fn display_text(entry: &FileEntry, root: &Path, mode: DisplayMode) -> String {
    match mode {
        DisplayMode::Full => entry.path.to_string_lossy().to_string(),
        DisplayMode::Relative => entry
            .path
            .strip_prefix(root)
            .map(|r| r.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry.name.clone()),
        DisplayMode::Name => entry.name.clone(),
    }
}

// ── Glob helpers ─────────────────────────────────────────────────────────────

fn build_glob_matcher(pattern: &str) -> Option<globset::GlobSet> {
    let mut builder = GlobSetBuilder::new();
    // Add the pattern as-is; also add `**/pattern` so a bare `*.rs` matches in subdirs
    // when called from recursive walk (rel path is just the filename at depth 0).
    if let Ok(g) = Glob::new(pattern) {
        builder.add(g);
    } else {
        return None;
    }
    builder.build().ok()
}

/// Extract the longest literal chunk from a glob pattern for highlight hints.
fn glob_literal_chunk(pattern: &str) -> Option<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in pattern.chars() {
        if matches!(ch, '*' | '?' | '[' | ']') {
            if !cur.is_empty() {
                chunks.push(cur.clone());
                cur.clear();
            }
        } else if ch != '/' {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks.into_iter().max_by_key(|s| s.len())
}

fn literal_highlights(literal: &Option<String>, name: &str) -> Vec<(usize, usize)> {
    let Some(lit) = literal else {
        return Vec::new();
    };
    if lit.is_empty() {
        return Vec::new();
    }
    let name_chars: Vec<char> = name.chars().collect();
    let lit_chars: Vec<char> = lit.chars().collect();
    if lit_chars.len() > name_chars.len() {
        return Vec::new();
    }
    for start in 0..=(name_chars.len() - lit_chars.len()) {
        if name_chars[start..start + lit_chars.len()]
            .iter()
            .zip(lit_chars.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            return vec![(start, start + lit_chars.len())];
        }
    }
    Vec::new()
}

// ── Path display helpers ─────────────────────────────────────────────────────

fn relative_path_str(entry: &FileEntry, display_root: Option<&Path>) -> String {
    if let Some(root) = display_root {
        if let Ok(rel) = entry.path.strip_prefix(root) {
            return rel.to_string_lossy().replace('\\', "/");
        }
    }
    entry.name.clone()
}

fn relative_prefix(path: &std::path::PathBuf, root: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parent = rel.parent()?;
    let s = parent.to_string_lossy().to_string();
    if s.is_empty() || s == "." {
        return Some(String::new());
    }
    let mut display = s.replace('\\', "/");
    if !display.ends_with('/') {
        display.push('/');
    }
    Some(elide_middle(&display, RELATIVE_PREFIX_MAX))
}

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
    let tail: String = text
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}...{}", head, tail)
}

// ── Prefilter ────────────────────────────────────────────────────────────────

fn prefilter(entries: &[FileEntry], query: &str) -> Option<Vec<usize>> {
    if query.contains('/') || query.contains('\\') {
        return None;
    }
    let q = query.to_ascii_lowercase();
    if q.is_empty() {
        return None;
    }

    if q.starts_with('.') && q.len() > 1 {
        let v: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if e.name_lower.ends_with(&q) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        return if v.is_empty() { None } else { Some(v) };
    }

    if q.len() >= 2 {
        let qc: Vec<char> = q.chars().collect();
        let first = qc[0];
        let v: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if !e.name_lower.contains(first) {
                    return None;
                }
                let mut qi = 0;
                for ch in e.name_lower.chars() {
                    if qi >= qc.len() {
                        break;
                    }
                    if ch == qc[qi] {
                        qi += 1;
                    }
                }
                if qi == qc.len() { Some(i) } else { None }
            })
            .collect();
        return if v.is_empty() { None } else { Some(v) };
    }
    None
}

// ── Formatting helpers ───────────────────────────────────────────────────────

pub fn format_size(size: u64) -> String {
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

pub fn format_age(modified: SystemTime) -> String {
    let delta = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::ZERO);
    let secs = delta.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else if secs < 86400 * 30 {
        format!("{}d", secs / 86400)
    } else if secs < 86400 * 365 {
        format!("{}mo", secs / (86400 * 30))
    } else {
        format!("{}y", secs / (86400 * 365))
    }
}
