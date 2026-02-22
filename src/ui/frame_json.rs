use crate::terminal::TerminalSize;
use crate::ui::renderer::RenderFrame;
use crate::ui::style::{Color, Strike};

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

    serde_json::json!({
        "terminal": {
            "width": size.width,
            "height": size.height,
        },
        "cursor": cursor,
        "lines": lines,
    })
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
