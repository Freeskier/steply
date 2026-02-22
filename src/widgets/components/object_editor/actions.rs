use super::*;

impl ObjectEditor {
    pub(super) fn toggle_expand(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        if !matches!(obj.value, Value::Object(_) | Value::List(_)) {
            return;
        }
        let path = obj.path.clone();
        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }
        self.rebuild();
    }

    pub(super) fn start_edit_value(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        if obj.is_placeholder {
            return;
        }
        if matches!(obj.value, Value::Object(_) | Value::List(_)) {
            return;
        }
        let text = obj.value.to_text_scalar().unwrap_or_else(|| "null".into());
        let vis = self.active_vis();
        let mut key_value = InlineKeyValueEditor::new_text(format!("{}_ekv", self.base.id()), "")
            .with_default_key(obj.key.clone())
            .with_default_value(text);
        key_value.set_focus(InlineKeyValueFocus::Value);
        self.mode = Mode::EditValue { vis, key_value };
    }

    pub(super) fn commit_edit_value(&mut self) {
        let Mode::EditValue { vis, ref key_value } = self.mode else {
            return;
        };
        let text = key_value.value_text();
        let new_val = Self::parse_scalar(&text);
        let path = self.path_at(vis);
        let ppath = Self::parent_path(&path);
        let key = Self::leaf_key(&path);
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
            match parent {
                Value::Object(map) => {
                    map.insert(key, new_val);
                }
                Value::List(arr) => {
                    if let Ok(i) = key.parse::<usize>() {
                        if i < arr.len() {
                            arr[i] = new_val;
                        }
                    }
                }
                _ => {}
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    pub(super) fn start_edit_key(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        if obj.is_index || obj.is_placeholder {
            return;
        }
        let value = match &obj.value {
            Value::Object(map) => format!("{{{}}}", map.len()),
            Value::List(list) => format!("[{}]", list.len()),
            _ => obj
                .value
                .to_text_scalar()
                .unwrap_or_else(|| "null".to_string()),
        };
        let vis = self.active_vis();
        let mut key_value = InlineKeyValueEditor::new_text(format!("{}_ekv", self.base.id()), "")
            .with_default_key(obj.key.clone())
            .with_default_value(value);
        key_value.set_focus(InlineKeyValueFocus::Key);
        self.mode = Mode::EditKey { vis, key_value };
    }

    pub(super) fn commit_edit_key(&mut self) {
        let Mode::EditKey { vis, ref key_value } = self.mode else {
            return;
        };
        let new_key = key_value.key();
        if new_key.is_empty() {
            self.mode = Mode::Normal;
            return;
        }
        let path = self.path_at(vis);
        let ppath = Self::parent_path(&path);
        let old_key = Self::leaf_key(&path);
        let mut remap_paths: Option<(String, String)> = None;
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
            if let Value::Object(map) = parent {
                if old_key != new_key {
                    let mut insert_idx = map.get_index_of(old_key.as_str()).unwrap_or(map.len());
                    if let Some(val) = map.shift_remove(&old_key) {
                        if let Some(existing_idx) = map.get_index_of(new_key.as_str()) {
                            map.shift_remove(&new_key);
                            if existing_idx < insert_idx {
                                insert_idx = insert_idx.saturating_sub(1);
                            }
                        }
                        let insert_idx = insert_idx.min(map.len());
                        map.shift_insert(insert_idx, new_key.clone(), val);
                        remap_paths = Some((
                            path.clone(),
                            Self::append_key(ppath.as_str(), new_key.as_str()),
                        ));
                    }
                }
            }
        }
        if let Some((old_path, new_path)) = remap_paths {
            self.remap_expanded_prefix(old_path.as_str(), new_path.as_str());
            self.remap_array_name_prefix(old_path.as_str(), new_path.as_str());
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    pub(super) fn start_insert(&mut self) {
        let after_vis = self.active_vis();
        let path = self.path_at(after_vis);
        let parent_path = self
            .active_obj()
            .and_then(|obj| obj.placeholder_parent.clone())
            .unwrap_or_else(|| Self::parent_path(&path));
        let parent_is_list = matches!(
            Self::value_at_path(&self.value, &parent_path),
            Some(Value::List(_))
        );
        if parent_is_list {
            let next_index = if self
                .active_obj()
                .map(|obj| obj.is_placeholder)
                .unwrap_or(false)
            {
                0
            } else {
                Self::leaf_key(&path)
                    .parse::<usize>()
                    .ok()
                    .map(|idx| idx + 1)
                    .unwrap_or(0)
            };
            let mut key_value =
                InlineKeyValueEditor::new_text(format!("{}_iv", self.base.id()), "")
                    .with_default_key(next_index.to_string())
                    .with_default_value("");
            key_value.set_focus(InlineKeyValueFocus::Value);
            self.mode = Mode::InsertValue {
                after_vis,
                value_type: InsertValueType::Text,
                key_value,
            };
            return;
        }
        self.mode = Mode::InsertType {
            after_vis,
            key_value: InlineKeyValueEditor::new(
                format!("{}_ikv", self.base.id()),
                "",
                self.insert_type_options(),
            ),
        };
    }

    pub(super) fn commit_insert_type(&mut self) {
        let Mode::InsertType {
            after_vis,
            ref key_value,
            ..
        } = self.mode
        else {
            return;
        };
        let key = key_value.key();
        if key.is_empty() {
            self.mode = Mode::Normal;
            return;
        }
        let type_val = key_value.value_type();
        let av = after_vis;
        let k = key.clone();
        let tv = type_val.clone();

        match tv.as_str() {
            "object" | "array" => {
                let new_val = if tv == "object" {
                    Value::Object(IndexMap::new())
                } else {
                    Value::List(Vec::new())
                };
                let inserted_path = self.do_insert(av, k, new_val);
                self.mode = Mode::Normal;
                if let Some(path) = inserted_path.as_ref() {
                    self.expanded.insert(path.clone());
                }
                self.rebuild();
                if let Some(path) = inserted_path
                    && let Some(vis) = self
                        .vis_of_empty_placeholder(path.as_str())
                        .or_else(|| self.vis_of_path(path.as_str()))
                {
                    self.tree.set_active_visible_index(vis);
                }
            }
            _ => {
                let value_type = self.resolve_insert_value_type(tv.as_str());
                let mut key_value =
                    InlineKeyValueEditor::new_text(format!("{}_iv", self.base.id()), "")
                        .with_default_key(k)
                        .with_default_value("");
                key_value.set_focus(InlineKeyValueFocus::Value);
                self.mode = Mode::InsertValue {
                    after_vis: av,
                    value_type,
                    key_value,
                };
            }
        }
    }

    pub(super) fn commit_insert_value(&mut self) {
        let Mode::InsertValue {
            after_vis,
            ref key_value,
            value_type,
        } = self.mode
        else {
            return;
        };
        let text = key_value.value_text();
        let new_val = match value_type {
            InsertValueType::Number => Value::Number(text.parse::<f64>().unwrap_or(0.0)),
            InsertValueType::Text => Self::parse_scalar(&text),
            InsertValueType::Custom(index) => self
                .custom_insert_types
                .get(index)
                .map(|custom| custom.parse(text.as_str()))
                .unwrap_or_else(|| Self::parse_scalar(&text)),
        };
        let av = after_vis;
        let k = key_value.key();
        let inserted_path = self.do_insert(av, k, new_val);
        self.mode = Mode::Normal;
        self.rebuild();
        if let Some(path) = inserted_path
            && let Some(vis) = self.vis_of_path(path.as_str())
        {
            self.tree.set_active_visible_index(vis);
        }
    }

    pub(super) fn do_insert(
        &mut self,
        after_vis: usize,
        new_key: String,
        new_val: Value,
    ) -> Option<String> {
        let Some(anchor) = self.obj_at_vis(after_vis) else {
            return None;
        };
        let placeholder_anchor = anchor.is_placeholder;
        let anchor_path = anchor.path.clone();
        let placeholder_parent = anchor.placeholder_parent.clone();
        let ppath = if placeholder_anchor {
            placeholder_parent.unwrap_or_else(|| Self::parent_path(&anchor_path))
        } else {
            Self::parent_path(&anchor_path)
        };
        let sib_key = if placeholder_anchor {
            String::new()
        } else {
            Self::leaf_key(&anchor_path)
        };
        if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
            match parent {
                Value::Object(map) => {
                    let insert_idx = if placeholder_anchor {
                        0
                    } else {
                        map.get_index_of(sib_key.as_str())
                            .map(|idx| idx + 1)
                            .unwrap_or(map.len())
                            .min(map.len())
                    };
                    map.shift_insert(insert_idx, new_key.clone(), new_val);
                    return Some(Self::append_key(ppath.as_str(), new_key.as_str()));
                }
                Value::List(arr) => {
                    let insert_idx = if placeholder_anchor {
                        0
                    } else {
                        let idx = sib_key.parse::<usize>().unwrap_or(arr.len());
                        idx.saturating_add(1).min(arr.len())
                    };
                    arr.insert(insert_idx, new_val);
                    return Some(Self::append_index(ppath.as_str(), insert_idx));
                }
                _ => {}
            }
        } else {
            match &mut self.value {
                Value::Object(map) => {
                    map.insert(new_key.clone(), new_val);
                    return Some(new_key);
                }
                Value::List(arr) => {
                    arr.push(new_val);
                    let idx = arr.len().saturating_sub(1);
                    return Some(idx.to_string());
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn start_delete(&mut self) {
        let Some(obj) = self.active_obj() else { return };
        if obj.is_placeholder {
            return;
        }
        let vis = self.active_vis();
        let label = obj.key.clone();
        let select = SelectInput::new(
            format!("{}_cd", self.base.id()),
            format!("Delete {label}?"),
            vec!["No".into(), "Yes".into()],
        );
        self.mode = Mode::ConfirmDelete { vis, select };
    }

    pub(super) fn commit_delete(&mut self, confirmed: bool) {
        let Mode::ConfirmDelete { vis, .. } = self.mode else {
            return;
        };
        if confirmed {
            let path = self.path_at(vis);
            self.remove_array_name_subtree(&path);
            let ppath = Self::parent_path(&path);
            let key = Self::leaf_key(&path);
            if let Some(parent) = Self::value_at_path_mut(&mut self.value, &ppath) {
                match parent {
                    Value::Object(map) => {
                        map.shift_remove(&key);
                    }
                    Value::List(arr) => {
                        if let Ok(i) = key.parse::<usize>() {
                            if i < arr.len() {
                                arr.remove(i);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.mode = Mode::Normal;
        self.rebuild();
    }

    pub(super) fn start_move(&mut self) {
        if self.active_obj().map(|o| o.is_placeholder).unwrap_or(false) {
            return;
        }
        let vis = self.active_vis();
        self.mode = Mode::Move { vis };
    }

    pub(super) fn move_target_for_step(
        &self,
        current_vis: usize,
        step: isize,
    ) -> Option<(usize, bool)> {
        let total = self.tree.visible().len();
        if total <= 1 {
            return None;
        }
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();
        let current_node_idx = *visible.get(current_vis)?;
        let current_depth = nodes.get(current_node_idx).map(|n| n.depth).unwrap_or(0);

        let mut target_vis = if current_depth == 0 {
            let root_positions: Vec<usize> = visible
                .iter()
                .enumerate()
                .filter_map(|(vis_idx, node_idx)| {
                    nodes
                        .get(*node_idx)
                        .and_then(|node| (node.depth == 0).then_some(vis_idx))
                })
                .collect();
            if let (Some(first_root), Some(last_root)) = (
                root_positions.first().copied(),
                root_positions.last().copied(),
            ) {
                if step < 0 && current_vis == first_root {
                    last_root
                } else if step > 0 && current_vis == last_root {
                    first_root
                } else {
                    ((current_vis as isize + step + total as isize) % total as isize) as usize
                }
            } else {
                ((current_vis as isize + step + total as isize) % total as isize) as usize
            }
        } else {
            ((current_vis as isize + step + total as isize) % total as isize) as usize
        };

        if step > 0 {
            let source_path = self.path_at(current_vis);
            let candidate_path = self.path_at(target_vis);
            if Self::is_descendant_path(candidate_path.as_str(), source_path.as_str()) {
                let subtree_end = self.subtree_vis_range(current_vis).end;
                target_vis = if subtree_end < total { subtree_end } else { 0 };
            }
        }

        if self.is_placeholder_vis(current_vis) {
            return None;
        }

        if step > 0 {
            let mut guard = 0usize;
            while guard < total {
                let path = self.path_at(target_vis);
                let is_placeholder = self.is_placeholder_vis(target_vis);
                let source_path = self.path_at(current_vis);
                if !is_placeholder && !Self::is_descendant_path(path.as_str(), source_path.as_str())
                {
                    break;
                }
                target_vis = (target_vis + 1) % total;
                guard += 1;
            }
        } else {
            let mut guard = 0usize;
            while guard < total {
                let is_placeholder = self.is_placeholder_vis(target_vis);
                if !is_placeholder {
                    break;
                }
                target_vis = (target_vis + total - 1) % total;
                guard += 1;
            }
        }

        let wrapped_cycle =
            (step > 0 && target_vis < current_vis) || (step < 0 && target_vis > current_vis);
        Some((target_vis, wrapped_cycle))
    }

    pub(super) fn is_open_container_path(&self, path: &str) -> bool {
        matches!(
            Self::value_at_path(&self.value, path),
            Some(Value::Object(_) | Value::List(_))
        ) && self.expanded.contains(path)
    }

    pub(super) fn build_move_plan(
        &self,
        current_vis: usize,
        step: isize,
        target_vis: usize,
        wrapped_between_roots: bool,
    ) -> MovePlan {
        let source_path = self.path_at(current_vis);
        let source_parent = Self::parent_path(&source_path);
        let target_path = self.path_at(target_vis);
        let target_parent = Self::parent_path(&target_path);
        let target_is_open_container =
            self.is_open_container_path(target_path.as_str()) && !wrapped_between_roots;
        let can_enter_target =
            target_is_open_container && !(step > 0 && target_parent != source_parent);

        let (dest_parent, placement) = if step < 0 && target_path == source_parent {
            (
                target_parent.clone(),
                InsertPlacement::Before(Self::leaf_key(&source_parent)),
            )
        } else if can_enter_target {
            let placement = if step > 0 {
                InsertPlacement::Start
            } else {
                InsertPlacement::End
            };
            (target_path.clone(), placement)
        } else if target_parent == source_parent {
            let anchor = Self::leaf_key(&target_path);
            let placement = if wrapped_between_roots {
                if step > 0 {
                    InsertPlacement::Before(anchor)
                } else {
                    InsertPlacement::After(anchor)
                }
            } else if step > 0 {
                InsertPlacement::After(anchor)
            } else {
                InsertPlacement::Before(anchor)
            };
            (source_parent.clone(), placement)
        } else {
            let anchor = Self::leaf_key(&target_path);
            let placement = if step > 0 {
                InsertPlacement::Before(anchor)
            } else {
                InsertPlacement::After(anchor)
            };
            (target_parent.clone(), placement)
        };

        MovePlan {
            target_vis,
            source_path,
            dest_parent,
            placement,
        }
    }

    pub(super) fn can_apply_move(&self, source_path: &str, dest_parent: &str) -> bool {
        if dest_parent == source_path || Self::is_descendant_path(dest_parent, source_path) {
            return false;
        }
        if !dest_parent.is_empty() && !self.expanded.contains(dest_parent) {
            return false;
        }
        matches!(
            Self::value_at_path(&self.value, dest_parent),
            Some(Value::Object(_) | Value::List(_))
        )
    }

    pub(super) fn apply_move_plan(&mut self, plan: &MovePlan) -> Option<String> {
        if !self.can_apply_move(plan.source_path.as_str(), plan.dest_parent.as_str()) {
            return None;
        }

        let source_parent = Self::parent_path(&plan.source_path);
        let source_name = self.array_item_names.remove(&plan.source_path);
        let Some((key, was_index, value)) = self.take_node_at_path(plan.source_path.as_str())
        else {
            if let Some(name) = source_name {
                self.array_item_names.insert(plan.source_path.clone(), name);
            }
            return None;
        };

        let inserted = self.insert_node_under_parent(
            plan.dest_parent.as_str(),
            key.clone(),
            was_index,
            source_name.clone(),
            value,
            plan.placement.clone(),
            source_parent.as_str(),
        );

        let Some(new_path) = inserted else {
            if let Some(name) = source_name {
                self.array_item_names.insert(plan.source_path.clone(), name);
            }
            return None;
        };

        let dest_is_list = matches!(
            Self::value_at_path(&self.value, plan.dest_parent.as_str()),
            Some(Value::List(_))
        );
        if !was_index && dest_is_list {
            self.array_item_names.insert(new_path.clone(), key);
        } else if was_index
            && dest_is_list
            && let Some(name) = source_name
        {
            self.array_item_names.insert(new_path.clone(), name);
        }

        self.remap_expanded_prefix(plan.source_path.as_str(), new_path.as_str());
        self.remap_array_name_prefix(plan.source_path.as_str(), new_path.as_str());
        Some(new_path)
    }

    pub(super) fn move_node(&mut self, delta: isize) {
        let current_vis = match self.mode {
            Mode::Move { vis } => vis,
            _ => return,
        };
        let step = if delta >= 0 { 1 } else { -1 };
        let Some((target_vis, wrapped_between_roots)) =
            self.move_target_for_step(current_vis, step)
        else {
            return;
        };

        let plan = self.build_move_plan(current_vis, step, target_vis, wrapped_between_roots);
        let moved_path = self.apply_move_plan(&plan);

        self.rebuild();
        let new_vis = moved_path
            .as_deref()
            .and_then(|path| self.vis_of_path(path))
            .unwrap_or(plan.target_vis);
        self.tree.set_active_visible_index(new_vis);
        self.mode = Mode::Move { vis: new_vis };
    }
}
