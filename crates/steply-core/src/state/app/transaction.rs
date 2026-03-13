use super::AppState;
use crate::core::value::Value;
use crate::core::value_path::ValueTarget;
use crate::state::change::{
    StoreOwnership, StorePatch, StorePatchEntry, StoreTransaction, StoreWriteOrigin,
};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct AppliedStorePatch {
    changed_targets: Vec<ValueTarget>,
    seen_targets: HashSet<String>,
}

impl AppliedStorePatch {
    pub(super) fn record_change(&mut self, target: ValueTarget) {
        let selector = target.to_selector();
        if self.seen_targets.insert(selector) {
            self.changed_targets.push(target);
        }
    }

    pub(super) fn extend(&mut self, other: AppliedStorePatch) {
        for target in other.changed_targets {
            self.record_change(target);
        }
    }

    pub(super) fn into_targets(self) -> Vec<ValueTarget> {
        self.changed_targets
    }

    pub(super) fn is_empty(&self) -> bool {
        self.changed_targets.is_empty()
    }
}

impl AppState {
    pub(super) fn apply_runtime_store_patch(&mut self, patch: StorePatch) {
        let applied = self.apply_runtime_store_patch_collect(patch);
        self.emit_store_change_triggers(applied.into_targets());
    }

    pub(super) fn apply_store_transaction(
        &mut self,
        transaction: StoreTransaction,
    ) -> AppliedStorePatch {
        self.apply_store_patch(transaction.into_patch())
    }

    pub(super) fn apply_store_patch(&mut self, patch: StorePatch) -> AppliedStorePatch {
        let mut applied = AppliedStorePatch::default();
        for entry in patch.into_entries() {
            self.apply_store_patch_entry(entry, &mut applied);
        }
        applied
    }

    pub(super) fn apply_user_value_change(
        &mut self,
        source: String,
        target: ValueTarget,
        value: Value,
    ) {
        let mut transaction = StoreTransaction::new();
        transaction.push(
            target,
            value,
            StoreWriteOrigin::UserInput { node_id: source },
        );
        let applied = self.apply_runtime_store_transaction_collect(transaction);
        self.emit_store_change_triggers(applied.into_targets());
    }

    pub(super) fn apply_system_value_change(
        &mut self,
        target: ValueTarget,
        value: Value,
        origin: StoreWriteOrigin,
    ) {
        let mut transaction = StoreTransaction::new();
        transaction.push(target, value, origin);
        let applied = self.apply_runtime_store_transaction_collect(transaction);
        self.emit_store_change_triggers(applied.into_targets());
    }

    fn apply_runtime_store_transaction_collect(
        &mut self,
        transaction: StoreTransaction,
    ) -> AppliedStorePatch {
        let applied = self.apply_store_transaction(transaction);
        self.finalize_runtime_store_patch(applied)
    }

    fn apply_runtime_store_patch_collect(&mut self, patch: StorePatch) -> AppliedStorePatch {
        let applied = self.apply_store_patch(patch);
        self.finalize_runtime_store_patch(applied)
    }

    fn finalize_runtime_store_patch(
        &mut self,
        mut applied: AppliedStorePatch,
    ) -> AppliedStorePatch {
        if !self.reconcile_current_step_after_store_change() {
            applied.extend(self.refresh_current_step_bindings_collect_for_live());
        }
        applied
    }

    fn apply_store_patch_entry(
        &mut self,
        entry: StorePatchEntry,
        applied: &mut AppliedStorePatch,
    ) -> bool {
        let selector = entry.target.to_selector();
        let conflicting_owners = self.conflicting_store_owners(&entry.target, &entry.origin);
        if !conflicting_owners.is_empty() {
            self.runtime.validation.set_runtime_step_error(
                store_ownership_error_key(selector.as_str()),
                format!(
                    "store selector '{}' is owned by {} and cannot be written by {}",
                    selector,
                    describe_owners(conflicting_owners.as_slice()),
                    describe_origin(&entry.origin)
                ),
            );
            return false;
        }
        self.runtime
            .validation
            .clear_runtime_step_error(store_ownership_error_key(selector.as_str()).as_str());

        match self.write_store_target(entry.target.clone(), entry.value) {
            Ok(changed) => {
                if changed {
                    applied.record_change(entry.target);
                }
                changed
            }
            Err(err) => {
                let root = err_root_key(&err);
                self.runtime
                    .validation
                    .set_runtime_step_error(store_write_error_key(root.as_str()), err.to_string());
                false
            }
        }
    }

    fn conflicting_store_owners(
        &self,
        target: &ValueTarget,
        origin: &StoreWriteOrigin,
    ) -> Vec<StoreOwnership> {
        if matches!(origin, StoreWriteOrigin::System) {
            return Vec::new();
        }
        self.runtime
            .store_ownership
            .conflicting_owners_for_write(target, origin.ownership())
    }

    pub(super) fn write_store_target(
        &mut self,
        target: ValueTarget,
        value: Value,
    ) -> Result<bool, crate::state::store::StoreWriteError> {
        let root = target.root().clone();
        let before = self.data.store.get(root.as_str()).cloned();
        self.data.store.set_target(&target, value)?;
        self.runtime
            .validation
            .clear_runtime_step_error(store_write_error_key(root.as_str()).as_str());
        let updated = self.data.store.get(root.as_str()).cloned();
        Ok(updated
            .as_ref()
            .is_some_and(|updated| before.as_ref() != Some(updated)))
    }
}

pub(super) fn err_root_key(err: &crate::state::store::StoreWriteError) -> String {
    match err {
        crate::state::store::StoreWriteError::RootTypeConflict { root, .. } => root.clone(),
        crate::state::store::StoreWriteError::PathTypeConflict { target, .. } => {
            ValueTarget::parse_selector(target)
                .map(|target| target.root().to_string())
                .unwrap_or_else(|_| target.clone())
        }
    }
}

pub(super) fn store_write_error_key(root: &str) -> String {
    format!("store:{root}")
}

fn store_ownership_error_key(selector: &str) -> String {
    format!("ownership:{selector}")
}

fn describe_origin(origin: &StoreWriteOrigin) -> &'static str {
    match origin {
        StoreWriteOrigin::UserInput { .. } | StoreWriteOrigin::StepSubmit { .. } => "user input",
        StoreWriteOrigin::TaskResult { .. } => "task result",
        StoreWriteOrigin::Derived { .. } => "derived binding",
        StoreWriteOrigin::DefaultSeed { .. } => "default seed",
        StoreWriteOrigin::System => "system update",
    }
}

fn describe_owners(owners: &[StoreOwnership]) -> String {
    let mut labels = owners
        .iter()
        .map(|owner| match owner {
            StoreOwnership::User => "user bindings",
            StoreOwnership::Task => "task results",
            StoreOwnership::Derived => "derived bindings",
            StoreOwnership::Shared => "shared writers",
        })
        .collect::<Vec<_>>();
    labels.sort_unstable();
    labels.dedup();
    labels.join(" and ")
}
