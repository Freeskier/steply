use crate::state::app_state::AppState;
use crate::terminal::terminal::{CursorPos, TerminalSize};
use crate::ui::layout::Layout;
use crate::ui::span::{Span, SpanLine};
use crate::ui::style::{Color, Style};
use crate::widgets::traits::RenderContext;

#[derive(Debug, Default, Clone)]
pub struct RenderFrame {
    pub lines: Vec<SpanLine>,
    pub cursor: Option<CursorPos>,
}

pub struct Renderer;

impl Renderer {
    pub fn render(state: &AppState, terminal_size: TerminalSize) -> RenderFrame {
        let mut frame = RenderFrame::default();
        let ctx = RenderContext {
            focused_id: state.focus.current_id().map(ToOwned::to_owned),
            terminal_size,
        };
        let mut row_offset: u16 = 0;

        let title_style = Style::new().color(Color::Cyan);
        frame.lines.push(vec![Span::styled(
            format!("{} [{}]", state.current_prompt(), state.current_step_id()),
            title_style,
        )]);
        row_offset = row_offset.saturating_add(1);
        if let Some(hint) = state.current_hint() {
            let hint_style = Style::new().color(Color::Yellow);
            frame
                .lines
                .push(vec![Span::styled(format!("Hint: {}", hint), hint_style)]);
            row_offset = row_offset.saturating_add(1);
        }

        for node in state.active_nodes() {
            let out = node.draw(&ctx);
            if frame.cursor.is_none()
                && ctx
                    .focused_id
                    .as_deref()
                    .is_some_and(|focused| focused == node.id())
            {
                if let Some(local_cursor) = node.cursor_pos() {
                    frame.cursor = Some(CursorPos {
                        col: local_cursor.col,
                        row: row_offset.saturating_add(local_cursor.row),
                    });
                }
            }
            row_offset = row_offset.saturating_add(out.lines.len() as u16);
            frame.lines.extend(out.lines);

            if let Some(error) = state.visible_error(node.id()) {
                let error_style = Style::new().color(Color::Red);
                frame
                    .lines
                    .push(vec![Span::styled(format!("  ! {}", error), error_style)]);
                row_offset = row_offset.saturating_add(1);
            }
        }

        frame.lines = Layout::compose(&frame.lines, terminal_size.width);
        frame
    }
}
