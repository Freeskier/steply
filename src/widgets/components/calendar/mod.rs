use crate::core::value::Value;
use crate::core::value_path::{ValuePath, ValueTarget};
use crate::core::NodeId;
use crate::terminal::{KeyCode, KeyEvent, KeyModifiers};
use crate::ui::span::Span;
use crate::ui::style::{Color, Style};
use crate::widgets::base::WidgetBase;
use crate::widgets::inputs::masked::MaskedInput;
use crate::widgets::node::{Component, Node};
use crate::widgets::shared::calendar::{self, Date, MonthGrid};
use crate::widgets::traits::{
    DrawOutput, Drawable, FocusMode, InteractionResult, Interactive, RenderContext, ValidationMode,
};
use crate::widgets::validators::{Validator, run_validators};

// ── Mode ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalendarMode {
    #[default]
    Date,
    Time,
    DateTime,
}

// ── Focus sections ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Month,
    Year,
    Grid,
    Time,
}

// ── Calendar ─────────────────────────────────────────────────────────────

pub struct Calendar {
    base: WidgetBase,
    mode: CalendarMode,

    selected_date: Option<Date>,

    view_year: i32,
    view_month: u8,
    cursor_day: u8,

    section: Section,

    /// Internal masked input for time ("HH:mm:ss")
    time_input: MaskedInput,

    validators: Vec<Validator>,
    submit_target: Option<ValueTarget>,
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl Calendar {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let today = calendar::today();
        let id = id.into();
        let time_id = format!("{}_time", id);
        Self {
            base: WidgetBase::new(id, label),
            mode: CalendarMode::Date,
            selected_date: None,
            view_year: today.year,
            view_month: today.month,
            cursor_day: today.day,
            section: Section::Month,
            time_input: MaskedInput::new(time_id, "Time", "HH:mm:ss"),
            validators: Vec::new(),
            submit_target: None,
        }
    }

    pub fn with_mode(mut self, mode: CalendarMode) -> Self {
        self.mode = mode;
        if mode == CalendarMode::Time {
            self.section = Section::Time;
        }
        self
    }

