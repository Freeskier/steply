use crate::core::search::fuzzy;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub(crate) struct FileEntry {
    pub(crate) name: String,
    pub(crate) name_lower: String,
    pub(crate) ext_lower: Option<String>,
    pub(crate) path: Arc<PathBuf>,
    pub(crate) is_dir: bool,
    pub(crate) size: Option<u64>,
    pub(crate) modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub(crate) struct SearchResult {
    pub(crate) entries: Vec<FileEntry>,
    pub(crate) matches: Vec<fuzzy::FuzzyMatch>,
    pub(crate) display_root: Option<PathBuf>,
    pub(crate) show_relative: bool,
    pub(crate) show_info: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SearchMode {
    Fuzzy,
    Glob,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryFilter {
    All,
    FilesOnly,
    DirsOnly,
}

pub(crate) struct NewEntry {
    pub(crate) path: PathBuf,
    pub(crate) label: String,
    pub(crate) is_dir: bool,
}

pub(crate) fn entry_sort(a: &FileEntry, b: &FileEntry) -> Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name_lower.cmp(&b.name_lower),
    }
}

pub(crate) fn list_dir(dir: &Path, hide_hidden: bool) -> Vec<FileEntry> {
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

pub(crate) fn list_dir_recursive(dir: &Path, hide_hidden: bool) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    list_dir_recursive_inner(dir, &mut entries, hide_hidden);
    entries.sort_by(entry_sort);
    entries
}

pub(crate) fn list_dir_recursive_inner(
    dir: &Path,
    entries: &mut Vec<FileEntry>,
    hide_hidden: bool,
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
        entries.push(build_entry(name, path.clone(), is_dir, metadata));
        if is_dir {
            list_dir_recursive_inner(&path, entries, hide_hidden);
        }
    }
}

pub(crate) fn filter_entries(
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

pub(crate) fn normalize_ext(ext: &str) -> String {
    ext.trim_start_matches('.').to_ascii_lowercase()
}

pub(crate) fn build_entry(
    name: String,
    path: PathBuf,
    is_dir: bool,
    metadata: Option<fs::Metadata>,
) -> FileEntry {
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
        path: Arc::new(path),
        is_dir,
        size,
        modified,
    }
}
