#![allow(dead_code)]

use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::core::search::fuzzy;
use crate::ui::style::{Color, Style};
use crate::widgets::components::select_list::SelectOption;

use super::model::{FileEntry, entry_sort};
use super::parser::split_segments;

const MAX_MATCHES: usize = 5000;
const RELATIVE_PREFIX_MAX: usize = 24;

/// Processed result ready to feed into the SelectList and completion items.
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub entries: Vec<FileEntry>,
    pub options: Vec<SelectOption>,
    /// Plain names / relative paths for TextInput completion.
    pub completion_items: Vec<String>,
}

// ── Public search entry points ───────────────────────────────────────────────

/// Fuzzy-match `query` against `entries`, returning a `ScanResult`.
pub fn fuzzy_search(
    entries: &[FileEntry],
    query: &str,
    display_root: Option<&Path>,
    show_relative: bool,
) -> ScanResult {
    let query = query.trim();
    if query.is_empty() {
        return plain_result(entries, display_root, show_relative);
    }

    let indices = prefilter(entries, query).unwrap_or_else(|| (0..entries.len()).collect());
    if indices.is_empty() {
        return ScanResult {
            entries: Vec::new(),
            options: Vec::new(),
            completion_items: Vec::new(),
        };
    }

    let candidate_names: Vec<String> = indices.iter().map(|&i| entries[i].name.clone()).collect();

    let mut matches = fuzzy::ranked_matches(query, &candidate_names);
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

    build_result(matched_entries, matched_ranges, display_root, show_relative)
}

/// Glob-match `pattern` against `entries`, returning a `ScanResult`.
pub fn glob_search(
    entries: &[FileEntry],
    pattern: &str,
    display_root: Option<&Path>,
    show_relative: bool,
) -> ScanResult {
    let normalized = pattern.replace('\\', "/");
    let pattern_segments = split_segments(&normalized);
    let use_path = normalized.contains('/');
    let name_pattern = pattern_segments.last().map(String::as_str).unwrap_or("");
    let literal = longest_literal_chunk(name_pattern);

    let mut matched_entries: Vec<FileEntry> = entries
        .iter()
        .filter(|entry| {
            if let Some(lit) = &literal {
                if !entry.name.contains(lit.as_str()) {
                    return false;
                }
            }
            let target = if use_path {
                relative_path_str(entry, display_root)
            } else {
                entry.name.clone()
            };
            glob_match_path_segments(&pattern_segments, &target)
        })
        .cloned()
        .collect();

    sort_glob_results(&mut matched_entries, name_pattern, display_root);

    let ranges: Vec<Vec<(usize, usize)>> = matched_entries
        .iter()
        .map(|e| glob_highlights(name_pattern, &e.name))
        .collect();

    build_result(matched_entries, ranges, display_root, show_relative)
}

/// Plain listing with no filter.
pub fn plain_result(
    entries: &[FileEntry],
    display_root: Option<&Path>,
    show_relative: bool,
) -> ScanResult {
    build_result(
        entries.to_vec(),
        vec![vec![]; entries.len()],
        display_root,
        show_relative,
    )
}

// ── Internal build helpers ───────────────────────────────────────────────────

fn build_result(
    entries: Vec<FileEntry>,
    ranges: Vec<Vec<(usize, usize)>>,
    display_root: Option<&Path>,
    show_relative: bool,
) -> ScanResult {
    let options = entries
        .iter()
        .zip(ranges.iter())
        .map(|(entry, hl)| entry_option(entry, hl, display_root, show_relative))
        .collect();

    let completion_items = entries
        .iter()
        .map(|e| {
            if show_relative {
                if let Some(root) = display_root {
                    if let Ok(rel) = e.path.strip_prefix(root) {
                        return rel.to_string_lossy().to_string();
                    }
                }
            }
            e.name.clone()
        })
        .collect();

    ScanResult {
        entries,
        options,
        completion_items,
    }
}

