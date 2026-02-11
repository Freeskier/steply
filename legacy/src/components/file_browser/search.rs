use crate::components::select_component::SelectOption;
use crate::core::search::fuzzy;
use crate::ui::style::{Color, Style};
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::model::{FileEntry, entry_sort};
use super::parser::split_segments;

const MAX_MATCHES: usize = 5000;
const QUICK_MATCH_THRESHOLD: usize = 10000;
const RELATIVE_PREFIX_MAX: usize = 24;

pub(crate) fn options_from_query(
    entries: &[FileEntry],
    query: &str,
    display_root: Option<&Path>,
    show_relative: bool,
    show_info: bool,
) -> (Vec<FileEntry>, Vec<SelectOption>, Vec<fuzzy::FuzzyMatch>) {
    let query = query.trim();
    let max_name_width = compute_max_name_width(entries, show_info);

    if query.is_empty() {
        let options = entries
            .iter()
            .map(|entry| {
                entry_option(
                    entry,
                    &[],
                    display_root,
                    show_relative,
                    show_info,
                    max_name_width,
                )
            })
            .collect::<Vec<_>>();
        return (entries.to_vec(), options, Vec::new());
    }

    let indices: Vec<usize> = if let Some(filtered) = prefilter_entries(entries, query) {
        filtered
    } else {
        (0..entries.len()).collect()
    };

    if indices.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let candidate_names: Vec<String> = indices
        .iter()
        .map(|&idx| entries[idx].name.clone())
        .collect();

    let mut matches = if indices.len() > QUICK_MATCH_THRESHOLD {
        fuzzy::match_candidates_top(query, &candidate_names, MAX_MATCHES)
    } else if indices.len() > MAX_MATCHES * 2 {
        fuzzy::match_candidates_top(query, &candidate_names, MAX_MATCHES)
    } else {
        fuzzy::match_candidates(query, &candidate_names)
    };

    // Sort by score (fuzzy/glob results should prioritize match quality)
    matches.sort_unstable_by(|a, b| b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index)));

    if matches.len() > MAX_MATCHES {
        matches.truncate(MAX_MATCHES);
    }

    let capacity = matches.len();
    let mut matched_entries = Vec::with_capacity(capacity);
    let mut temp_matches = Vec::with_capacity(capacity);

    for (pos, m) in matches.into_iter().enumerate() {
        if let Some(&entry_idx) = indices.get(m.index) {
            if let Some(entry) = entries.get(entry_idx) {
                matched_entries.push(entry.clone());
                temp_matches.push((
                    m.ranges.clone(),
                    fuzzy::FuzzyMatch {
                        index: pos,
                        score: m.score,
                        matched_indices: m.matched_indices,
                        ranges: m.ranges,
                    },
                ));
            }
        }
    }

    let max_name_width = compute_max_name_width(&matched_entries, show_info);

    let mut options = Vec::with_capacity(matched_entries.len());
    let mut adjusted = Vec::with_capacity(matched_entries.len());

    for (entry, (ranges, adj_match)) in matched_entries.iter().zip(temp_matches.into_iter()) {
        options.push(entry_option(
            entry,
            &ranges,
            display_root,
            show_relative,
            show_info,
            max_name_width,
        ));
        adjusted.push(adj_match);
    }

    (matched_entries, options, adjusted)
}

pub(crate) fn glob_options(
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

pub(crate) fn build_glob_options(
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
        let score_a = glob_score(name_pattern, &ea.name);
        let score_b = glob_score(name_pattern, &eb.name);
        let depth_a = glob_depth(ea, display_root).unwrap_or(usize::MAX);
        let depth_b = glob_depth(eb, display_root).unwrap_or(usize::MAX);

        score_b
            .cmp(&score_a)
            .then_with(|| depth_a.cmp(&depth_b))
            .then_with(|| ea.name_lower.cmp(&eb.name_lower))
    });

    let mut sorted_entries = Vec::with_capacity(entries.len());
    let mut sorted_options = Vec::with_capacity(entries.len());

    let max_name_width = compute_max_name_width(entries, show_info);

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
            max_name_width,
        ));
    }
    *entries = sorted_entries;
    sorted_options
}

