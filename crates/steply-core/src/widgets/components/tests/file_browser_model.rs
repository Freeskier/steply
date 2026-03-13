use super::{EntryFilter, EntryKind, build_entry, filter_entries};

#[test]
fn files_only_filter_keeps_directories_visible_for_navigation() {
    let entries = vec![
        build_entry("src".into(), "src".into(), EntryKind::Dir),
        build_entry("main.rs".into(), "src/main.rs".into(), EntryKind::File),
    ];

    let filtered = filter_entries(entries, EntryFilter::FilesOnly, None);

    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().any(|entry| entry.kind.is_dir()));
    assert!(
        filtered
            .iter()
            .any(|entry| matches!(entry.kind, EntryKind::File))
    );
}
