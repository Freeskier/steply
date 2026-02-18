use std::path::Path;

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
    /// Highlight ranges (char indices) aligned with `entries`.
    pub highlights: Vec<Vec<(usize, usize)>>,
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
            highlights: Vec::new(),
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
        if let Some(&ei) = indices.get(m.index)
            && let Some(entry) = entries.get(ei)
        {
            matched_entries.push(entry.clone());
            matched_ranges.push(m.ranges.clone());
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
    let literals = glob_literal_chunks(pattern)
        .into_iter()
        .map(|s| s.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let primary_literal = literals.iter().max_by_key(|s| s.len()).cloned();

    let mut matched_entries: Vec<FileEntry> = entries
        .iter()
        .filter(|entry| {
            if !use_path
                && let Some(lit) = &primary_literal
                && !entry.name_lower.contains(lit)
            {
                return false;
            }

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

    let ranges: Vec<Vec<(usize, usize)>> = matched_entries
        .iter()
        .map(|e| literal_highlights(&literals, &e.name))
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
            entries.push(build_entry(name.clone(), path.clone(), is_dir));
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
        highlights: ranges,
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

    // Always include the user pattern as-is.
    builder.add(Glob::new(pattern).ok()?);

    // For patterns without an explicit path segment, also include a recursive
    // variant so `*.rs` matches both `main.rs` and `src/main.rs` when scanning
    // recursively.
    if !pattern.contains('/') {
        let recursive = format!("**/{pattern}");
        if let Ok(glob) = Glob::new(recursive.as_str()) {
            builder.add(glob);
        }
    }

    builder.build().ok()
}

/// Extract literal chunks from a glob pattern for highlight hints.
fn glob_literal_chunks(pattern: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_class = false;
    for ch in pattern.chars() {
        if in_class {
            if ch == ']' {
                in_class = false;
            }
            continue;
        }
        if ch == '[' {
            in_class = true;
            if !cur.is_empty() {
                chunks.push(cur.clone());
                cur.clear();
            }
            continue;
        }
        if matches!(ch, '*' | '?' | '{' | '}' | '/' | '\\') {
            if !cur.is_empty() {
                chunks.push(cur.clone());
                cur.clear();
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }

    chunks.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    chunks.dedup();
    chunks
}

fn literal_highlights(literals: &[String], name: &str) -> Vec<(usize, usize)> {
    if literals.is_empty() {
        return Vec::new();
    }
    let lower_name = name.to_ascii_lowercase();
    if lower_name.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    for lit in literals {
        if lit.is_empty() {
            continue;
        }
        let mut cursor = 0usize;
        while let Some(found) = lower_name[cursor..].find(lit.as_str()) {
            let start_byte = cursor + found;
            let end_byte = start_byte + lit.len();
            let start_char = lower_name[..start_byte].chars().count();
            let end_char = lower_name[..end_byte].chars().count();
            if end_char > start_char {
                ranges.push((start_char, end_char));
            }
            cursor = start_byte + 1;
        }
    }

    merge_ranges(ranges)
}

fn merge_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return ranges;
    }
    ranges.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut merged = Vec::with_capacity(ranges.len());
    let mut current = ranges[0];
    for (start, end) in ranges.into_iter().skip(1) {
        if start <= current.1 {
            current.1 = current.1.max(end);
        } else {
            merged.push(current);
            current = (start, end);
        }
    }
    merged.push(current);
    merged
}

// ── Path display helpers ─────────────────────────────────────────────────────

fn relative_prefix(path: &Path, root: &Path) -> Option<String> {
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
