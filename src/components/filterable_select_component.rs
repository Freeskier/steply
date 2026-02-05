use crate::components::select_component::{SelectComponent, SelectMode, SelectOption};
use crate::core::component::{Component, ComponentBase, EventContext, FocusMode};
use crate::core::search::{autocomplete, fuzzy};
use crate::core::value::Value;
use crate::inputs::Input;
use crate::inputs::text_input::TextInput;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::ui::render::{RenderContext, RenderLine};

pub struct FilterableSelectComponent {
    base: ComponentBase,
    filter_input: TextInput,
    select: SelectComponent,
    all_options: Vec<String>,
    matches: Vec<fuzzy::FuzzyMatch>,
}

impl FilterableSelectComponent {
    pub fn new(id: impl Into<String>, options: Vec<String>) -> Self {
        let id = id.into();
        let filter_input = TextInput::new(format!("{}_filter", id), "Filter");
        let select = SelectComponent::new(format!("{}_list", id), options.clone())
            .with_mode(SelectMode::List);
        let mut component = Self {
            base: ComponentBase::new(id),
            filter_input,
            select,
            all_options: options,
            matches: Vec::new(),
        };
        component.refresh_matches();
        component
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.filter_input.base_mut_ref().label = label.into();
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.filter_input = self.filter_input.with_placeholder(placeholder);
        self
    }

    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.set_options(options);
        self
    }

    pub fn set_options(&mut self, options: Vec<String>) {
        self.all_options = options;
        self.refresh_matches();
    }

    pub fn filter_value(&self) -> String {
        self.filter_input.value()
    }

    fn refresh_matches(&mut self) {
        let query = self.filter_input.value();
        let matches = fuzzy::match_candidates(&query, &self.all_options);
        let mut filtered = Vec::with_capacity(matches.len());
        for item in &matches {
            if let Some(value) = self.all_options.get(item.index) {
                filtered.push(SelectOption::Highlighted {
                    text: value.clone(),
                    highlights: item.ranges.clone(),
                });
            }
        }

        self.matches = matches;
        self.select.set_options(filtered);
        self.select.reset_active();
    }

    fn accept_autocomplete(&mut self) -> bool {
        let query = self.filter_input.value();
        let suggestion = autocomplete::suggest(&query, &self.matches, &self.all_options);
        let Some(suggestion) = suggestion else {
            return false;
        };

        if suggestion == query {
            return false;
        }

        self.filter_input.set_value(suggestion);
        self.refresh_matches();
        true
    }
}

impl Component for FilterableSelectComponent {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ComponentBase {
        &mut self.base
    }

    fn focus_mode(&self) -> FocusMode {
        FocusMode::Group
    }

    fn render(&self, ctx: &RenderContext) -> Vec<RenderLine> {
        let mut lines = Vec::new();

        let inline_error = self.filter_input.has_visible_error();
        let (spans, cursor_offset) =
            ctx.render_input_full(&self.filter_input, inline_error, self.base.focused);
        lines.push(RenderLine {
            spans,
            cursor_offset,
        });

        lines.extend(self.select.render(ctx));
        lines
    }

    fn value(&self) -> Option<Value> {
        self.select.value()
    }

    fn set_value(&mut self, value: Value) {
        match value {
            Value::List(items) => self.set_options(items),
            Value::Text(filter) => {
                self.filter_input.set_value(filter);
                self.refresh_matches();
            }
            _ => {}
        }
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        ctx: &mut EventContext,
    ) -> bool {
        if modifiers == KeyModifiers::NONE {
            let handled = match code {
                KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.select.handle_key(code, modifiers, ctx)
                }
                _ => false,
            };
            if handled {
                return true;
            }
        }

        if modifiers == KeyModifiers::NONE {
            if code == KeyCode::Right {
                let at_end =
                    self.filter_input.cursor_pos() >= self.filter_input.value().chars().count();
                if at_end && self.accept_autocomplete() {
                    ctx.handled();
                    return true;
                }
            }
        }

        let before = self.filter_input.value();
        let result = self.filter_input.handle_key(code, modifiers);
        let after = self.filter_input.value();

        if before != after {
            self.refresh_matches();
            ctx.handled();
            return true;
        }

        match result {
            crate::inputs::KeyResult::Submit => {
                ctx.submit();
                true
            }
            crate::inputs::KeyResult::Handled => {
                ctx.handled();
                true
            }
            crate::inputs::KeyResult::NotHandled => false,
        }
    }

    fn set_focused(&mut self, focused: bool) {
        self.base.focused = focused;
        self.filter_input.set_focused(focused);
        self.select.set_focused(focused);
    }
}
