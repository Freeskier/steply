use crate::core::NodeId;
use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::terminal::{CursorPos, KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::node::{Component, Node};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};




#[derive(Debug, Clone)]
enum Chunk {

    Text(String),

    Slot(String),
}

fn parse_template(template: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut rest = template;
    while !rest.is_empty() {
        if let Some(open) = rest.find('<') {
            if open > 0 {
                chunks.push(Chunk::Text(rest[..open].to_string()));
            }
            let after_open = &rest[open + 1..];
            if let Some(close) = after_open.find('>') {
                let key = after_open[..close].trim().to_string();
                chunks.push(Chunk::Slot(key));
                rest = &after_open[close + 1..];
            } else {

                chunks.push(Chunk::Text(rest.to_string()));
                break;
            }
        } else {
            chunks.push(Chunk::Text(rest.to_string()));
            break;
        }
    }
    chunks
}



pub struct Snippet {
    base: WidgetBase,
    chunks: Vec<Chunk>,

    inputs: Vec<Node>,

    slot_order: Vec<String>,

    active_slot: usize,
    submit_target: Option<ValueTarget>,
}

impl Snippet {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        let template = template.into();
        let chunks = parse_template(&template);


        let mut slot_order: Vec<String> = Vec::new();
        for chunk in &chunks {
            if let Chunk::Slot(key) = chunk {
                if !slot_order.contains(key) {
                    slot_order.push(key.clone());
                }
            }
        }

        Self {
            base: WidgetBase::new(id, label),
            chunks,
            inputs: Vec::new(),
            slot_order,
            active_slot: 0,
            submit_target: None,
        }
    }

    pub fn with_input(mut self, node: Node) -> Self {
        self.inputs.push(node);
        self
    }

    pub fn with_submit_target(mut self, target: impl Into<NodeId>) -> Self {
        self.submit_target = Some(ValueTarget::node(target));
        self
    }

    pub fn with_submit_target_path(mut self, root: impl Into<NodeId>, path: ValuePath) -> Self {
        self.submit_target = Some(ValueTarget::path(root, path));
        self
    }



    fn input_for(&self, key: &str) -> Option<&Node> {
        self.inputs.iter().find(|n| n.id() == key)
    }

    fn input_for_mut(&mut self, key: &str) -> Option<&mut Node> {
        self.inputs.iter_mut().find(|n| n.id() == key)
    }

    fn active_key(&self) -> Option<&str> {
        self.slot_order.get(self.active_slot).map(String::as_str)
    }

    fn slot_count(&self) -> usize {
        self.slot_order.len()
    }

    fn value_of(&self, key: &str) -> String {
        self.input_for(key)
            .and_then(|n| n.value())
            .and_then(|v| {
                if let Value::Text(t) = v {
                    Some(t)
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }




    fn render_lines(
        &self,
        focused: bool,
        ctx: &RenderContext,
    ) -> (Vec<Vec<Span>>, Option<(u16, u16)>) {
        let dim = Style::new().color(Color::DarkGrey);
        let active_st = Style::new().color(Color::Cyan);
        let inactive_input_st = Style::new().color(Color::White);

        let mut lines: Vec<Vec<Span>> = Vec::new();
        let mut current_line: Vec<Span> = Vec::new();


        let mut cursor: Option<(u16, u16)> = None;
        let mut current_row: u16 = 0;


        let mut seen_slots: std::collections::HashSet<String> = std::collections::HashSet::new();

        for chunk in &self.chunks {
            match chunk {
                Chunk::Text(text) => {

                    let mut first = true;
                    for part in text.split('\n') {
                        if !first {
                            lines.push(std::mem::take(&mut current_line));
                            current_row += 1;
                        }
                        first = false;
                        if !part.is_empty() {
                            current_line.push(Span::new(part.to_string()).no_wrap());
                        }
                    }
                }

                Chunk::Slot(key) => {
                    let is_active = focused && self.active_key() == Some(key.as_str());
                    let is_first = !seen_slots.contains(key);
                    seen_slots.insert(key.clone());

                    if is_first {

                        if let Some(input) = self.input_for(key) {


                            let input_ctx = if is_active {
                                RenderContext {
                                    focused_id: Some(input.id().to_string()),
                                    ..ctx.clone()
                                }
                            } else {
                                RenderContext {
                                    focused_id: None,
                                    ..ctx.clone()
                                }
                            };

                            let out = input.draw(&input_ctx);


                            if let Some(input_line) = out.lines.into_iter().next() {





                                let col_before = col_width(&current_line);
                                let st = if is_active {
                                    active_st
                                } else {
                                    inactive_input_st
                                };
                                for span in input_line {
                                    current_line.push(span.with_style_if_unstyled(st));
                                }


                                if is_active {
                                    if let Some(input_node) = self.input_for(key) {
                                        if let Some(cp) = input_node.cursor_pos() {
                                            cursor = Some((
                                                current_row + cp.row,
                                                col_before as u16 + cp.col,
                                            ));
                                        }
                                    }
                                }
                            }
                        } else {

                            let st = if is_active { active_st } else { dim };
                            current_line.push(Span::styled(format!("<{}>", key), st).no_wrap());
                        }
                    } else {

                        let val = self.value_of(key);
                        let display = if val.is_empty() {
                            format!("<{}>", key)
                        } else {
                            val
                        };
                        current_line.push(Span::styled(display, dim).no_wrap());
                    }
                }
            }
        }

        if !current_line.is_empty() || lines.is_empty() {
            lines.push(current_line);
        }

        (lines, cursor)
    }
}



fn col_width(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.text.chars().count()).sum()
}



trait SpanExt {
    fn with_style_if_unstyled(self, st: Style) -> Span;
}

impl SpanExt for Span {
    fn with_style_if_unstyled(self, st: Style) -> Span {
        if self.style == Style::default() {
            Span { style: st, ..self }
        } else {
            self
        }
    }
}



impl Drawable for Snippet {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let (lines, _cursor) = self.render_lines(focused, ctx);
        DrawOutput { lines }
    }
}



