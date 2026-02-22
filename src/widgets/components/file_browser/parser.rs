use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ParsedInput {
    pub view_dir: PathBuf,
    pub query: String,
    pub mode: QueryMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    Fuzzy,
    FuzzyRecursive,
    Glob,
}

impl QueryMode {
    pub fn is_glob(self) -> bool {
        matches!(self, Self::Glob)
    }

    pub fn recursive(self, default_recursive: bool, query: &str) -> bool {
        match self {
            Self::FuzzyRecursive => true,
            Self::Glob => default_recursive || query.contains("**"),
            Self::Fuzzy => default_recursive,
        }
    }
}

pub fn parse_input(raw: &str, cwd: &Path) -> ParsedInput {
    let expanded = expand_home(raw);
    let normalized = expanded.replace('\\', "/");

    let (dir_part, raw_query) = split_dir_query(&normalized);
    let (mode, query) = classify_query(raw_query.as_str());

    let base = if dir_part.is_empty() {
        cwd.to_path_buf()
    } else {
        let p = PathBuf::from(dir_part);
        if p.is_absolute() {
            p
        } else {
            cwd.join(p)
        }
    };

    let view_dir = normalize_path(&base);

    ParsedInput {
        view_dir,
        query,
        mode,
    }
}

fn expand_home(path: &str) -> String {
    if (path == "~" || path.starts_with("~/") || path.starts_with("~\\"))
        && let Some(home) = home_dir()
    {
        let rest = &path[1..];
        return format!("{}{}", home.to_string_lossy(), rest);
    }
    path.to_string()
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn split_dir_query(path: &str) -> (&str, String) {
    let double_star_pos = path.find("**");

    let split_pos = match double_star_pos {
        Some(ds) => path[..ds].rfind('/'),
        None => path.rfind('/'),
    };

    match split_pos {
        Some(pos) => (&path[..=pos], path[pos + 1..].to_string()),
        None => ("", path.to_string()),
    }
}

fn classify_query(raw_query: &str) -> (QueryMode, String) {
    if let Some(suffix) = raw_query.strip_prefix("**")
        && !suffix.is_empty()
        && !suffix.starts_with('/')
        && !suffix.contains('/')
        && !contains_glob_meta(suffix)
    {
        return (QueryMode::FuzzyRecursive, suffix.to_string());
    }

    if is_glob_query(raw_query) {
        return (QueryMode::Glob, raw_query.to_string());
    }

    (
        QueryMode::Fuzzy,
        raw_query.trim_matches('*').trim_matches('?').to_string(),
    )
}

fn is_glob_query(query: &str) -> bool {
    contains_glob_meta(query)
        && !matches!(query.trim_matches('*').trim_matches('?'), "" | "." | "..")
}

fn contains_glob_meta(query: &str) -> bool {
    query
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
}
