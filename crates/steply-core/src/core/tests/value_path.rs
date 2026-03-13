use super::{PathSegment, ValuePath, ValueTarget, ValueTargetRelation};

#[test]
fn node_target_contains_nested_path_target() {
    let root = ValueTarget::node("store");
    let nested = ValueTarget::path(
        "store",
        ValuePath::new(vec![PathSegment::Key("value".into())]),
    );

    assert_eq!(root.relation_to(&nested), ValueTargetRelation::Contains);
    assert!(root.overlaps(&nested));
    assert!(root.contains_target(&nested));
    assert_eq!(nested.relation_to(&root), ValueTargetRelation::ContainedBy);
}

#[test]
fn sibling_targets_are_disjoint() {
    let left = ValueTarget::parse_selector("store::items[0]").expect("left target");
    let right = ValueTarget::parse_selector("store::items[1]").expect("right target");

    assert_eq!(left.relation_to(&right), ValueTargetRelation::Disjoint);
    assert!(!left.overlaps(&right));
    assert!(!left.contains_target(&right));
}

#[test]
fn equal_targets_overlap() {
    let left = ValueTarget::parse_selector("store::items[0].name").expect("left target");
    let right = ValueTarget::parse_selector("store::items[0].name").expect("right target");

    assert_eq!(left.relation_to(&right), ValueTargetRelation::Equal);
    assert!(left.overlaps(&right));
    assert!(left.contains_target(&right));
}