impl Interactive for Snippet {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn cursor_pos(&self) -> Option<CursorPos> {
        let key = self.active_key()?;
        let input = self.input_for(key)?;
        let local = input.cursor_pos()?;


        let mut row: u16 = 0;
        let mut col: u16 = 0;
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for chunk in &self.chunks {
            match chunk {
                Chunk::Text(text) => {
                    for ch in text.chars() {
                        if ch == '\n' {
                            row += 1;
                            col = 0;
                        } else {
                            col += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
                        }
                    }
                }
                Chunk::Slot(k) => {
                    if k == key && !seen.contains(k.as_str()) {
                        return Some(CursorPos {
                            row: row + local.row,
                            col: col + local.col,
                        });
                    }
                    seen.insert(k);
                    let val = self.value_of(k);
                    col += val.chars().count() as u16;
                }
            }
        }
        None
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Tab => {
                if self.slot_count() == 0 {
                    return InteractionResult::ignored();
                }
                if shift {
                    self.active_slot =
                        (self.active_slot + self.slot_count() - 1) % self.slot_count();
                } else {
                    self.active_slot = (self.active_slot + 1) % self.slot_count();
                }
                InteractionResult::handled()
            }

            KeyCode::Enter => {

                if self.active_slot + 1 >= self.slot_count() {
                    let val = Value::Text(self.formatted_value());
                    InteractionResult::submit_or_produce(self.submit_target.as_ref(), val)
                } else {
                    self.active_slot += 1;
                    InteractionResult::handled()
                }
            }

            _ => {

                if let Some(key_str) = self.active_key().map(str::to_string) {
                    if let Some(input) = self.input_for_mut(&key_str) {
                        return input.on_key(key);
                    }
                }
                InteractionResult::ignored()
            }
        }
    }

    fn value(&self) -> Option<Value> {
        let v = self.formatted_value();
        if v.is_empty() {
            None
        } else {
            Some(Value::Text(v))
        }
    }

    fn set_value(&mut self, _value: Value) {}

    fn validate(&self, mode: ValidationMode) -> Result<(), String> {
        for input in &self.inputs {
            input.validate(mode)?;
        }
        Ok(())
    }
}

impl Snippet {
    fn formatted_value(&self) -> String {

        let mut out = String::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for chunk in &self.chunks {
            match chunk {
                Chunk::Text(t) => out.push_str(t),
                Chunk::Slot(key) => {
                    seen.insert(key.clone());
                    out.push_str(&self.value_of(key));
                }
            }
        }
        out
    }
}



impl Component for Snippet {
    fn children(&self) -> &[Node] {
        &self.inputs
    }
    fn children_mut(&mut self) -> &mut [Node] {
        &mut self.inputs
    }
}
