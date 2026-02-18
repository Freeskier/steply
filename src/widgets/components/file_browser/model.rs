use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    File,
    SymlinkDir,
    SymlinkFile,
}

impl EntryKind {
    pub fn is_dir(self) -> bool {
        matches!(self, Self::Dir | Self::SymlinkDir)
    }

    pub fn is_symlink(self) -> bool {
        matches!(self, Self::SymlinkDir | Self::SymlinkFile)
    }

    pub fn should_recurse(self) -> bool {
        matches!(self, Self::Dir)
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub name_lower: String,
    pub ext_lower: Option<String>,
    pub path: Arc<PathBuf>,
    pub kind: EntryKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryFilter {
    All,
    FilesOnly,
    DirsOnly,
}

pub fn build_entry(name: String, path: PathBuf, kind: EntryKind) -> FileEntry {
    let name_lower = name.to_ascii_lowercase();
    let ext_lower = if kind.is_dir() {
        None
    } else {
        name.rsplit_once('.')
            .map(|(_, ext)| ext.trim_start_matches('.').to_ascii_lowercase())
            .filter(|ext| !ext.is_empty())
    };
    FileEntry {
        name,
        name_lower,
        ext_lower,
        path: Arc::new(path),
        kind,
    }
}

pub fn list_dir(dir: &Path, hide_hidden: bool) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            let kind = classify_entry_kind(&entry);
            let name = entry.file_name().to_string_lossy().to_string();
            if hide_hidden && name.starts_with('.') {
                continue;
            }
            entries.push(build_entry(name, path, kind));
        }
    }
    sort_entries(&mut entries);
    entries
}

pub fn list_dir_recursive(dir: &Path, hide_hidden: bool) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    list_dir_recursive_inner(dir, &mut entries, hide_hidden);
    sort_entries(&mut entries);
    entries
}

fn list_dir_recursive_inner(dir: &Path, entries: &mut Vec<FileEntry>, hide_hidden: bool) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        let kind = classify_entry_kind(&entry);
        let name = entry.file_name().to_string_lossy().to_string();
        if hide_hidden && name.starts_with('.') {
            continue;
        }
        entries.push(build_entry(name, path.clone(), kind));
        if kind.should_recurse() {
            list_dir_recursive_inner(&path, entries, hide_hidden);
        }
    }
}

pub fn filter_entries(
    mut entries: Vec<FileEntry>,
    entry_filter: EntryFilter,
    ext_filter: Option<&HashSet<String>>,
) -> Vec<FileEntry> {
    entries.retain(|e| match entry_filter {
        EntryFilter::All => true,
        EntryFilter::FilesOnly => !e.kind.is_dir(),
        EntryFilter::DirsOnly => e.kind.is_dir(),
    });
    if let Some(exts) = ext_filter {
        entries.retain(|e| {
            e.kind.is_dir()
                || e.ext_lower
                    .as_ref()
                    .map(|ext| exts.contains(ext))
                    .unwrap_or(false)
        });
    }
    entries
}

pub fn entry_sort(a: &FileEntry, b: &FileEntry) -> std::cmp::Ordering {
    match (a.kind.is_dir(), b.kind.is_dir()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name_lower.cmp(&b.name_lower),
    }
}

pub fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(entry_sort);
}

pub fn completion_item_label(entry: &FileEntry) -> String {
    if entry.kind.is_dir() {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    }
}

pub fn classify_entry_kind(entry: &fs::DirEntry) -> EntryKind {
    let Ok(ft) = entry.file_type() else {
        return EntryKind::File;
    };
    if ft.is_symlink() {
        // Follow symlink target to preserve dir/file behavior in browser logic.
        let target_is_dir = fs::metadata(entry.path())
            .map(|m| m.is_dir())
            .unwrap_or(false);
        if target_is_dir {
            EntryKind::SymlinkDir
        } else {
            EntryKind::SymlinkFile
        }
    } else if ft.is_dir() {
        EntryKind::Dir
    } else {
        EntryKind::File
    }
}
