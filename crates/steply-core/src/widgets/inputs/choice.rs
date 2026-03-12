use crate::core::value::Value;
use crate::terminal::{KeyCode, KeyEvent};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::shared::list_nav;
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, HintContext, HintItem, InteractionResult, Interactive,
    RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

pub struct ChoiceInput {
    base: WidgetBase,
    options: Vec<String>,
    selected: usize,
    show_bullets: bool,
    validators: Vec<Validator>,
}

impl ChoiceInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            base: WidgetBase::new(id, label),
            options,
            selected: 0,
            show_bullets: true,
            validators: Vec::new(),
        }
    }

    pub fn with_bullets(mut self, enabled: bool) -> Self {
        self.show_bullets = enabled;
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn with_default(mut self, value: impl Into<Value>) -> Self {
        self.set_value(value.into());
        self
    }

    fn selected_text(&self) -> &str {
        self.options
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }

    fn select_by_letter(&mut self, ch: char) -> bool {
        let needle = ch.to_ascii_lowercase();
        if let Some(index) = self.options.iter().position(|opt| {
            opt.chars()
                .next()
                .map(|c| c.to_ascii_lowercase() == needle)
                .unwrap_or(false)
        }) {
            self.selected = index;
            return true;
        }
        false
    }

    fn clamp_selected(&mut self) {
        if self.options.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.options.len() {
            self.selected = self.options.len() - 1;
        }
    }

    pub fn set_options(&mut self, options: Vec<String>) {
        self.options = options;
        self.clamp_selected();
    }
}

impl Drawable for ChoiceInput {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn label(&self) -> &str {
        self.base.label()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);

        let spans = if focused {
            let active_style = Style::new().color(Color::Cyan).bold();
            let inactive_style = Style::new().color(Color::DarkGrey);
            let mut s = vec![];
            for (index, option) in self.options.iter().enumerate() {
                if index > 0 {
                    s.push(Span::new(" / ").no_wrap());
                }
                if self.show_bullets {
                    if index == self.selected {
                        s.push(
                            Span::styled("●", Style::new().color(Color::Green).bold()).no_wrap(),
                        );
                    } else {
                        s.push(Span::styled("○", inactive_style).no_wrap());
                    }
                    s.push(Span::new(" ").no_wrap());
                }
                s.push(
                    Span::styled(
                        option.clone(),
                        if index == self.selected {
                            active_style
                        } else {
                            inactive_style
                        },
                    )
                    .no_wrap(),
                );
            }
            s
        } else {
            vec![Span::new(self.selected_text().to_string()).no_wrap()]
        };

        DrawOutput::with_lines(vec![spans])
    }

    fn hints(&self, ctx: HintContext) -> Vec<HintItem> {
        crate::widgets::traits::focused_static_hints(
            ctx,
            crate::widgets::static_hints::CHOICE_INPUT_HINTS,
        )
    }
}

impl Interactive for ChoiceInput {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        match key.code {
            KeyCode::Left | KeyCode::Up => InteractionResult::handled_if(
                list_nav::apply_cycle_index(&mut self.selected, self.options.len(), true),
            ),
            KeyCode::Right | KeyCode::Down => InteractionResult::handled_if(
                list_nav::apply_cycle_index(&mut self.selected, self.options.len(), false),
            ),
            KeyCode::Char(ch) => {
                if self.select_by_letter(ch) {
                    InteractionResult::handled()
                } else {
                    InteractionResult::ignored()
                }
            }
            KeyCode::Enter => InteractionResult::input_done(),
            _ => InteractionResult::ignored(),
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Text(self.selected_text().to_string()))
    }

    fn set_value(&mut self, value: Value) {
        if let Some(text) = value.to_text_scalar()
            && let Some(pos) = self.options.iter().position(|opt| opt == &text)
        {
            self.selected = pos;
        }
    }

    fn set_options_from_value(&mut self, value: Value) -> bool {
        let Some(options) = value.as_list().map(|items| {
            items
                .iter()
                .filter_map(Value::to_text_scalar)
                .collect::<Vec<_>>()
        }) else {
            return false;
        };
        if self.options == options {
            return false;
        }
        self.set_options(options);
        true
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(
            &self.validators,
            &Value::Text(self.selected_text().to_string()),
        )
    }
}
