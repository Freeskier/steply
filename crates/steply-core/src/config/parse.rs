use crate::core::value_path::ValueTarget;
use crate::ui::spinner::SpinnerStyle;
use crate::widgets::components::calendar::{Calendar, CalendarMode};
use crate::widgets::components::file_browser::{BrowserMode, DisplayMode, FileBrowserInput};
use crate::widgets::components::object_editor::ObjectEditor;
use crate::widgets::components::repeater::Repeater;
use crate::widgets::components::select_list::{SelectList, SelectMode};
use crate::widgets::components::snippet::Snippet;
use crate::widgets::components::tree_view::TreeView;
use crate::widgets::inputs::choice::ChoiceInput;
use crate::widgets::inputs::color::ColorInput;
use crate::widgets::inputs::masked::MaskedInput;
use crate::widgets::inputs::select::SelectInput;
use crate::widgets::inputs::slider::SliderInput;
use crate::widgets::inputs::text::{TextInput, TextMode};
use crate::widgets::outputs::chart::ChartRenderMode;
use crate::widgets::outputs::progress::{Easing, ProgressStyle, ProgressTransition};
use crate::widgets::outputs::table::TableOutputStyle;
use crate::widgets::outputs::thinking::ThinkingMode;
use crate::widgets::validators;

use super::model::{ConfirmModeDef, ProgressTransitionDef, ValidatorDef};

pub(super) fn parse_text_mode(raw: Option<&str>) -> Result<TextMode, String> {
    match raw.unwrap_or("plain") {
        "plain" => Ok(TextMode::Plain),
        "password" => Ok(TextMode::Password),
        "secret" => Ok(TextMode::Secret),
        other => Err(format!(
            "unsupported text mode: {other} (expected plain|password|secret)"
        )),
    }
}

pub(super) fn parse_select_mode(raw: Option<&str>) -> Result<SelectMode, String> {
    match raw.unwrap_or("single") {
        "single" => Ok(SelectMode::Single),
        "multi" => Ok(SelectMode::Multi),
        "radio" => Ok(SelectMode::Radio),
        "list" => Ok(SelectMode::List),
        other => Err(format!(
            "unsupported select_list mode: {other} (expected single|multi|radio|list)"
        )),
    }
}

pub(super) fn parse_run_mode(
    raw: Option<&str>,
) -> Result<crate::widgets::components::command_runner::RunMode, String> {
    match raw.unwrap_or("manual") {
        "manual" => Ok(crate::widgets::components::command_runner::RunMode::Manual),
        "auto" => Ok(crate::widgets::components::command_runner::RunMode::Auto),
        other => Err(format!(
            "unsupported command_runner run_mode: {other} (expected manual|auto)"
        )),
    }
}

pub(super) fn parse_on_error(
    raw: Option<&str>,
) -> Result<crate::widgets::components::command_runner::OnError, String> {
    match raw.unwrap_or("stay") {
        "stay" => Ok(crate::widgets::components::command_runner::OnError::Stay),
        "continue" => Ok(crate::widgets::components::command_runner::OnError::Continue),
        other => Err(format!(
            "unsupported command_runner on_error: {other} (expected stay|continue)"
        )),
    }
}

pub(super) fn parse_browser_mode(raw: Option<&str>) -> Result<BrowserMode, String> {
    match raw.unwrap_or("list") {
        "list" => Ok(BrowserMode::List),
        "tree" => Ok(BrowserMode::Tree),
        other => Err(format!(
            "unsupported file_browser browser_mode: {other} (expected list|tree)"
        )),
    }
}

pub(super) fn parse_display_mode(raw: &str) -> Result<DisplayMode, String> {
    match raw {
        "full" => Ok(DisplayMode::Full),
        "relative" => Ok(DisplayMode::Relative),
        "name" => Ok(DisplayMode::Name),
        other => Err(format!(
            "unsupported file_browser display_mode: {other} (expected full|relative|name)"
        )),
    }
}

pub(super) fn parse_calendar_mode(raw: Option<&str>) -> Result<CalendarMode, String> {
    match raw.unwrap_or("date") {
        "date" => Ok(CalendarMode::Date),
        "time" => Ok(CalendarMode::Time),
        "date_time" => Ok(CalendarMode::DateTime),
        other => Err(format!(
            "unsupported calendar mode: {other} (expected date|time|date_time)"
        )),
    }
}

