mod commit_policy;
mod conditions;
mod derived;
mod outputs;
mod submit;
mod triggering;

pub(super) use super::AppState;

use crate::state::change::StoreCommitPolicy;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::widgets::inputs::text::TextInput;
use crate::widgets::node::Node;
use crate::widgets::shared::binding::{
    ReadBinding, StoreBinding, WriteBinding, WriteExpr, bind_node,
};

pub(super) fn bound_on_submit_text_input(id: &str, label: &str, selector: &str) -> Node {
    let target = crate::core::value_path::ValueTarget::node(selector);
    bind_node(
        Node::Input(Box::new(TextInput::new(id, label))),
        StoreBinding {
            value: Some(target.clone()),
            options: None,
            reads: Some(ReadBinding::Selector(target.clone())),
            writes: vec![WriteBinding {
                target,
                expr: WriteExpr::ScopeRef("value".to_string()),
            }],
            commit_policy: StoreCommitPolicy::OnSubmit,
        },
    )
}

pub(super) fn bound_immediate_text_input(id: &str, label: &str, target: &str) -> Node {
    let target = crate::core::store_refs::parse_store_selector(target)
        .unwrap_or_else(|_| crate::core::value_path::ValueTarget::node(target));
    bind_node(
        Node::Input(Box::new(TextInput::new(id, label))),
        StoreBinding {
            value: Some(target.clone()),
            options: None,
            reads: Some(ReadBinding::Selector(target.clone())),
            writes: vec![WriteBinding {
                target,
                expr: WriteExpr::ScopeRef("value".to_string()),
            }],
            commit_policy: StoreCommitPolicy::Immediate,
        },
    )
}

pub(super) fn derived_copy_text_input(
    id: &str,
    label: &str,
    read_selector: &str,
    write_selector: &str,
) -> Node {
    bind_node(
        Node::Input(Box::new(TextInput::new(id, label))),
        StoreBinding {
            value: None,
            options: None,
            reads: Some(ReadBinding::Selector(
                crate::core::value_path::ValueTarget::node(read_selector),
            )),
            writes: vec![WriteBinding {
                target: crate::core::value_path::ValueTarget::node(write_selector),
                expr: WriteExpr::ScopeRef("value".to_string()),
            }],
            commit_policy: StoreCommitPolicy::Immediate,
        },
    )
}

pub(super) fn char_key(ch: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(ch),
        modifiers: KeyModifiers::NONE,
    }
}
