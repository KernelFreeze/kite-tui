//! Configurable key bindings and the helpers for parsing, validating, and
//! labelling key sequences.
//!
//! Bindings are stored as one sequence string per [`KeyBindingAction`], indexed
//! by the action's position in [`KeyBindingAction::ALL`]. Driving everything
//! off that single array keeps the per-action logic in one place instead of a
//! method per binding. The conversion to and from [`KeyBindingSettings`] stays
//! explicit because that struct is the on-disk TOML schema.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::settings::KeyBindingSettings;

pub(crate) const KEY_TAB: &str = "tab";
pub(crate) const KEY_SHIFT_TAB: &str = "shift+tab";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBindingAction {
    Help,
    Settings,
    CategoryFilter,
    NextCategory,
    PreviousCategory,
    Refresh,
    RefreshAll,
    Quit,
    ResetDefaults,
    JumpTop,
    JumpBottom,
}

impl KeyBindingAction {
    pub const ALL: [Self; 11] = [
        Self::Help,
        Self::Settings,
        Self::CategoryFilter,
        Self::NextCategory,
        Self::PreviousCategory,
        Self::Refresh,
        Self::RefreshAll,
        Self::Quit,
        Self::ResetDefaults,
        Self::JumpTop,
        Self::JumpBottom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Help => "Help",
            Self::Settings => "Settings",
            Self::CategoryFilter => "Category filter",
            Self::NextCategory => "Next category",
            Self::PreviousCategory => "Previous category",
            Self::Refresh => "Refresh",
            Self::RefreshAll => "Refresh all",
            Self::Quit => "Quit",
            Self::ResetDefaults => "Restore defaults",
            Self::JumpTop => "Jump to top",
            Self::JumpBottom => "Jump to bottom",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Help => "Open help",
            Self::Settings => "Open settings",
            Self::CategoryFilter => "Filter categories",
            Self::NextCategory => "Load next category",
            Self::PreviousCategory => "Load previous category",
            Self::Refresh => "Refresh selected category",
            Self::RefreshAll => "Refresh categories and selected category",
            Self::Quit => "Quit or close popup",
            Self::ResetDefaults => "Restore defaults in settings",
            Self::JumpTop => "First article or article top",
            Self::JumpBottom => "Last article or article bottom",
        }
    }

    pub(crate) fn supports_sequences(self) -> bool {
        matches!(self, Self::JumpTop | Self::JumpBottom)
    }

    /// Position of this action within [`Self::ALL`]; also its slot in
    /// [`KeyBindings::sequences`].
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Help => 0,
            Self::Settings => 1,
            Self::CategoryFilter => 2,
            Self::NextCategory => 3,
            Self::PreviousCategory => 4,
            Self::Refresh => 5,
            Self::RefreshAll => 6,
            Self::Quit => 7,
            Self::ResetDefaults => 8,
            Self::JumpTop => 9,
            Self::JumpBottom => 10,
        }
    }

    pub(crate) fn default_sequence(self) -> &'static str {
        match self {
            Self::Help => "?",
            Self::Settings => ",",
            Self::CategoryFilter => "/",
            Self::NextCategory => KEY_TAB,
            Self::PreviousCategory => KEY_SHIFT_TAB,
            Self::Refresh => "r",
            Self::RefreshAll => "R",
            Self::Quit => "q",
            Self::ResetDefaults => "d",
            Self::JumpTop => "gg",
            Self::JumpBottom => "G",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindings {
    sequences: [String; KeyBindingAction::ALL.len()],
}

impl Default for KeyBindings {
    fn default() -> Self {
        let sequences =
            std::array::from_fn(|index| KeyBindingAction::ALL[index].default_sequence().to_owned());
        Self { sequences }
    }
}

impl KeyBindings {
    pub(crate) fn from_settings(settings: &KeyBindingSettings) -> Self {
        let sequences = std::array::from_fn(|index| {
            let action = KeyBindingAction::ALL[index];
            configured_key_sequence(
                settings_value(settings, action),
                action.default_sequence(),
                action.supports_sequences(),
            )
        });
        Self { sequences }
    }

    pub(crate) fn as_settings(&self) -> KeyBindingSettings {
        use KeyBindingAction::*;
        KeyBindingSettings {
            help: self.get(Help).to_owned(),
            settings: self.get(Settings).to_owned(),
            category_filter: self.get(CategoryFilter).to_owned(),
            next_category: self.get(NextCategory).to_owned(),
            previous_category: self.get(PreviousCategory).to_owned(),
            refresh: self.get(Refresh).to_owned(),
            refresh_all: self.get(RefreshAll).to_owned(),
            quit: self.get(Quit).to_owned(),
            reset_defaults: self.get(ResetDefaults).to_owned(),
            jump_top: self.get(JumpTop).to_owned(),
            jump_bottom: self.get(JumpBottom).to_owned(),
        }
    }

    pub(crate) fn get(&self, action: KeyBindingAction) -> &str {
        &self.sequences[action.index()]
    }

    pub(crate) fn set(&mut self, action: KeyBindingAction, sequence: String) {
        self.sequences[action.index()] = sequence;
    }

    pub fn action_label(&self, action: KeyBindingAction) -> String {
        key_sequence_label(self.get(action))
    }

    pub(crate) fn matches(&self, action: KeyBindingAction, key: KeyEvent) -> bool {
        key_matches_sequence(key, self.get(action))
    }

    pub(crate) fn conflicting_action(
        &self,
        action: KeyBindingAction,
        sequence: &str,
    ) -> Option<KeyBindingAction> {
        KeyBindingAction::ALL.into_iter().find(|candidate| {
            *candidate != action && key_sequences_conflict(self.get(*candidate), sequence)
        })
    }

    pub(crate) fn has_article_sequence_prefix(&self, sequence: &str) -> bool {
        self.get(KeyBindingAction::JumpTop).starts_with(sequence)
            || self.get(KeyBindingAction::JumpBottom).starts_with(sequence)
    }

    pub(crate) fn matches_article_jump_top(&self, sequence: &str) -> bool {
        self.get(KeyBindingAction::JumpTop) == sequence
    }

    pub(crate) fn matches_article_jump_bottom(&self, sequence: &str) -> bool {
        self.get(KeyBindingAction::JumpBottom) == sequence
    }
}

/// Reads the configured sequence for `action` from the on-disk settings schema.
///
/// This is the one place that maps the named [`KeyBindingSettings`] fields onto
/// actions; the schema is fixed by serde, so the mapping stays explicit.
fn settings_value(settings: &KeyBindingSettings, action: KeyBindingAction) -> &str {
    match action {
        KeyBindingAction::Help => &settings.help,
        KeyBindingAction::Settings => &settings.settings,
        KeyBindingAction::CategoryFilter => &settings.category_filter,
        KeyBindingAction::NextCategory => &settings.next_category,
        KeyBindingAction::PreviousCategory => &settings.previous_category,
        KeyBindingAction::Refresh => &settings.refresh,
        KeyBindingAction::RefreshAll => &settings.refresh_all,
        KeyBindingAction::Quit => &settings.quit,
        KeyBindingAction::ResetDefaults => &settings.reset_defaults,
        KeyBindingAction::JumpTop => &settings.jump_top,
        KeyBindingAction::JumpBottom => &settings.jump_bottom,
    }
}

fn configured_key_sequence(value: &str, default: &str, allow_multi: bool) -> String {
    let sequence = normalize_key_sequence(value);
    if allow_multi {
        if valid_key_sequence(&sequence) {
            sequence
        } else {
            default.to_owned()
        }
    } else if valid_single_key_binding(&sequence) {
        sequence
    } else {
        default.to_owned()
    }
}

fn key_matches_sequence(key: KeyEvent, configured: &str) -> bool {
    if configured == KEY_TAB {
        return key.code == KeyCode::Tab
            && !key.modifiers.contains(KeyModifiers::SHIFT)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT);
    }

    if configured == KEY_SHIFT_TAB {
        return !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && (key.code == KeyCode::BackTab
                || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT)));
    }

    let mut chars = configured.chars();
    match (chars.next(), chars.next()) {
        (Some(configured), None) => key_sequence_part(key) == Some(configured),
        _ => false,
    }
}