    pub fn with_validator(mut self, v: Validator) -> Self {
        self.validators.push(v);
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

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn clamp_cursor_day(&mut self) {
        let max = calendar::days_in_month(self.view_year, self.view_month);
        if self.cursor_day < 1 {
            self.cursor_day = 1;
        }
        if self.cursor_day > max {
            self.cursor_day = max;
        }
    }

    fn month_delta(&mut self, delta: i32) {
        let total = self.view_month as i32 - 1 + delta;
        self.view_year += total.div_euclid(12);
        self.view_month = (total.rem_euclid(12) + 1) as u8;
        self.clamp_cursor_day();
    }

    fn year_delta(&mut self, delta: i32) {
        self.view_year += delta;
        self.clamp_cursor_day();
    }

    fn grid_move(&mut self, dr: i32, dc: i32) {
        let grid = MonthGrid::new(self.view_year, self.view_month);
        let (mut row, mut col) = self.day_grid_pos(&grid, self.cursor_day);
        let new_col = col as i32 + dc;
        let new_row = row as i32 + dr;
        if new_col >= 0 && new_col <= 6 && new_row >= 0 && new_row <= 5 {
            col = new_col as usize;
            row = new_row as usize;
            if let Some(day) = grid.cells[row][col] {
                self.cursor_day = day;
            }
        }
    }

    fn day_grid_pos(&self, grid: &MonthGrid, day: u8) -> (usize, usize) {
        for r in 0..6 {
            for c in 0..7 {
                if grid.cells[r][c] == Some(day) {
                    return (r, c);
                }
            }
        }
        (0, 0)
    }

    fn select_day(&mut self) {
        self.selected_date = Some(Date {
            year: self.view_year,
            month: self.view_month,
            day: self.cursor_day,
        });
        if self.mode == CalendarMode::DateTime {
            self.section = Section::Time;
        }
    }

    fn next_section(&self) -> Section {
        match self.mode {
            CalendarMode::Date => match self.section {
                Section::Month => Section::Year,
                Section::Year => Section::Grid,
                _ => Section::Month,
            },
            CalendarMode::Time => Section::Time,
            CalendarMode::DateTime => match self.section {
                Section::Month => Section::Year,
                Section::Year => Section::Grid,
                Section::Grid => Section::Time,
                Section::Time => Section::Month,
            },
        }
    }

    fn prev_section(&self) -> Section {
        match self.mode {
            CalendarMode::Date => match self.section {
                Section::Month => Section::Grid,
                Section::Year => Section::Month,
                _ => Section::Year,
            },
            CalendarMode::Time => Section::Time,
            CalendarMode::DateTime => match self.section {
                Section::Month => Section::Time,
                Section::Year => Section::Month,
                Section::Grid => Section::Year,
                Section::Time => Section::Grid,
            },
        }
    }

    fn formatted_value(&self) -> String {
        let time_str = self
            .time_input
            .value()
            .and_then(|v| {
                if let Value::Text(t) = v {
                    Some(t)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        match self.mode {
            CalendarMode::Date => self.selected_date.map(|d| d.to_iso()).unwrap_or_default(),
            CalendarMode::Time => time_str,
            CalendarMode::DateTime => match self.selected_date {
                Some(d) => format!("{} {}", d.to_iso(), time_str),
                None => String::new(),
            },
        }
    }

    fn is_time_section(&self) -> bool {
        self.section == Section::Time
    }
}

// ── Drawable ──────────────────────────────────────────────────────────────────

impl Drawable for Calendar {
    fn id(&self) -> &str {
        self.base.id()
    }

    fn draw(&self, ctx: &RenderContext) -> DrawOutput {
        let focused = self.base.is_focused(ctx);
        let mut lines: Vec<Vec<Span>> = Vec::new();

        let marker = if focused { ">" } else { " " };
        lines.push(vec![
            Span::new(format!("{} {}:", marker, self.base.label())).no_wrap(),
        ]);

        if self.mode != CalendarMode::Time {
            // ── Month / Year row ──────────────────────────────────────────────
            let month_st = if focused && self.section == Section::Month {
                Style::new().color(Color::Cyan)
            } else {
                Style::default()
            };
            let year_st = if focused && self.section == Section::Year {
                Style::new().color(Color::Cyan)
            } else {
                Style::default()
            };

            let month_name = MONTH_NAMES[(self.view_month as usize).saturating_sub(1) % 12];
            lines.push(vec![
                Span::styled(format!("  ‹ {:<9} ›", month_name), month_st).no_wrap(),
                Span::styled(format!("   ‹ {:4} ›", self.view_year), year_st).no_wrap(),
            ]);

            lines.push(vec![Span::new("").no_wrap()]);

            // ── Weekday header ────────────────────────────────────────────────
            lines.push(vec![
                Span::styled(
                    "  Mo  Tu  We  Th  Fr  Sa  Su",
                    Style::new().color(Color::DarkGrey),
                )
                .no_wrap(),
            ]);

            // ── Grid ──────────────────────────────────────────────────────────
            let grid = MonthGrid::new(self.view_year, self.view_month);
            let grid_focused = focused && self.section == Section::Grid;

            for row in &grid.cells {
                let has_any = row.iter().any(|c| c.is_some());
                if !has_any {
                    continue;
                }

                let mut line: Vec<Span> = vec![Span::new("  ").no_wrap()];
                for day in row.iter() {
                    match day {
                        None => line.push(Span::new("    ").no_wrap()),
                        Some(day) => {
                            let is_cursor = grid_focused && *day == self.cursor_day;
                            let is_selected = self
                                .selected_date
                                .map(|d| {
                                    d.year == self.view_year
                                        && d.month == self.view_month
                                        && d.day == *day
                                })
                                .unwrap_or(false);

                            let st = if is_cursor {
                                Style::new().color(Color::Yellow).bold()
                            } else if is_selected {
                                Style::new().color(Color::Cyan).bold()
                            } else {
                                Style::default()
                            };
                            let (l, r) = if is_cursor { ("[", "]") } else { (" ", " ") };
                            line.push(Span::styled(format!("{}{:2}{}", l, day, r), st).no_wrap());
                        }
                    }
                }
                lines.push(line);
            }

            lines.push(vec![Span::new("").no_wrap()]);
        }

        // ── Time row (DateTime or Time mode) ──────────────────────────────────
        if self.mode != CalendarMode::Date {
            let time_spans = self.time_input.render_spans();
            let dim = Style::new().color(Color::DarkGrey);
            let mut line: Vec<Span> = Vec::new();

            if self.mode == CalendarMode::DateTime {
                let date_str = self
                    .selected_date
                    .map(|d| format!("  {}   ", d.to_iso()))
                    .unwrap_or_else(|| "  pick a date   ".to_string());
                line.push(Span::styled(date_str, dim).no_wrap());
            } else {
                line.push(Span::new("  ").no_wrap());
            }

            line.extend(time_spans);
            lines.push(line);
        }

        DrawOutput { lines }
    }
}

// ── Interactive ───────────────────────────────────────────────────────────────

impl Interactive for Calendar {
    fn focus_mode(&self) -> FocusMode {
        FocusMode::Leaf
    }

    fn cursor_pos(&self) -> Option<crate::terminal::CursorPos> {
        if !self.is_time_section() {
            return None;
        }

        // Count lines drawn before the time row.
        let row_offset = if self.mode == CalendarMode::Time {
            1u16 // label only
        } else {
            // label + month/year + blank + weekday header + grid rows + blank
            let grid = MonthGrid::new(self.view_year, self.view_month);
            let grid_rows = grid
                .cells
                .iter()
                .filter(|r| r.iter().any(|c| c.is_some()))
                .count() as u16;
            1 + 1 + 1 + 1 + grid_rows + 1
        };

        // Col prefix before mask spans: DateTime = "  YYYY-MM-DD   " (16), Time = "  " (2).
        let col_offset = if self.mode == CalendarMode::DateTime {
            16u16
        } else {
            2u16
        };

        Some(crate::terminal::CursorPos {
            row: row_offset,
            col: col_offset + self.time_input.cursor_col() as u16,
        })
    }

    fn on_key(&mut self, key: KeyEvent) -> InteractionResult {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Delegate to time_input when in Time section
        if self.is_time_section() {
            match key.code {
                KeyCode::Tab => {
                    self.section = if shift {
                        self.prev_section()
                    } else {
                        self.next_section()
                    };
                    return InteractionResult::handled();
                }
                KeyCode::Enter => {
                    let val = Value::Text(self.formatted_value());
                    return InteractionResult::submit_or_produce(
                        self.submit_target.as_ref(),
                        val,
                    );
                }
                _ => return self.time_input.on_key(key),
            }
        }

        match key.code {
            KeyCode::Tab => {
                self.section = if shift {
                    self.prev_section()
                } else {
                    self.next_section()
                };
                InteractionResult::handled()
            }
            KeyCode::Left => match self.section {
                Section::Month => {
                    self.month_delta(-1);
                    InteractionResult::handled()
                }
                Section::Year => {
                    self.year_delta(-1);
                    InteractionResult::handled()
                }
                Section::Grid => {
                    self.grid_move(0, -1);
                    InteractionResult::handled()
                }
                _ => InteractionResult::ignored(),
            },
            KeyCode::Right => match self.section {
                Section::Month => {
                    self.month_delta(1);
                    InteractionResult::handled()
                }
                Section::Year => {
                    self.year_delta(1);
                    InteractionResult::handled()
                }
                Section::Grid => {
                    self.grid_move(0, 1);
                    InteractionResult::handled()
                }
                _ => InteractionResult::ignored(),
            },
            KeyCode::Up => match self.section {
                Section::Grid => {
                    self.grid_move(-1, 0);
                    InteractionResult::handled()
                }
                _ => InteractionResult::ignored(),
            },
            KeyCode::Down => match self.section {
                Section::Grid => {
                    self.grid_move(1, 0);
                    InteractionResult::handled()
                }
                _ => InteractionResult::ignored(),
            },
            KeyCode::Enter => match self.section {
                Section::Grid => {
                    self.select_day();
                    InteractionResult::handled()
                }
                _ => {
                    let val = Value::Text(self.formatted_value());
                    InteractionResult::submit_or_produce(self.submit_target.as_ref(), val)
                }
            },
            _ => InteractionResult::ignored(),
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

    fn set_value(&mut self, value: Value) {
        let Some(text) = value.as_text() else { return };
        let parts: Vec<&str> = text.splitn(2, ' ').collect();
        if let Some(date_str) = parts.first() {
            let segs: Vec<&str> = date_str.split('-').collect();
            if segs.len() == 3 {
                if let (Ok(y), Ok(m), Ok(d)) = (
                    segs[0].parse::<i32>(),
                    segs[1].parse::<u8>(),
                    segs[2].parse::<u8>(),
                ) {
                    self.view_year = y;
                    self.view_month = m;
                    self.cursor_day = d;
                    self.selected_date = Some(Date {
                        year: y,
                        month: m,
                        day: d,
                    });
                }
            }
        }
        if let Some(time_str) = parts.get(1) {
            self.time_input.set_value(Value::Text(time_str.to_string()));
        }
    }

    fn validate(&self, _mode: ValidationMode) -> Result<(), String> {
        run_validators(&self.validators, &Value::Text(self.formatted_value()))
    }
}

impl Component for Calendar {
    fn children(&self) -> &[Node] {
        &[]
    }
    fn children_mut(&mut self) -> &mut [Node] {
        &mut []
    }
}
