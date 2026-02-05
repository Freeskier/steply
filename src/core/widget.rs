use crate::core::component::EventContext;
use crate::inputs::Input;
use crate::terminal::{KeyCode, KeyModifiers};

pub trait Widget {
    fn id(&self) -> &str;
    fn is_focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);
    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        ctx: &mut EventContext,
    ) -> bool;
}

impl Widget for dyn Input {
    fn id(&self) -> &str {
        Input::id(self)
    }

    fn is_focused(&self) -> bool {
        Input::is_focused(self)
    }

    fn set_focused(&mut self, focused: bool) {
        Input::set_focused(self, focused);
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        ctx: &mut EventContext,
    ) -> bool {
        Input::handle_key_with_context(self, code, modifiers, ctx)
    }
}

impl Widget for dyn crate::core::component::Component {
    fn id(&self) -> &str {
        crate::core::component::Component::id(self)
    }

    fn is_focused(&self) -> bool {
        crate::core::component::Component::is_focused(self)
    }

    fn set_focused(&mut self, focused: bool) {
        crate::core::component::Component::set_focused(self, focused);
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        ctx: &mut EventContext,
    ) -> bool {
        crate::core::component::Component::handle_key(self, code, modifiers, ctx)
    }
}