pub(crate) fn key_sequence_part(key: KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => Some(ch),
        _ => None,
    }
}

pub(crate) fn normalize_key_sequence(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        KEY_TAB => KEY_TAB.to_owned(),
        KEY_SHIFT_TAB | "shift-tab" | "backtab" => KEY_SHIFT_TAB.to_owned(),
        "space" => " ".to_owned(),
        _ => value.to_owned(),
    }
}

pub(crate) fn valid_single_key_binding(value: &str) -> bool {
    if is_named_key_binding(value) {
        return true;
    }

    let mut chars = value.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => valid_key_sequence_char(ch),
        _ => false,
    }
}

pub(crate) fn valid_key_sequence(value: &str) -> bool {
    !is_named_key_binding(value) && !value.is_empty() && value.chars().all(valid_key_sequence_char)
}

pub(crate) fn valid_key_sequence_char(key: char) -> bool {
    !key.is_control()
}

pub(crate) fn is_named_key_binding(value: &str) -> bool {
    matches!(value, KEY_TAB | KEY_SHIFT_TAB)
}

fn key_sequences_conflict(existing: &str, proposed: &str) -> bool {
    if is_named_key_binding(existing) || is_named_key_binding(proposed) {
        return existing == proposed;
    }

    existing == proposed || existing.starts_with(proposed) || proposed.starts_with(existing)
}

pub(crate) fn key_sequence_label(sequence: &str) -> String {
    if sequence == KEY_TAB {
        return "Tab".to_owned();
    }

    if sequence == KEY_SHIFT_TAB {
        return "Shift+Tab".to_owned();
    }

    if sequence == " " {
        return "Space".to_owned();
    }

    if sequence.chars().all(|ch| !ch.is_whitespace()) {
        return sequence.to_owned();
    }

    sequence
        .chars()
        .map(key_label)
        .collect::<Vec<_>>()
        .join(" ")
}

