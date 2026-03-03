use crate::terminal::TerminalSize;
use crate::ui::renderer::RenderFrame;
use crate::ui::span::SpanLine;
use crate::ui::style::{Color, Strike};
use crate::widgets::traits::DrawOutput;
use crate::widgets::traits::StickyPosition;

pub fn frame_to_json(frame: &RenderFrame, size: TerminalSize) -> serde_json::Value {
    let cursor = frame.cursor.map(|c| {
        serde_json::json!({
            "row": c.row,
            "col": c.col,
        })
    });

    let lines = frame
        .lines
        .iter()
        .map(|line| {
            serde_json::Value::Array(
                line.iter()
                    .map(|span| {
                        serde_json::json!({
                            "text": span.text,
                            "wrap_mode": match span.wrap_mode {
                                crate::ui::span::WrapMode::NoWrap => "no_wrap",
                                crate::ui::span::WrapMode::Wrap => "wrap",
                            },
                            "style": {
                                "color": span.style.color.map(color_to_json),
                                "background": span.style.background.map(color_to_json),
                                "bold": span.style.bold,
                                "strike": match span.style.strike {
                                    Strike::Inherit => "inherit",
                                    Strike::On => "on",
                                    Strike::Off => "off",
                                },
                            }
                        })
                    })
                    .collect(),
            )
        })
        .collect::<Vec<_>>();

    let sticky = frame
        .sticky
        .iter()
        .map(|block| {
            let lines = block
                .lines
                .iter()
                .map(|line| {
                    serde_json::Value::Array(
                        line.iter()
                            .map(|span| {
                                serde_json::json!({
                                    "text": span.text,
                                    "wrap_mode": match span.wrap_mode {
                                        crate::ui::span::WrapMode::NoWrap => "no_wrap",
                                        crate::ui::span::WrapMode::Wrap => "wrap",
                                    },
                                    "style": {
                                        "color": span.style.color.map(color_to_json),
                                        "background": span.style.background.map(color_to_json),
                                        "bold": span.style.bold,
                                        "strike": match span.style.strike {
                                            Strike::Inherit => "inherit",
                                            Strike::On => "on",
                                            Strike::Off => "off",
                                        },
                                    }
                                })
                            })
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "position": match block.position {
                    StickyPosition::Top => "top",
                    StickyPosition::Bottom => "bottom",
                },
                "priority": block.priority,
                "lines": lines,
            })
        })
        .collect::<Vec<_>>();

    let active_step_range = frame.active_step_range.map(|range| {
        serde_json::json!({
            "start": range.start,
            "end_exclusive": range.end_exclusive,
        })
    });

    serde_json::json!({
        "terminal": {
            "width": size.width,
            "height": size.height,
        },
        "cursor": cursor,
        "focus_anchor_row": frame.focus_anchor_row,
        "focus_anchor_col": frame.focus_anchor_col,
        "active_step_range": active_step_range,
        "cursor_visible": frame.cursor_visible,
        "lines": lines,
        "sticky": sticky,
    })
}

pub fn draw_output_to_json(output: &DrawOutput, size: TerminalSize) -> serde_json::Value {
    serde_json::json!({
        "terminal": {
            "width": size.width,
            "height": size.height,
        },
        "cursor": serde_json::Value::Null,
        "focus_anchor_row": serde_json::Value::Null,
        "focus_anchor_col": serde_json::Value::Null,
        "active_step_range": serde_json::Value::Null,
        "cursor_visible": false,
        "lines": lines_to_json(output.lines.as_slice()),
        "sticky": sticky_to_json(output.sticky.as_slice()),
    })
}

fn lines_to_json(lines: &[SpanLine]) -> Vec<serde_json::Value> {
    lines
        .iter()
        .map(|line| {
            serde_json::Value::Array(
                line.iter()
                    .map(|span| {
                        serde_json::json!({
                            "text": span.text,
                            "wrap_mode": match span.wrap_mode {
                                crate::ui::span::WrapMode::NoWrap => "no_wrap",
                                crate::ui::span::WrapMode::Wrap => "wrap",
                            },
                            "style": {
                                "color": span.style.color.map(color_to_json),
                                "background": span.style.background.map(color_to_json),
                                "bold": span.style.bold,
                                "strike": match span.style.strike {
                                    Strike::Inherit => "inherit",
                                    Strike::On => "on",
                                    Strike::Off => "off",
                                },
                            }
                        })
                    })
                    .collect(),
            )
        })
        .collect::<Vec<_>>()
}

fn sticky_to_json(sticky: &[crate::widgets::traits::StickyBlock]) -> Vec<serde_json::Value> {
    sticky
        .iter()
        .map(|block| {
            serde_json::json!({
                "position": match block.position {
                    StickyPosition::Top => "top",
                    StickyPosition::Bottom => "bottom",
                },
                "priority": block.priority,
                "lines": lines_to_json(block.lines.as_slice()),
            })
        })
        .collect::<Vec<_>>()
}

fn color_to_json(color: Color) -> serde_json::Value {
    match color {
        Color::Reset => serde_json::json!("reset"),
        Color::Black => serde_json::json!("black"),
        Color::DarkGrey => serde_json::json!("dark_grey"),
        Color::Red => serde_json::json!("red"),
        Color::Green => serde_json::json!("green"),
        Color::Yellow => serde_json::json!("yellow"),
        Color::Blue => serde_json::json!("blue"),
        Color::Magenta => serde_json::json!("magenta"),
        Color::Cyan => serde_json::json!("cyan"),
        Color::White => serde_json::json!("white"),
        Color::Rgb(r, g, b) => serde_json::json!({
            "rgb": [r, g, b]
        }),
    }
}
