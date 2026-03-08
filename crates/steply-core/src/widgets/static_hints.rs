use crate::widgets::traits::{HintGroup, StaticHintSpec};

pub const CHOICE_INPUT_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("↑ ↓ / ← →", "change option", HintGroup::Navigation, 10),
    StaticHintSpec::new("A-Z", "jump by first letter", HintGroup::Navigation, 11),
    StaticHintSpec::new("Enter", "confirm", HintGroup::Action, 20),
];

pub const CONFIRM_RELAXED_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Enter", "confirm", HintGroup::Action, 10),
    StaticHintSpec::new("Y / N", "choose yes/no", HintGroup::Action, 11),
];

pub const CONFIRM_STRICT_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Type", "enter confirmation word", HintGroup::Edit, 10),
    StaticHintSpec::new("← →", "move cursor", HintGroup::Navigation, 11),
    StaticHintSpec::new("Backspace", "delete", HintGroup::Edit, 12),
    StaticHintSpec::new("Enter", "confirm", HintGroup::Action, 20),
];

pub const TEXTAREA_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Shift+Enter / Alt+Enter", "new line", HintGroup::Edit, 10),
    StaticHintSpec::new("Enter / Esc", "finish", HintGroup::Action, 20),
    StaticHintSpec::new("← → ↑ ↓", "move cursor", HintGroup::Navigation, 11),
    StaticHintSpec::new("Home / End", "line start/end", HintGroup::Navigation, 12),
];

pub const COMMAND_RUNNER_HINTS: &[StaticHintSpec] = &[StaticHintSpec::new(
    "Enter",
    "run command",
    HintGroup::Action,
    20,
)];

pub const CALENDAR_COMMON_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Enter", "select / submit", HintGroup::Action, 20),
    StaticHintSpec::new(
        "Tab / Shift+Tab",
        "switch section",
        HintGroup::Navigation,
        10,
    ),
];

pub const CALENDAR_TIME_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new(
        "Tab / Shift+Tab",
        "next/prev segment (edge: section)",
        HintGroup::Navigation,
        10,
    ),
    StaticHintSpec::new("Type", "edit time", HintGroup::Edit, 11),
    StaticHintSpec::new("Enter", "select / submit", HintGroup::Action, 20),
];

pub const FILE_BROWSER_DOC_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Tab", "completion", HintGroup::Completion, 10),
    StaticHintSpec::new("Ctrl+Space", "toggle completion", HintGroup::Completion, 11),
    StaticHintSpec::new(
        "Shift+Space / Alt+Space",
        "open browser",
        HintGroup::View,
        20,
    ),
    StaticHintSpec::new("Enter", "select / submit", HintGroup::Action, 30),
    StaticHintSpec::new("Esc", "close browser", HintGroup::View, 21),
    StaticHintSpec::new("← →", "navigate dirs", HintGroup::Navigation, 12),
    StaticHintSpec::new("↑ ↓", "move entries", HintGroup::Navigation, 13),
    StaticHintSpec::new("Space", "expand/collapse", HintGroup::Navigation, 14),
    StaticHintSpec::new("Ctrl+T", "switch tree/list", HintGroup::View, 22),
];

pub const REPEATER_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Enter / Tab", "next field", HintGroup::Navigation, 10),
    StaticHintSpec::new("Shift+Tab", "previous field", HintGroup::Navigation, 11),
    StaticHintSpec::new("final Enter", "submit", HintGroup::Action, 20),
];

pub const SELECT_LIST_DOC_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("↑ ↓", "move", HintGroup::Navigation, 10),
    StaticHintSpec::new("Enter", "confirm", HintGroup::Action, 20),
    StaticHintSpec::new("Space", "toggle selection", HintGroup::Action, 21),
    StaticHintSpec::new("Ctrl+F", "toggle filter", HintGroup::View, 30),
    StaticHintSpec::new("Esc", "close filter", HintGroup::View, 31),
];

pub const SNIPPET_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Tab / Shift+Tab", "switch slot", HintGroup::Navigation, 10),
    StaticHintSpec::new("Enter", "next slot / submit", HintGroup::Action, 20),
];

pub const TREE_VIEW_DOC_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("↑ ↓", "move", HintGroup::Navigation, 10),
    StaticHintSpec::new("→", "expand", HintGroup::Navigation, 11),
    StaticHintSpec::new("←", "collapse / parent", HintGroup::Navigation, 12),
    StaticHintSpec::new("Enter", "select", HintGroup::Action, 20),
    StaticHintSpec::new("Ctrl+F", "toggle filter", HintGroup::View, 30),
    StaticHintSpec::new("Esc", "leave filter", HintGroup::View, 31),
];

pub const TABLE_DOC_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Ctrl+F", "toggle filter", HintGroup::View, 30),
    StaticHintSpec::new("Enter", "submit step", HintGroup::Action, 40),
    StaticHintSpec::new("↑ ↓", "move rows", HintGroup::Navigation, 10),
    StaticHintSpec::new(
        "Tab / Shift+Tab",
        "switch column",
        HintGroup::Navigation,
        11,
    ),
    StaticHintSpec::new("e", "edit cell", HintGroup::Action, 20),
    StaticHintSpec::new("i / d", "insert/delete row", HintGroup::Action, 21),
    StaticHintSpec::new("m", "move row", HintGroup::Action, 22),
    StaticHintSpec::new("Space", "sort column", HintGroup::Action, 20),
];

pub const OBJECT_EDITOR_DOC_HINTS: &[StaticHintSpec] = &[
    StaticHintSpec::new("Ctrl+F", "toggle filter", HintGroup::View, 30),
    StaticHintSpec::new("↑ ↓", "move", HintGroup::Navigation, 10),
    StaticHintSpec::new("Space / ← →", "expand/collapse", HintGroup::Navigation, 11),
    StaticHintSpec::new("e / r", "edit value/key", HintGroup::Action, 20),
    StaticHintSpec::new("i / d / m", "insert/delete/move", HintGroup::Action, 21),
    StaticHintSpec::new("Enter", "confirm", HintGroup::Action, 20),
    StaticHintSpec::new("Esc", "cancel", HintGroup::Action, 21),
];