/// Build a `SelectOption` for a single entry with optional fuzzy/glob highlights.
fn entry_option(
    entry: &FileEntry,
    highlights: &[(usize, usize)],
    display_root: Option<&Path>,
    show_relative: bool,
) -> SelectOption {
    let dir_style = Style::new().color(Color::Blue).bold();
    let prefix_style = Style::new().color(Color::DarkGrey);

    // Relative path prefix (e.g. "src/foo/")
    if show_relative {
        if let Some(root) = display_root {
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
    }

    // No prefix — plain name, optionally styled for dirs
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

// ── Glob helpers (ported from legacy) ───────────────────────────────────────

pub fn glob_match_path_segments(pattern_segments: &[String], target: &str) -> bool {
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

fn glob_match_segment(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut pi = 0;
    let mut ti = 0;
    let mut star_idx: Option<usize> = None;
    let mut match_idx = 0;
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

fn sort_glob_results(
    entries: &mut Vec<FileEntry>,
    name_pattern: &str,
    display_root: Option<&Path>,
) {
    let scores: Vec<usize> = entries
        .iter()
        .map(|e| glob_score(name_pattern, &e.name))
        .collect();
    let depths: Vec<usize> = entries
        .iter()
        .map(|e| glob_depth(e, display_root))
        .collect();
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    indices.sort_by(|&a, &b| {
        scores[b]
            .cmp(&scores[a])
            .then(depths[a].cmp(&depths[b]))
            .then(entries[a].name_lower.cmp(&entries[b].name_lower))
    });
    let sorted: Vec<FileEntry> = indices.iter().map(|&i| entries[i].clone()).collect();
    *entries = sorted;
}

fn glob_score(pattern: &str, name: &str) -> usize {
    glob_highlights(pattern, name)
        .iter()
        .map(|(s, e)| e.saturating_sub(*s))
        .sum()
}

fn glob_depth(entry: &FileEntry, display_root: Option<&Path>) -> usize {
    let rel = relative_path_str(entry, display_root);
    split_segments(&rel).len()
}

fn glob_highlights(pattern: &str, name: &str) -> Vec<(usize, usize)> {
    let literals = glob_literal_chunks(pattern);
    let best = literals
        .into_iter()
        .max_by_key(|s| s.len())
        .unwrap_or_default();
    if best.is_empty() {
        return Vec::new();
    }
    let name_chars: Vec<char> = name.chars().collect();
    let best_chars: Vec<char> = best.chars().collect();
    if best_chars.len() > name_chars.len() {
        return Vec::new();
    }
    for start in 0..=(name_chars.len() - best_chars.len()) {
        if name_chars[start..start + best_chars.len()] == best_chars[..] {
            return vec![(start, start + best_chars.len())];
        }
    }
    Vec::new()
}

fn glob_literal_chunks(pattern: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut cur = String::new();
    for ch in pattern.chars() {
        if ch == '*' || ch == '?' {
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
    chunks
}

fn longest_literal_chunk(pattern: &str) -> Option<String> {
    glob_literal_chunks(pattern)
        .into_iter()
        .max_by_key(|s| s.len())
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

// ── Prefilter (fast candidate reduction before fuzzy scoring) ────────────────

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

// ── Recursive glob scan ──────────────────────────────────────────────────────

pub fn list_dir_recursive_glob(
    dir: &std::path::Path,
    hide_hidden: bool,
    pattern: &str,
) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    let normalized = pattern.replace('\\', "/");
    // Normalize `**.ext` → `**/*.ext` so `**` is always its own segment
    let normalized =
        if normalized.starts_with("**") && !normalized.starts_with("**/") && normalized.len() > 2 {
            format!("**/*{}", &normalized[2..])
        } else {
            normalized
        };
    let pattern_segments = split_segments(&normalized);
    let name_pattern = pattern_segments.last().map(String::as_str).unwrap_or("");
    let literal = longest_literal_chunk(name_pattern);
    // `**` can appear as its own segment OR embedded (e.g. `foo/**bar`)
    let has_double_star = pattern_segments.iter().any(|s| s.contains("**"));
    let mut rel = Vec::new();
    list_dir_recursive_glob_inner(
        dir,
        &pattern_segments,
        hide_hidden,
        literal.as_deref(),
        has_double_star,
        &mut rel,
        &mut entries,
    );
    entries.sort_by(entry_sort);
    entries
}

fn list_dir_recursive_glob_inner(
    dir: &std::path::Path,
    pattern_segments: &[String],
    hide_hidden: bool,
    literal: Option<&str>,
    has_double_star: bool,
    rel_segments: &mut Vec<String>,
    entries: &mut Vec<FileEntry>,
) {
    use super::model::build_entry;
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        if hide_hidden && name.starts_with('.') {
            continue;
        }
        let meta = entry.metadata().ok();
        rel_segments.push(name.clone());

        if !has_double_star && rel_segments.len() > pattern_segments.len() {
            rel_segments.pop();
            continue;
        }

        let literal_ok = literal.map(|lit| name.contains(lit)).unwrap_or(true);
        if literal_ok && glob_match_segments(pattern_segments, rel_segments) {
            entries.push(build_entry(name, path.clone(), is_dir, meta));
        }
        if is_dir {
            list_dir_recursive_glob_inner(
                &path,
                pattern_segments,
                hide_hidden,
                literal,
                has_double_star,
                rel_segments,
                entries,
            );
        }
        rel_segments.pop();
    }
}
