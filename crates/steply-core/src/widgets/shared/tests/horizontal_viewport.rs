use super::render_single_line;
use crate::terminal::CursorPos;
use crate::ui::span::Span;

#[test]
fn clips_long_line_with_left_and_right_overflow_indicators() {
    let rendered = render_single_line(
        &[Span::new("abcdefghijklmnopqrstuvwxyz").no_wrap()],
        8,
        Some((10, 11)),
        Some(10),
    );
    let text = rendered
        .spans
        .iter()
        .map(|span| span.text.as_str())
        .collect::<String>();

    assert!(text.starts_with('…'));
    assert!(text.ends_with('…'));
    assert_eq!(rendered.cursor, Some(CursorPos { col: 6, row: 0 }));
}

#[test]
fn keeps_short_line_unchanged_without_indicators() {
    let rendered = render_single_line(&[Span::new("short").no_wrap()], 10, None, None);
    let text = rendered
        .spans
        .iter()
        .map(|span| span.text.as_str())
        .collect::<String>();

    assert_eq!(text, "short");
    assert_eq!(rendered.cursor, None);
}
