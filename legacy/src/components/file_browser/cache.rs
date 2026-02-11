use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::model::{EntryFilter, SearchMode};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SearchKey {
    pub(crate) dir: PathBuf,
    pub(crate) recursive: bool,
    pub(crate) hide_hidden: bool,
    pub(crate) query: String,
    pub(crate) show_relative: bool,
    pub(crate) show_info: bool,
    pub(crate) mode: SearchMode,
    pub(crate) entry_filter: EntryFilter,
    pub(crate) ext_filter: Option<Vec<String>>,
}

impl SearchKey {
    pub(crate) fn new(
        dir: &Path,
        recursive: bool,
        hide_hidden: bool,
        query: &str,
        show_relative: bool,
        show_info: bool,
        mode: SearchMode,
        entry_filter: EntryFilter,
        ext_filter: Option<&HashSet<String>>,
    ) -> Self {
        let ext_filter = ext_filter.map(|exts| {
            let mut list = exts.iter().cloned().collect::<Vec<_>>();
            list.sort();
            list
        });
        Self {
            dir: dir.to_path_buf(),
            recursive,
            hide_hidden,
            query: query.to_string(),
            show_relative,
            show_info,
            mode,
            entry_filter,
            ext_filter,
        }
    }
}
