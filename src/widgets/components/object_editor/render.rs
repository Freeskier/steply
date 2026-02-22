use super::*;

impl ObjectEditor {
    fn value_display(val: &Value) -> (String, Style) {
        match val {
            Value::Text(s) => (s.clone(), Style::new().color(Color::Green)),
            Value::Number(n) => {
                let s = if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                };
                (s, Style::new().color(Color::Cyan))
            }
            Value::Bool(b) => (b.to_string(), Style::new().color(Color::Yellow)),
            Value::None => ("null".to_string(), Style::new().color(Color::DarkGrey)),
            Value::Object(m) => (
                format!("{{{}}}", m.len()),
                Style::new().color(Color::DarkGrey),
            ),
            Value::List(a) => (
                format!("[{}]", a.len()),
                Style::new().color(Color::DarkGrey),
            ),
        }
    }

    fn row_spans(
        &self,
        vis: usize,
        obj: &ObjNode,
        red: bool,
        yellow: bool,
        focused: bool,
    ) -> Vec<Span> {
        let red_st = Style::new().color(Color::Red);
        let yellow_st = Style::new().color(Color::Yellow);
        let key_st = Style::new().color(Color::White).bold();
        let key_dim = Style::new().color(Color::DarkGrey);
        let cyan_st = Style::new().color(Color::Cyan);
        let highlight_st = Style::new().color(Color::Yellow).bold();
        let query = self.tree.filter_query().trim();

        if let Mode::EditKey { vis: ev, key_value } | Mode::EditValue { vis: ev, key_value } =
            &self.mode
            && *ev == vis
        {
            return key_value.inline_spans();
        }

        if obj.is_placeholder {
            let style = if red {
                red_st
            } else if yellow {
                yellow_st
            } else {
                key_dim
            };
            return vec![Span::styled(obj.key.clone(), style).no_wrap()];
        }

        let key_style = if red {
            red_st
        } else if yellow {
            yellow_st
        } else if obj.is_index {
            key_dim
        } else {
            key_st
        };

        let mut key_part = if query.is_empty() {
            vec![Span::styled(obj.key.clone(), key_style).no_wrap()]
        } else {
            let ranges = match_text(query, obj.key.as_str())
                .map(|(_, ranges)| ranges)
                .unwrap_or_default();
            render_text_spans(obj.key.as_str(), ranges.as_slice(), key_style, highlight_st)
        };
        key_part.push(Span::styled(":", key_style).no_wrap());

        if let Mode::ConfirmDelete { vis: dv, select } = &self.mode
            && *dv == vis
        {
            let selected = select
                .value()
                .and_then(|v| v.to_text_scalar())
                .unwrap_or_else(|| "No".to_string());
            let mut spans = key_part;
            spans.push(Span::new(" ").no_wrap());
            spans.push(Span::styled("Delete? ", red_st).no_wrap());
            spans.push(
                Span::styled(
                    format!("‹ {selected} ›"),
                    if focused { cyan_st } else { key_dim },
                )
                .no_wrap(),
            );
            return spans;
        }

        let (text, style) = Self::value_display(&obj.value);
        let style = if red {
            red_st
        } else if yellow {
            yellow_st
        } else {
            style
        };
        let mut val_part = vec![Span::new(" ").no_wrap()];
        if query.is_empty() {
            val_part.push(Span::styled(text, style).no_wrap());
        } else {
            let ranges = match_text(query, text.as_str())
                .map(|(_, ranges)| ranges)
                .unwrap_or_default();
            val_part.extend(render_text_spans(
                text.as_str(),
                ranges.as_slice(),
                style,
                highlight_st,
            ));
        }

        let mut spans = key_part;
        spans.extend(val_part);
        spans
    }

    fn insert_value_spans(&self, key_value: &KeyValueComponent, error: Option<&str>) -> Vec<Span> {
        if let Some(error) = error {
            let key_style = if key_value.focus() == KeyValueFocus::Key {
                Style::new().color(Color::Cyan)
            } else {
                Style::new().color(Color::DarkGrey)
            };
            return vec![
                Span::styled(key_value.key(), key_style).no_wrap(),
                Span::new(": ").no_wrap(),
                Span::styled(format!("✗ {error}"), Style::new().color(Color::Red).bold()).no_wrap(),
            ];
        }
        key_value.inline_spans()
    }
}

impl Component for ObjectEditor {
    fn children(&self) -> &[Node] {
        &[]
    }
    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}

impl Drawable for ObjectEditor {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let inactive = Style::new().color(Color::DarkGrey);
        let insert_value_error = matches!(self.mode, Mode::InsertValue { .. })
            .then(|| ctx.visible_errors.get(self.base.id()).map(String::as_str))
            .flatten();

