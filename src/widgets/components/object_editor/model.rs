use super::*;

impl ObjectEditor {
    pub(super) fn build_nodes(
        value: &Value,
        expanded: &HashSet<String>,
        depth: usize,
        prefix: &ValuePath,
    ) -> Vec<TreeNode<ObjNode>> {
        let mut out = Vec::new();
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let mut segs = prefix.segments().to_vec();
                    segs.push(PathSegment::Key(key.clone()));
                    let path_value = ValuePath::new(segs);
                    let path = path_value.to_string();
                    let is_container = matches!(child, Value::Object(_) | Value::List(_));
                    let is_exp = is_container && expanded.contains(&path);
                    let mut node = TreeNode::new(
                        ObjNode {
                            key: key.clone(),
                            value: child.clone(),
                            path: path.clone(),
                            is_index: false,
                            is_placeholder: false,
                            placeholder_parent: None,
                        },
                        depth,
                        is_container,
                    );
                    if is_exp {
                        node.expanded = true;
                        node.children_loaded = true;
                    }
                    out.push(node);
                    if is_exp {
                        match child {
                            Value::Object(map) if map.is_empty() => {
                                let mut placeholder = path_value.segments().to_vec();
                                placeholder.push(PathSegment::Key("__placeholder__".to_string()));
                                out.push(TreeNode::new(
                                    ObjNode {
                                        key: "(empty)".to_string(),
                                        value: Value::None,
                                        path: ValuePath::new(placeholder).to_string(),
                                        is_index: false,
                                        is_placeholder: true,
                                        placeholder_parent: Some(path.clone()),
                                    },
                                    depth + 1,
                                    false,
                                ));
                            }
                            Value::List(list) if list.is_empty() => {
                                let mut placeholder = path_value.segments().to_vec();
                                placeholder.push(PathSegment::Key("__placeholder__".to_string()));
                                out.push(TreeNode::new(
                                    ObjNode {
                                        key: "(empty)".to_string(),
                                        value: Value::None,
                                        path: ValuePath::new(placeholder).to_string(),
                                        is_index: false,
                                        is_placeholder: true,
                                        placeholder_parent: Some(path.clone()),
                                    },
                                    depth + 1,
                                    false,
                                ));
                            }
                            _ => out.extend(Self::build_nodes(
                                child,
                                expanded,
                                depth + 1,
                                &path_value,
                            )),
                        }
                    }
                }
            }
            Value::List(arr) => {
                for (i, child) in arr.iter().enumerate() {
                    let key = i.to_string();
                    let mut segs = prefix.segments().to_vec();
                    segs.push(PathSegment::Index(i));
                    let path_value = ValuePath::new(segs);
                    let path = path_value.to_string();
                    let is_container = matches!(child, Value::Object(_) | Value::List(_));
                    let is_exp = is_container && expanded.contains(&path);
                    let mut node = TreeNode::new(
                        ObjNode {
                            key: key.clone(),
                            value: child.clone(),
                            path: path.clone(),
                            is_index: true,
                            is_placeholder: false,
                            placeholder_parent: None,
                        },
                        depth,
                        is_container,
                    );
                    if is_exp {
                        node.expanded = true;
                        node.children_loaded = true;
                    }
                    out.push(node);
                    if is_exp {
                        match child {
                            Value::Object(map) if map.is_empty() => {
                                let mut placeholder = path_value.segments().to_vec();
                                placeholder.push(PathSegment::Key("__placeholder__".to_string()));
                                out.push(TreeNode::new(
                                    ObjNode {
                                        key: "(empty)".to_string(),
                                        value: Value::None,
                                        path: ValuePath::new(placeholder).to_string(),
                                        is_index: false,
                                        is_placeholder: true,
                                        placeholder_parent: Some(path.clone()),
                                    },
                                    depth + 1,
                                    false,
                                ));
                            }
                            Value::List(list) if list.is_empty() => {
                                let mut placeholder = path_value.segments().to_vec();
                                placeholder.push(PathSegment::Key("__placeholder__".to_string()));
                                out.push(TreeNode::new(
                                    ObjNode {
                                        key: "(empty)".to_string(),
                                        value: Value::None,
                                        path: ValuePath::new(placeholder).to_string(),
                                        is_index: false,
                                        is_placeholder: true,
                                        placeholder_parent: Some(path.clone()),
                                    },
                                    depth + 1,
                                    false,
                                ));
                            }
                            _ => out.extend(Self::build_nodes(
                                child,
                                expanded,
                                depth + 1,
                                &path_value,
                            )),
                        }
                    }
                }
            }
            _ => {}
        }
        out
    }

    pub(super) fn active_vis(&self) -> usize {
        self.tree.active_visible_index()
    }

    pub(super) fn active_obj(&self) -> Option<&ObjNode> {
        self.tree.active_node().map(|n| &n.item)
    }

    pub(super) fn obj_at_vis(&self, vis: usize) -> Option<&ObjNode> {
        let visible = self.tree.visible();
        visible
            .get(vis)
            .and_then(|&idx| self.tree.nodes().get(idx))
            .map(|n| &n.item)
    }

    pub(super) fn is_placeholder_vis(&self, vis: usize) -> bool {
        self.obj_at_vis(vis)
            .map(|obj| obj.is_placeholder)
            .unwrap_or(false)
    }

    pub(super) fn path_at(&self, vis: usize) -> String {
        self.obj_at_vis(vis)
            .map(|obj| obj.path.clone())
            .unwrap_or_default()
    }

    pub(super) fn vis_of_path(&self, path: &str) -> Option<usize> {
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();
        visible
            .iter()
            .position(|&idx| nodes.get(idx).map(|n| n.item.path == path).unwrap_or(false))
    }

    pub(super) fn vis_of_empty_placeholder(&self, parent_path: &str) -> Option<usize> {
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();
        visible.iter().position(|&idx| {
            nodes
                .get(idx)
                .map(|n| {
                    n.item.is_placeholder
                        && n.item.placeholder_parent.as_deref() == Some(parent_path)
                })
                .unwrap_or(false)
        })
    }

    pub(super) fn subtree_vis_range(&self, vis: usize) -> std::ops::Range<usize> {
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();
        if vis >= visible.len() {
            return vis..vis;
        }
        let depth = nodes[visible[vis]].depth;
        let end = visible[vis + 1..]
            .iter()
            .position(|&idx| nodes.get(idx).map(|n| n.depth <= depth).unwrap_or(true))
            .map(|p| vis + 1 + p)
            .unwrap_or(visible.len());
        vis + 1..end
    }

    pub(super) fn parse_path(path: &str) -> Option<ValuePath> {
        if path.is_empty() {
            return Some(ValuePath::empty());
        }
        ValuePath::parse_relative(path).ok()
    }

    pub(super) fn value_at_path_mut<'a>(root: &'a mut Value, path: &str) -> Option<&'a mut Value> {
        let parsed = Self::parse_path(path)?;
        root.get_path_mut(&parsed)
    }

    pub(super) fn value_at_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
        let parsed = Self::parse_path(path)?;
        root.get_path(&parsed)
    }

    pub(super) fn parent_path(path: &str) -> String {
        let Some(parsed) = Self::parse_path(path) else {
            return String::new();
        };
        let mut segments = parsed.segments().to_vec();
        segments.pop();
        ValuePath::new(segments).to_string()
    }

    pub(super) fn leaf_key(path: &str) -> String {
        let Some(parsed) = Self::parse_path(path) else {
            return path.to_string();
        };
        match parsed.segments().last() {
            Some(PathSegment::Key(key)) => key.clone(),
            Some(PathSegment::Index(idx)) => idx.to_string(),
            None => String::new(),
        }
    }

    pub(super) fn append_key(parent: &str, key: &str) -> String {
        let mut segments = Self::parse_path(parent)
            .unwrap_or_else(ValuePath::empty)
            .segments()
            .to_vec();
        segments.push(PathSegment::Key(key.to_string()));
        ValuePath::new(segments).to_string()
    }

    pub(super) fn append_index(parent: &str, index: usize) -> String {
        let mut segments = Self::parse_path(parent)
            .unwrap_or_else(ValuePath::empty)
            .segments()
            .to_vec();
        segments.push(PathSegment::Index(index));
        ValuePath::new(segments).to_string()
    }

    pub(super) fn is_descendant_path(path: &str, ancestor: &str) -> bool {
        let Some(path_parsed) = Self::parse_path(path) else {
            return false;
        };
        let Some(ancestor_parsed) = Self::parse_path(ancestor) else {
            return false;
        };
        if ancestor_parsed.is_empty() {
            return !path_parsed.is_empty();
        }
        let p = path_parsed.segments();
        let a = ancestor_parsed.segments();
        p.len() > a.len() && p[..a.len()] == *a
    }

    pub(super) fn rebase_path(path: &str, old_prefix: &str, new_prefix: &str) -> Option<String> {
        let path_parsed = Self::parse_path(path)?;
        let old_parsed = Self::parse_path(old_prefix)?;
        let new_parsed = Self::parse_path(new_prefix)?;
        if path_parsed == old_parsed {
            return Some(new_parsed.to_string());
        }
        let path_segments = path_parsed.segments();
        let old_segments = old_parsed.segments();
        if path_segments.len() <= old_segments.len()
            || path_segments[..old_segments.len()] != *old_segments
        {
            return Some(path.to_string());
        }
        let mut out = new_parsed.segments().to_vec();
        out.extend_from_slice(&path_segments[old_segments.len()..]);
        Some(ValuePath::new(out).to_string())
    }

    pub(super) fn remap_expanded_prefix(&mut self, old_prefix: &str, new_prefix: &str) {
        let mut next = HashSet::new();
        for path in &self.expanded {
            if let Some(remapped) = Self::rebase_path(path.as_str(), old_prefix, new_prefix) {
                next.insert(remapped);
            }
        }
        self.expanded = next;
    }

    pub(super) fn remap_array_name_prefix(&mut self, old_prefix: &str, new_prefix: &str) {
        let mut next = HashMap::new();
        for (path, name) in &self.array_item_names {
            if let Some(remapped) = Self::rebase_path(path.as_str(), old_prefix, new_prefix) {
                next.insert(remapped, name.clone());
            }
        }
        self.array_item_names = next;
    }

    pub(super) fn remove_array_name_subtree(&mut self, prefix: &str) {
        self.array_item_names
            .retain(|path, _| path != prefix && !Self::is_descendant_path(path, prefix));
    }

    pub(super) fn take_node_at_path(&mut self, path: &str) -> Option<(String, bool, Value)> {
        let parent_path = Self::parent_path(path);
        let key = Self::leaf_key(path);
        let parent = Self::value_at_path_mut(&mut self.value, &parent_path)?;
        match parent {
            Value::Object(map) => map.shift_remove(&key).map(|value| (key, false, value)),
            Value::List(arr) => {
                let idx = key.parse::<usize>().ok()?;
                if idx < arr.len() {
                    Some((key, true, arr.remove(idx)))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(super) fn unique_key(map: &IndexMap<String, Value>, base: &str) -> String {
        if !map.contains_key(base) {
            return base.to_string();
        }
        let mut idx = 2usize;
        loop {
            let candidate = format!("{base}_{idx}");
            if !map.contains_key(&candidate) {
                return candidate;
            }
            idx += 1;
        }
    }

    pub(super) fn insert_node_under_parent(
        &mut self,
        parent_path: &str,
        key: String,
        was_index: bool,
        source_name: Option<String>,
        value: Value,
        placement: InsertPlacement,
        source_parent: &str,
    ) -> Option<String> {
        let parent = Self::value_at_path_mut(&mut self.value, parent_path)?;
        match parent {
            Value::Object(map) => {
                let insert_idx = match placement {
                    InsertPlacement::Start => 0,
                    InsertPlacement::End => map.len(),
                    InsertPlacement::Before(ref anchor) => {
                        map.get_index_of(anchor).unwrap_or(map.len())
                    }
                    InsertPlacement::After(ref anchor) => map
                        .get_index_of(anchor)
                        .map(|idx| idx + 1)
                        .unwrap_or(map.len()),
                }
                .min(map.len());
                let final_key = if was_index {
                    Self::unique_key(map, source_name.as_deref().unwrap_or("item"))
                } else {
                    Self::unique_key(map, &key)
                };
                map.shift_insert(insert_idx, final_key.clone(), value);
                Some(Self::append_key(parent_path, &final_key))
            }
            Value::List(arr) => {
                let mut idx = match placement {
                    InsertPlacement::Start => 0,
                    InsertPlacement::End => arr.len(),
                    InsertPlacement::Before(ref anchor) => {
                        anchor.parse::<usize>().unwrap_or(arr.len())
                    }
                    InsertPlacement::After(ref anchor) => anchor
                        .parse::<usize>()
                        .ok()
                        .map(|idx| idx + 1)
                        .unwrap_or(arr.len()),
                };
                if source_parent == parent_path
                    && let Ok(src_idx) = key.parse::<usize>()
                    && src_idx < idx
                {
                    idx = idx.saturating_sub(1);
                }
                idx = idx.min(arr.len());
                arr.insert(idx, value);
                Some(Self::append_index(parent_path, idx))
            }
            _ => None,
        }
    }

    pub fn parse_scalar(s: &str) -> Value {
        let s = s.trim();
        if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
            return Value::Text(s[1..s.len() - 1].to_string());
        }
        if s == "null" {
            return Value::None;
        }
        if s == "true" {
            return Value::Bool(true);
        }
        if s == "false" {
            return Value::Bool(false);
        }
        if let Ok(n) = s.parse::<f64>() {
            return Value::Number(n);
        }
        Value::Text(s.to_string())
    }
}