pub(super) fn parse_confirm_mode(
    def: Option<ConfirmModeDef>,
) -> crate::widgets::inputs::confirm::ConfirmMode {
    match def.unwrap_or(ConfirmModeDef::Relaxed) {
        ConfirmModeDef::Relaxed => crate::widgets::inputs::confirm::ConfirmMode::Relaxed,
        ConfirmModeDef::Strict { word } => {
            crate::widgets::inputs::confirm::ConfirmMode::Strict { word }
        }
    }
}

pub(super) fn parse_table_style(
    raw: Option<&str>,
) -> Result<crate::widgets::components::table::TableStyle, String> {
    match raw.unwrap_or("grid") {
        "grid" => Ok(crate::widgets::components::table::TableStyle::Grid),
        "clean" => Ok(crate::widgets::components::table::TableStyle::Clean),
        other => Err(format!(
            "unsupported table style: {other} (expected grid|clean)"
        )),
    }
}

pub(super) fn parse_repeater_layout(
    raw: Option<&str>,
) -> Result<crate::widgets::components::repeater::RepeaterLayout, String> {
    match raw.unwrap_or("single_field") {
        "single_field" => Ok(crate::widgets::components::repeater::RepeaterLayout::SingleField),
        "stacked" => Ok(crate::widgets::components::repeater::RepeaterLayout::Stacked),
        other => Err(format!(
            "unsupported repeater layout: {other} (expected single_field|stacked)"
        )),
    }
}

pub(super) fn parse_chart_mode(raw: Option<&str>) -> Result<ChartRenderMode, String> {
    match raw.unwrap_or("braille") {
        "braille" => Ok(ChartRenderMode::Braille),
        "dots" => Ok(ChartRenderMode::Dots),
        "sparkline" => Ok(ChartRenderMode::Sparkline),
        other => Err(format!(
            "unsupported chart mode: {other} (expected braille|dots|sparkline)"
        )),
    }
}

pub(super) fn parse_progress_style(raw: Option<&str>) -> Result<ProgressStyle, String> {
    match raw.unwrap_or("classic_line") {
        "classic_line" => Ok(ProgressStyle::ClassicLine),
        "block_classic" => Ok(ProgressStyle::BlockClassic),
        other => Err(format!(
            "unsupported progress style: {other} (expected classic_line|block_classic)"
        )),
    }
}

pub(super) fn parse_progress_transition(
    def: ProgressTransitionDef,
) -> Result<ProgressTransition, String> {
    match def {
        ProgressTransitionDef::Immediate => Ok(ProgressTransition::Immediate),
        ProgressTransitionDef::Tween {
            duration_ms,
            easing,
        } => Ok(ProgressTransition::Tween {
            duration_ms,
            easing: parse_easing(easing.as_deref())?,
        }),
    }
}

pub(super) fn parse_easing(raw: Option<&str>) -> Result<Easing, String> {
    match raw.unwrap_or("out_cubic") {
        "linear" => Ok(Easing::Linear),
        "out_quad" => Ok(Easing::OutQuad),
        "out_cubic" => Ok(Easing::OutCubic),
        other => Err(format!(
            "unsupported easing: {other} (expected linear|out_quad|out_cubic)"
        )),
    }
}

pub(super) fn parse_table_output_style(raw: Option<&str>) -> Result<TableOutputStyle, String> {
    match raw.unwrap_or("grid") {
        "grid" => Ok(TableOutputStyle::Grid),
        "clean" => Ok(TableOutputStyle::Clean),
        other => Err(format!(
            "unsupported table_output style: {other} (expected grid|clean)"
        )),
    }
}

pub(super) fn parse_thinking_mode(raw: Option<&str>) -> Result<ThinkingMode, String> {
    match raw.unwrap_or("beam") {
        "beam" => Ok(ThinkingMode::Beam),
        "wave" => Ok(ThinkingMode::Wave),
        other => Err(format!(
            "unsupported thinking mode: {other} (expected beam|wave)"
        )),
    }
}

pub(super) fn parse_spinner_style(raw: &str) -> Result<SpinnerStyle, String> {
    match raw {
        "line" => Ok(SpinnerStyle::Line),
        "dots" => Ok(SpinnerStyle::Dots),
        "arc" => Ok(SpinnerStyle::Arc),
        "braille" => Ok(SpinnerStyle::Braille),
        other => Err(format!(
            "unsupported spinner style: {other} (expected line|dots|arc|braille)"
        )),
    }
}