fn key_label(key: char) -> String {
    match key {
        ' ' => "Space".to_owned(),
        _ => key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::KeyBindingSettings;

    #[test]
    fn default_settings_keybind_is_comma() {
        assert_eq!(KeyBindings::default().get(KeyBindingAction::Settings), ",");
    }

    #[test]
    fn default_refresh_all_keybind_is_uppercase_r() {
        let keybinds = KeyBindings::default();

        assert_eq!(keybinds.get(KeyBindingAction::RefreshAll), "R");
        assert!(keybinds.matches(
            KeyBindingAction::RefreshAll,
            KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT)
        ));
    }

    #[test]
    fn default_category_keybinds_use_tab_keys() {
        let keybinds = KeyBindings::default();

        assert_eq!(keybinds.get(KeyBindingAction::NextCategory), KEY_TAB);
        assert_eq!(
            keybinds.get(KeyBindingAction::PreviousCategory),
            KEY_SHIFT_TAB
        );
        assert!(keybinds.matches(
            KeyBindingAction::NextCategory,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)
        ));
        assert!(keybinds.matches(
            KeyBindingAction::PreviousCategory,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
        ));
    }

    #[test]
    fn keybinds_use_configured_single_character_values() {
        let settings = KeyBindingSettings {
            help: "h".to_owned(),
            settings: ";".to_owned(),
            category_filter: "f".to_owned(),
            next_category: "n".to_owned(),
            previous_category: "p".to_owned(),
            refresh: "u".to_owned(),
            refresh_all: "U".to_owned(),
            quit: "x".to_owned(),
            reset_defaults: "D".to_owned(),
            jump_top: "tt".to_owned(),
            jump_bottom: "B".to_owned(),
        };

        let keybinds = KeyBindings::from_settings(&settings);

        use KeyBindingAction::*;
        assert_eq!(keybinds.get(Help), "h");
        assert_eq!(keybinds.get(Settings), ";");
        assert_eq!(keybinds.get(CategoryFilter), "f");
        assert_eq!(keybinds.get(NextCategory), "n");
        assert_eq!(keybinds.get(PreviousCategory), "p");
        assert_eq!(keybinds.get(Refresh), "u");
        assert_eq!(keybinds.get(RefreshAll), "U");
        assert_eq!(keybinds.get(Quit), "x");
        assert_eq!(keybinds.get(ResetDefaults), "D");
        assert_eq!(keybinds.get(JumpTop), "tt");
        assert_eq!(keybinds.get(JumpBottom), "B");
    }

    #[test]
    fn invalid_keybind_values_fall_back_to_defaults() {
        let settings = KeyBindingSettings {
            help: String::new(),
            settings: "two".to_owned(),
            category_filter: "\n".to_owned(),
            next_category: "two".to_owned(),
            previous_category: String::new(),
            refresh: "u".to_owned(),
            refresh_all: "two".to_owned(),
            quit: "x".to_owned(),
            reset_defaults: "D".to_owned(),
            jump_top: String::new(),
            jump_bottom: "\n".to_owned(),
        };

        let keybinds = KeyBindings::from_settings(&settings);

        use KeyBindingAction::*;
        assert_eq!(keybinds.get(Help), "?");
        assert_eq!(keybinds.get(Settings), ",");
        assert_eq!(keybinds.get(CategoryFilter), "/");
        assert_eq!(keybinds.get(NextCategory), KEY_TAB);
        assert_eq!(keybinds.get(PreviousCategory), KEY_SHIFT_TAB);
        assert_eq!(keybinds.get(Refresh), "u");
        assert_eq!(keybinds.get(RefreshAll), "R");
        assert_eq!(keybinds.get(Quit), "x");
        assert_eq!(keybinds.get(ResetDefaults), "D");
        assert_eq!(keybinds.get(JumpTop), "gg");
        assert_eq!(keybinds.get(JumpBottom), "G");
    }

    #[test]
    fn keybind_conflict_detection_ignores_same_action() {
        let keybinds = KeyBindings::default();

        assert_eq!(
            keybinds.conflicting_action(KeyBindingAction::Help, "q"),
            Some(KeyBindingAction::Quit)
        );
        assert_eq!(
            keybinds.conflicting_action(KeyBindingAction::Help, "?"),
            None
        );
        assert_eq!(
            keybinds.conflicting_action(KeyBindingAction::Settings, "g"),
            Some(KeyBindingAction::JumpTop)
        );
        assert_eq!(
            keybinds.conflicting_action(KeyBindingAction::PreviousCategory, KEY_TAB),
            Some(KeyBindingAction::NextCategory)
        );
        assert_eq!(
            keybinds.conflicting_action(KeyBindingAction::Help, "t"),
            None
        );
    }
}
