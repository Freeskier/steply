use super::{StoreOwnership, StoreOwnershipRegistry};
use crate::core::value_path::ValueTarget;

#[test]
fn registry_detects_overlapping_conflicting_owners() {
    let mut registry = StoreOwnershipRegistry::default();
    registry.register(
        ValueTarget::parse_selector("payload::user.name").expect("user target"),
        StoreOwnership::User,
    );
    registry.register(
        ValueTarget::parse_selector("payload::task").expect("task target"),
        StoreOwnership::Task,
    );

    let conflicts = registry.conflicting_owners_for_write(
        &ValueTarget::parse_selector("payload").expect("write target"),
        StoreOwnership::User,
    );

    assert_eq!(conflicts, vec![StoreOwnership::Task]);
}

#[test]
fn registry_allows_same_owner_overlaps() {
    let mut registry = StoreOwnershipRegistry::default();
    registry.register(
        ValueTarget::parse_selector("payload::items[0]").expect("first target"),
        StoreOwnership::User,
    );
    registry.register(
        ValueTarget::parse_selector("payload::items[1]").expect("second target"),
        StoreOwnership::User,
    );

    let conflicts = registry.conflicting_owners_for_write(
        &ValueTarget::parse_selector("payload::items").expect("write target"),
        StoreOwnership::User,
    );

    assert!(conflicts.is_empty());
}
