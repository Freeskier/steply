use std::path::{Path, PathBuf};

/// Result of parsing the raw text-input value.
#[derive(Debug, Clone)]
pub struct ParsedInput {
    /// Directory to scan / display entries from.
    pub view_dir: PathBuf,
    /// The search/filter segment (last path component being typed).
    pub query: String,
    /// Whether the input looks like a glob pattern.
    pub is_glob: bool,
}

/// Parse a raw input string into a view directory + search segment.
pub fn parse_input(raw: &str, cwd: &Path) -> ParsedInput {
    let expanded = expand_home(raw);
    let normalized = expanded.replace('\\', "/");

    // Detect glob — bare `**` or `*` alone is not treated as glob (no useful pattern)
    let query_part = if let Some(pos) = normalized.rfind('/') {
        &normalized[pos + 1..]
    } else {
        normalized.as_str()
    };
    let is_glob = (normalized.contains('*') || normalized.contains('?'))
        && !matches!(query_part.trim_matches('*'), "" | "." | "..");

    // Split on the last '/' that is NOT inside a `**/` glob segment.
    // For `src/**/*.rs` we want dir=`src/` and query=`**/*.rs`.
    let (dir_part, raw_query) = split_dir_query(&normalized);

    // If not a glob, strip any bare wildcards from query so fuzzy search gets clean input
    let query = if !is_glob {
        raw_query.trim_matches('*').trim_matches('?').to_string()
    } else {
        raw_query
    };

    let base = if dir_part.is_empty() {
        cwd.to_path_buf()
    } else {
        let p = PathBuf::from(dir_part);
        if p.is_absolute() { p } else { cwd.join(p) }
    };

    let view_dir = normalize_path(&base);

    ParsedInput {
        view_dir,
        query,
        is_glob,
    }
}

/// Expand `~` at the start of a path to the home directory.
fn expand_home(path: &str) -> String {
    if path == "~" || path.starts_with("~/") || path.starts_with("~\\") {
        if let Some(home) = home_dir() {
            let rest = &path[1..];
            return format!("{}{}", home.to_string_lossy(), rest);
        }
    }
    path.to_string()
}

/// Resolve `.` and `..` components without requiring the path to exist.
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

/// Split `path` into `(dir_part, query)` at the last `/` that precedes any `**` segment.
/// Examples:
///   `"src/**/*.rs"` → `("src/", "**/*.rs")`
///   `"/home/user/Doc"` → `("/home/user/", "Doc")`
///   `"foo"` → `("", "foo")`
fn split_dir_query(path: &str) -> (&str, String) {
    // Find the position of the first `**` — everything from there on is the query.
    let double_star_pos = path.find("**");

    // Candidate: last `/` before `**` (or last `/` overall if no `**`)
    let split_pos = match double_star_pos {
        Some(ds) => path[..ds].rfind('/'),
        None => path.rfind('/'),
    };

    match split_pos {
        Some(pos) => (&path[..=pos], path[pos + 1..].to_string()),
        None => ("", path.to_string()),
    }
}
