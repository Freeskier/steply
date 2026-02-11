use std::path::{Path, PathBuf};

pub(crate) struct ParsedInput {
    pub(crate) path_mode: bool,
    pub(crate) view_dir: PathBuf,
    pub(crate) segment: String,
    pub(crate) ends_with_slash: bool,
    pub(crate) dir_prefix: String,
}

pub(crate) fn parse_input(raw: &str, current_dir: &Path) -> ParsedInput {
    let raw = raw.to_string();
    let trimmed = raw.trim();
    let path_part = trimmed;

    let path_mode = is_path_mode(path_part);

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

pub(crate) fn normalize_input(raw: &str, _current_dir: &Path) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return raw.to_string();
    }
    let path_part = trimmed;

    let path_mode = is_path_mode(path_part);

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

    let normalized_path = normalize_path_part(path_part);
    if normalized_path == path_part {
        return raw.to_string();
    }
    normalized_path
}

pub(crate) fn is_path_mode(path_part: &str) -> bool {
    path_part.starts_with('~')
        || path_part.starts_with('/')
        || path_part.starts_with("./")
        || path_part.starts_with("../")
        || path_part.starts_with(".\\")
        || path_part.starts_with("..\\")
}

pub(crate) fn split_path(path: &str) -> (String, String) {
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

pub(crate) fn resolve_path(path: &str, current_dir: &Path) -> PathBuf {
    if path.starts_with('~') {
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
    }
}

pub(crate) fn normalize_path_part(path_part: &str) -> String {
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
        format!("/{normalized}")
    } else if had_dot_prefix && !normalized.starts_with("..") && !normalized.is_empty() {
        format!("./{normalized}")
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

pub(crate) fn normalize_absolute_components(path: &str) -> String {
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

pub(crate) fn normalize_relative_components(path: &str) -> String {
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

pub(crate) fn is_only_dot_segments(path: &str) -> bool {
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

pub(crate) fn split_segments(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

pub(crate) fn is_glob_query(query: &str) -> bool {
    query.contains('*') || query.contains('?')
}

pub(crate) fn is_recursive_glob(pattern: &str) -> bool {
    pattern.contains("**") || pattern.contains('/')
}

pub(crate) fn split_glob_path(path_part: &str) -> Option<(String, String)> {
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

pub(crate) fn strip_recursive_fuzzy(query: &str) -> Option<String> {
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

pub(crate) fn strip_recursive_fuzzy_segment(segment: &str) -> Option<String> {
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

pub(crate) fn rebuild_path(parsed: &ParsedInput, segment: &str) -> String {
    let mut base = parsed.dir_prefix.clone();
    base.push_str(segment);
    base
}

pub(crate) fn path_to_string(path: &Path) -> String {
    let mut text = path.to_string_lossy().to_string();
    if !text.ends_with('/') {
        text.push('/');
    }
    text
}