pub(super) fn parse_value_target(raw: &str) -> Result<ValueTarget, String> {
    ValueTarget::parse_selector(raw)
        .map_err(|err| format!("invalid target selector '{raw}': {err}"))
}

pub(super) fn compile_validators(defs: Vec<ValidatorDef>) -> Vec<validators::Validator> {
    defs.into_iter()
        .map(|def| match def {
            ValidatorDef::Required { message } => message
                .map(validators::required_msg)
                .unwrap_or_else(validators::required),
            ValidatorDef::MinLength { value } => validators::min_length(value),
            ValidatorDef::MaxLength { value } => validators::max_length(value),
            ValidatorDef::MinSelections { value } => validators::min_selections(value),
            ValidatorDef::MaxSelections { value } => validators::max_selections(value),
            ValidatorDef::MustBeChecked => validators::must_be_checked(),
            ValidatorDef::MinValue { value } => validators::min_value(value),
            ValidatorDef::MaxValue { value } => validators::max_value(value),
        })
        .collect()
}

pub(super) trait WithSubmitTargetPathValue: Sized {
    fn with_submit_target_path_value(self, target: ValueTarget) -> Self;
}

pub(super) trait WithChangeTargetPathValue: Sized {
    fn with_change_target_path_value(self, target: ValueTarget) -> Self;
}

trait SupportsSubmitTarget: Sized {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self;
    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self;
}

trait SupportsChangeTarget: Sized {
    fn with_change_target_node(self, node: crate::core::NodeId) -> Self;
    fn with_change_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self;
}

impl<T: SupportsSubmitTarget> WithSubmitTargetPathValue for T {
    fn with_submit_target_path_value(self, target: ValueTarget) -> Self {
        match target {
            ValueTarget::Node(node) => self.with_submit_target_node(node),
            ValueTarget::Path { root, path } => self.with_submit_target_parts(root, path),
        }
    }
}

impl<T: SupportsChangeTarget> WithChangeTargetPathValue for T {
    fn with_change_target_path_value(self, target: ValueTarget) -> Self {
        match target {
            ValueTarget::Node(node) => self.with_change_target_node(node),
            ValueTarget::Path { root, path } => self.with_change_target_parts(root, path),
        }
    }
}

impl SupportsSubmitTarget for TextInput {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsChangeTarget for TextInput {
    fn with_change_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_change_target(node)
    }

    fn with_change_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_change_target_path(root, path)
    }
}

impl SupportsSubmitTarget for SelectInput {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for ChoiceInput {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for SelectList {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for MaskedInput {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsChangeTarget for SliderInput {
    fn with_change_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_change_target(node)
    }

    fn with_change_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_change_target_path(root, path)
    }
}

impl SupportsSubmitTarget for ColorInput {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for Calendar {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for FileBrowserInput {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl<T: crate::widgets::components::tree_view::TreeItemLabel> SupportsSubmitTarget for TreeView<T> {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for ObjectEditor {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for Snippet {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

impl SupportsSubmitTarget for Repeater {
    fn with_submit_target_node(self, node: crate::core::NodeId) -> Self {
        self.with_submit_target(node)
    }

    fn with_submit_target_parts(
        self,
        root: crate::core::NodeId,
        path: crate::core::value_path::ValuePath,
    ) -> Self {
        self.with_submit_target_path(root, path)
    }
}

pub(super) fn parse_task_parse(raw: &str) -> Result<crate::task::TaskParse, String> {
    match raw {
        "raw_text" => Ok(crate::task::TaskParse::RawText),
        "number" => Ok(crate::task::TaskParse::Number),
        "json" => Ok(crate::task::TaskParse::Json),
        "lines" => Ok(crate::task::TaskParse::Lines),
        other => Err(format!(
            "unsupported task parse mode: {other} (expected raw_text|number|json|lines)"
        )),
    }
}

pub(super) fn parse_task_kind(raw: &str) -> Result<(), String> {
    if raw == "exec" {
        Ok(())
    } else {
        Err(format!(
            "unsupported task kind: {} (only exec is supported in v1)",
            raw
        ))
    }
}