        let red_range: Option<std::ops::Range<usize>> = match &self.mode {
            Mode::ConfirmDelete { vis, .. } => Some(self.subtree_vis_range(*vis)),
            _ => None,
        };
        let yellow_range: Option<std::ops::Range<usize>> = match &self.mode {
            Mode::Move { vis } => Some(*vis..self.subtree_vis_range(*vis).end),
            _ => None,
        };

        let mut lines: Vec<Vec<Span>> = Vec::new();

        if !self.base.label().is_empty() {
            lines.push(vec![Span::new(self.base.label()).no_wrap()]);
        }

        if self.filter_visible {
            let filter_ctx = self.child_context(
                ctx,
                if focused && self.filter_focus {
                    Some(self.filter.id().to_string())
                } else {
                    None
                },
            );
            let mut filter_line =
                vec![Span::styled("Filter: ", Style::new().color(Color::DarkGrey)).no_wrap()];
            filter_line.extend(
                self.filter
                    .draw(&filter_ctx)
                    .lines
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| vec![Span::new("").no_wrap()]),
            );
            lines.push(filter_line);
        }

        let tree_lines = self.tree.render_lines(focused && !self.filter_focus);
        let (start, end) = self.tree.visible_range();
        let visible = self.tree.visible();
        let nodes = self.tree.nodes();

        for (line_idx, mut tree_line) in tree_lines.into_iter().enumerate() {
            if line_idx >= end.saturating_sub(start) {
                lines.push(tree_line);
                continue;
            }
            let vis = start + line_idx;

            if vis >= visible.len() {
                lines.push(tree_line);
                continue;
            }

            let node_idx = visible[vis];
            let obj = &nodes[node_idx].item;

            let in_red = red_range
                .as_ref()
                .map(|r| r.contains(&vis))
                .unwrap_or(false);
            let in_yellow = yellow_range
                .as_ref()
                .map(|r| r.contains(&vis))
                .unwrap_or(false);

            let icon_idx = Self::tree_content_start(&tree_line);
            let insert_prefix = Self::tree_insert_prefix(&tree_line);

            let insert_after_this_row = matches!(
                self.mode,
                Mode::InsertType { after_vis, .. } | Mode::InsertValue { after_vis, .. }
                    if after_vis == vis
            );
            let insert_inline_on_placeholder = insert_after_this_row && obj.is_placeholder;
            if insert_inline_on_placeholder {
                let mut row = insert_prefix.clone();
                match &self.mode {
                    Mode::InsertType { key_value, .. } => {
                        row.extend(key_value.inline_spans());
                    }
                    Mode::InsertValue { key_value, .. } => {
                        row.extend(self.insert_value_spans(key_value, insert_value_error));
                        row.push(Span::styled("  Enter confirm  Esc cancel", inactive).no_wrap());
                    }
                    _ => {}
                }
                lines.push(row);
                continue;
            }
            if insert_after_this_row && !tree_line.is_empty() {
                tree_line[0] = Span::styled(" ", Style::new().color(Color::DarkGrey)).no_wrap();
            }
            tree_line.truncate(icon_idx);

            if in_red || in_yellow {
                let tint = if in_red {
                    Style::new().color(Color::Red)
                } else {
                    Style::new().color(Color::Yellow)
                };
                for span in tree_line.iter_mut() {
                    if !span.text.trim().is_empty() {
                        span.style = tint;
                    }
                }
            }

            tree_line.extend(self.row_spans(vis, obj, in_red, in_yellow, focused));
            lines.push(tree_line);

            if let Mode::InsertType {
                after_vis,
                key_value,
            } = &self.mode
            {
                if *after_vis == vis {
                    let mut row = insert_prefix.clone();
                    row.extend(key_value.inline_spans());
                    lines.push(row);
                }
            }

            if let Mode::InsertValue {
                after_vis,
                key_value,
                ..
            } = &self.mode
            {
                if *after_vis == vis {
                    let mut row = insert_prefix.clone();
                    row.extend(self.insert_value_spans(key_value, insert_value_error));
                    row.push(Span::styled("  Enter confirm  Esc cancel", inactive).no_wrap());
                    lines.push(row);
                }
            }
        }

        if focused {
            let hint = match &self.mode {
                Mode::Normal if self.filter_focus => "  Type to filter  Enter/Esc back to tree",
                Mode::Normal => {
                    "  ↑↓ nav  Space expand  e edit val  r rename  i insert  d delete  m move"
                }
                Mode::EditValue { .. } | Mode::EditKey { .. } => {
                    "  Enter confirm  Tab key↔val  Esc cancel"
                }
                Mode::InsertType { .. } => "  Tab key↔type  ←→ type  Enter confirm  Esc cancel",
                Mode::InsertValue { .. } => "  Enter confirm  Tab key↔val  Esc cancel",
                Mode::ConfirmDelete { .. } => "  ←→ No/Yes  Enter confirm",
                Mode::Move { .. } => "  ↑↓ move  m or Esc done",
            };
            lines.push(vec![Span::styled(hint, inactive).no_wrap()]);
        }

        DrawOutput { lines }
    }
}
