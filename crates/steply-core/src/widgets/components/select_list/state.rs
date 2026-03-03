use super::model::SelectMode;

pub(super) fn marker_symbol(mode: SelectMode, is_selected: bool) -> &'static str {
    match mode {
        SelectMode::Multi | SelectMode::Single => {
            if is_selected {
                "■"
            } else {
                "□"
            }
        }
        SelectMode::Radio => {
            if is_selected {
                "●"
            } else {
                "○"
            }
        }
        SelectMode::List => "",
    }
}