pub(crate) fn build_options(
    entries: &[FileEntry],
    matches: Option<&[fuzzy::FuzzyMatch]>,
    display_root: Option<&Path>,
    show_relative: bool,
    show_info: bool,
) -> Vec<SelectOption> {
    let max_name_width = compute_max_name_width(entries, show_info);
    if let Some(matches) = matches {
        if !matches.is_empty() {
            return entries
                .iter()
                .zip(matches.iter())
                .map(|(entry, m)| {
                    entry_option(
                        entry,
                        &m.ranges,
                        display_root,
                        show_relative,
                        show_info,
                        max_name_width,
                    )
                })
                .collect();
        }
    }

    entries
        .iter()
        .map(|entry| {
            entry_option(
                entry,
                &[],
                display_root,
                show_relative,
                show_info,
                max_name_width,
            )
        })
        .collect()
}

pub(crate) fn longest_common_prefix(entries: &[FileEntry], prefix: &str) -> String {
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

pub(crate) fn list_dir_recursive_glob(
    dir: &Path,
    hide_hidden: bool,
    pattern: &str,
) -> Vec<FileEntry> {
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
    let Ok(read_dir) = std::fs::read_dir(dir) else {
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
            entries.push(super::model::build_entry(
                name,
                path.clone(),
                is_dir,
                metadata,
            ));
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

fn glob_prefix_len(pattern_segments: &[String]) -> usize {
    for (idx, segment) in pattern_segments.iter().enumerate() {
        if segment == "**" {
            return idx;
        }
    }
    pattern_segments.len()
}

fn relative_path_for_match(entry: &FileEntry, display_root: Option<&Path>) -> String {
    let path = if let Some(root) = display_root {
        entry.path.strip_prefix(root).unwrap_or(&entry.path)
    } else {
        &entry.path
    };
    path.to_string_lossy().replace('\\', "/")
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

fn glob_score(pattern: &str, name: &str) -> usize {
    let highlights = glob_highlights(pattern, name);
    highlights
        .iter()
        .map(|(start, end)| end.saturating_sub(*start))
        .sum()
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

    let q = query.to_ascii_lowercase();

    if q.is_empty() {
        return None;
    }

    if q.starts_with('.') && q.len() > 1 {
        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                if entry.name_lower.ends_with(&q) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        return if filtered.is_empty() {
            None
        } else {
            Some(filtered)
        };
    }

    if q.len() >= 2 {
        let query_chars: Vec<char> = q.chars().collect();
        let first_char = query_chars[0];

        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                if !entry.name_lower.contains(first_char) {
                    return None;
                }

                let mut q_idx = 0;
                for ch in entry.name_lower.chars() {
                    if q_idx >= query_chars.len() {
                        break;
                    }
                    if ch == query_chars[q_idx] {
                        q_idx += 1;
                    }
                }

                if q_idx == query_chars.len() {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        return if filtered.is_empty() {
            None
        } else {
            Some(filtered)
        };
    }

    None
}

pub(crate) fn compute_max_name_width(entries: &[FileEntry], show_info: bool) -> usize {
    if !show_info {
        return 0;
    }
    entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(0)
}

pub(crate) fn entry_option(
    entry: &FileEntry,
    highlights: &[(usize, usize)],
    display_root: Option<&Path>,
    show_relative: bool,
    show_info: bool,
    max_name_width: usize,
) -> SelectOption {
    let suffix = if show_info {
        let padding_needed = max_name_width.saturating_sub(entry.name.chars().count());
        let padding = " ".repeat(padding_needed);
        entry_info_suffix(entry).map(|info| format!("{}  {}", padding, info))
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

    let type_str = if entry.is_dir {
        "DIR".to_string()
    } else if let Some(ext) = &entry.ext_lower {
        if ext.len() <= 5 {
            ext.to_uppercase()
        } else {
            ext[..5].to_uppercase()
        }
    } else {
        "FILE".to_string()
    };
    parts.push(format!("{:>5}", type_str));

    if !entry.is_dir {
        if let Some(size) = entry.size {
            parts.push(format!("{:>8}", format_size(size)));
        } else {
            parts.push(format!("{:>8}", "-"));
        }
    } else {
        parts.push(format!("{:>8}", "-"));
    }

    if let Some(modified) = entry.modified {
        parts.push(format!("{:>7}", format_age(modified)));
    } else {
        parts.push(format!("{:>7}", "-"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("  {}", parts.join("  ")))
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
