use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{
    args::Args,
    error::{KiteError, Result},
    kagi::KagiClient,
    models::{Article, Category, SummaryBlock},
    settings::{self, CategorySettings, KeyBindingSettings, Settings},
    ui,
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const DEFAULT_ENABLED_CATEGORY_KEYS: &[&str] = &[
    "world",
    "gaming",
    "science",
    "ai",
    "technology",
    "business",
    "sports",
    "todayinhistory",
    "usa",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Categories,
    Articles,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Categories => Self::Articles,
            Self::Articles => Self::Categories,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Categories,
    Keybinds,
}

impl SettingsSection {
    fn next(self) -> Self {
        match self {
            Self::Categories => Self::Keybinds,
            Self::Keybinds => Self::Categories,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Categories => Self::Keybinds,
            Self::Keybinds => Self::Categories,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Categories => "Categories",
            Self::Keybinds => "Keybinds",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBindingAction {
    Help,
    Settings,
    CategoryFilter,
    Refresh,
    Quit,
    ResetDefaults,
    JumpTop,
    JumpBottom,
}

impl KeyBindingAction {
    pub const ALL: [Self; 8] = [
        Self::Help,
        Self::Settings,
        Self::CategoryFilter,
        Self::Refresh,
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
            Self::Refresh => "Refresh",
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
            Self::Refresh => "Refresh selected category",
            Self::Quit => "Quit or close popup",
            Self::ResetDefaults => "Restore defaults in settings",
            Self::JumpTop => "First article or article top",
            Self::JumpBottom => "Last article or article bottom",
        }
    }

    fn supports_sequences(self) -> bool {
        matches!(self, Self::JumpTop | Self::JumpBottom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindings {
    pub help: String,
    pub settings: String,
    pub category_filter: String,
    pub refresh: String,
    pub quit: String,
    pub reset_defaults: String,
    pub jump_top: String,
    pub jump_bottom: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            help: "?".to_owned(),
            settings: ",".to_owned(),
            category_filter: "/".to_owned(),
            refresh: "r".to_owned(),
            quit: "q".to_owned(),
            reset_defaults: "d".to_owned(),
            jump_top: "gg".to_owned(),
            jump_bottom: "G".to_owned(),
        }
    }
}

impl KeyBindings {
    fn from_settings(settings: &KeyBindingSettings) -> Self {
        let defaults = Self::default();

        Self {
            help: configured_key_sequence(&settings.help, &defaults.help, false),
            settings: configured_key_sequence(&settings.settings, &defaults.settings, false),
            category_filter: configured_key_sequence(
                &settings.category_filter,
                &defaults.category_filter,
                false,
            ),
            refresh: configured_key_sequence(&settings.refresh, &defaults.refresh, false),
            quit: configured_key_sequence(&settings.quit, &defaults.quit, false),
            reset_defaults: configured_key_sequence(
                &settings.reset_defaults,
                &defaults.reset_defaults,
                false,
            ),
            jump_top: configured_key_sequence(&settings.jump_top, &defaults.jump_top, true),
            jump_bottom: configured_key_sequence(
                &settings.jump_bottom,
                &defaults.jump_bottom,
                true,
            ),
        }
    }

    fn as_settings(&self) -> KeyBindingSettings {
        KeyBindingSettings {
            help: self.help.clone(),
            settings: self.settings.clone(),
            category_filter: self.category_filter.clone(),
            refresh: self.refresh.clone(),
            quit: self.quit.clone(),
            reset_defaults: self.reset_defaults.clone(),
            jump_top: self.jump_top.clone(),
            jump_bottom: self.jump_bottom.clone(),
        }
    }

    pub fn help_label(&self) -> String {
        key_sequence_label(&self.help)
    }

    pub fn settings_label(&self) -> String {
        key_sequence_label(&self.settings)
    }

    pub fn category_filter_label(&self) -> String {
        key_sequence_label(&self.category_filter)
    }

    pub fn refresh_label(&self) -> String {
        key_sequence_label(&self.refresh)
    }

    pub fn quit_label(&self) -> String {
        key_sequence_label(&self.quit)
    }

    pub fn reset_defaults_label(&self) -> String {
        key_sequence_label(&self.reset_defaults)
    }

    pub fn action_label(&self, action: KeyBindingAction) -> String {
        key_sequence_label(self.action_sequence(action))
    }

    fn action_sequence(&self, action: KeyBindingAction) -> &str {
        match action {
            KeyBindingAction::Help => &self.help,
            KeyBindingAction::Settings => &self.settings,
            KeyBindingAction::CategoryFilter => &self.category_filter,
            KeyBindingAction::Refresh => &self.refresh,
            KeyBindingAction::Quit => &self.quit,
            KeyBindingAction::ResetDefaults => &self.reset_defaults,
            KeyBindingAction::JumpTop => &self.jump_top,
            KeyBindingAction::JumpBottom => &self.jump_bottom,
        }
    }

    fn set_action_sequence(&mut self, action: KeyBindingAction, sequence: String) {
        match action {
            KeyBindingAction::Help => self.help = sequence,
            KeyBindingAction::Settings => self.settings = sequence,
            KeyBindingAction::CategoryFilter => self.category_filter = sequence,
            KeyBindingAction::Refresh => self.refresh = sequence,
            KeyBindingAction::Quit => self.quit = sequence,
            KeyBindingAction::ResetDefaults => self.reset_defaults = sequence,
            KeyBindingAction::JumpTop => self.jump_top = sequence,
            KeyBindingAction::JumpBottom => self.jump_bottom = sequence,
        }
    }

    fn conflicting_action(
        &self,
        action: KeyBindingAction,
        sequence: &str,
    ) -> Option<KeyBindingAction> {
        KeyBindingAction::ALL.into_iter().find(|candidate| {
            *candidate != action
                && key_sequences_conflict(self.action_sequence(*candidate), sequence)
        })
    }

    fn has_article_sequence_prefix(&self, sequence: &str) -> bool {
        self.jump_top.starts_with(sequence) || self.jump_bottom.starts_with(sequence)
    }

    fn matches_article_jump_top(&self, sequence: &str) -> bool {
        self.jump_top == sequence
    }

    fn matches_article_jump_bottom(&self, sequence: &str) -> bool {
        self.jump_bottom == sequence
    }

    fn matches_help(&self, key: KeyEvent) -> bool {
        key_matches_sequence(key, &self.help)
    }

    fn matches_settings(&self, key: KeyEvent) -> bool {
        key_matches_sequence(key, &self.settings)
    }

    fn matches_category_filter(&self, key: KeyEvent) -> bool {
        key_matches_sequence(key, &self.category_filter)
    }

    fn matches_refresh(&self, key: KeyEvent) -> bool {
        key_matches_sequence(key, &self.refresh)
    }

    fn matches_quit(&self, key: KeyEvent) -> bool {
        key_matches_sequence(key, &self.quit)
    }

    fn matches_reset_defaults(&self, key: KeyEvent) -> bool {
        key_matches_sequence(key, &self.reset_defaults)
    }
}

#[derive(Debug)]
pub struct AppState {
    pub categories: Vec<Category>,
    pub enabled_categories: Vec<bool>,
    pub keybinds: KeyBindings,
    pub selected_category: usize,
    pub loaded_category: Option<usize>,
    pub articles: Vec<Article>,
    pub selected_article: usize,
    pub focus: Focus,
    pub status: String,
    pub error: Option<String>,
    pub category_filter: String,
    pub category_filter_active: bool,
    pub settings_open: bool,
    pub settings_section: SettingsSection,
    pub config_selected_category: usize,
    pub config_filter: String,
    pub config_filter_active: bool,
    pub selected_keybind: usize,
    pub editing_keybind: Option<KeyBindingAction>,
    pub keybind_input: String,
    pub help_open: bool,
    pub detail_open: bool,
    pub detail_scroll: u16,
    pending_key_sequence: String,
    should_quit: bool,
}

impl AppState {
    pub async fn bootstrap(client: &KagiClient, initial_category: Option<&str>) -> Result<Self> {
        let categories = client.categories().await?;
        let (settings, settings_error) = load_settings();
        let mut enabled_categories = enabled_categories_from_settings(&categories, &settings)
            .unwrap_or_else(|| default_enabled_categories(&categories));
        let keybinds = KeyBindings::from_settings(&settings.keybinds);
        let selected_category =
            select_initial_category(&categories, &mut enabled_categories, initial_category)?;

        let mut state = Self {
            categories,
            enabled_categories,
            keybinds,
            selected_category,
            loaded_category: None,
            articles: Vec::new(),
            selected_article: 0,
            focus: Focus::Articles,
            status: "Loading articles".to_owned(),
            error: None,
            category_filter: String::new(),
            category_filter_active: false,
            settings_open: false,
            settings_section: SettingsSection::Categories,
            config_selected_category: selected_category,
            config_filter: String::new(),
            config_filter_active: false,
            selected_keybind: 0,
            editing_keybind: None,
            keybind_input: String::new(),
            help_open: false,
            detail_open: false,
            detail_scroll: 0,
            pending_key_sequence: String::new(),
            should_quit: false,
        };

        state.load_selected_category(client).await;
        if state.error.is_none() {
            state.error = settings_error;
        }
        Ok(state)
    }

    pub async fn load_selected_category(&mut self, client: &KagiClient) {
        let Some(category) = self.categories.get(self.selected_category).cloned() else {
            return;
        };

        self.status = format!("Loading {}", category.name);
        self.error = None;

        match client.articles(&category).await {
            Ok(articles) => {
                self.articles = articles;
                self.loaded_category = Some(self.selected_category);
                self.selected_article = 0;
                self.detail_open = false;
                self.detail_scroll = 0;
                self.status = format!("{} articles in {}", self.articles.len(), category.name);
            }
            Err(error) => {
                self.articles.clear();
                self.loaded_category = None;
                self.selected_article = 0;
                self.detail_open = false;
                self.detail_scroll = 0;
                self.status = format!("Could not load {}", category.name);
                self.error = Some(error.to_string());
            }
        }
    }

    pub fn selected_category(&self) -> Option<&Category> {
        self.categories.get(self.selected_category)
    }

    pub fn loaded_category(&self) -> Option<&Category> {
        self.loaded_category
            .and_then(|index| self.categories.get(index))
    }

    pub fn selected_article(&self) -> Option<&Article> {
        self.articles.get(self.selected_article)
    }

    pub fn has_category_filter(&self) -> bool {
        !self.category_filter.trim().is_empty()
    }

    pub fn filtered_category_indices(&self) -> Vec<usize> {
        self.categories
            .iter()
            .enumerate()
            .filter_map(|(index, category)| {
                (self.is_category_enabled(index) && self.category_matches_filter(category))
                    .then_some(index)
            })
            .collect()
    }

    pub fn is_category_enabled(&self, index: usize) -> bool {
        self.enabled_categories.get(index).copied().unwrap_or(false)
    }

    pub fn enabled_category_count(&self) -> usize {
        self.enabled_categories
            .iter()
            .filter(|enabled| **enabled)
            .count()
    }

    pub fn hidden_category_count(&self) -> usize {
        self.categories
            .len()
            .saturating_sub(self.enabled_category_count())
    }

    pub fn enabled_category_indices(&self) -> Vec<usize> {
        self.enabled_categories
            .iter()
            .enumerate()
            .filter_map(|(index, enabled)| enabled.then_some(index))
            .collect()
    }

    pub fn has_config_filter(&self) -> bool {
        !self.config_filter.trim().is_empty()
    }

    pub fn filtered_config_category_indices(&self) -> Vec<usize> {
        self.categories
            .iter()
            .enumerate()
            .filter_map(|(index, category)| {
                category_matches_filter(category, &self.config_filter).then_some(index)
            })
            .collect()
    }

    fn category_matches_filter(&self, category: &Category) -> bool {
        category_matches_filter(category, &self.category_filter)
    }

    fn selected_category_matches_filter(&self) -> bool {
        self.is_category_enabled(self.selected_category)
            && self
                .selected_category()
                .is_some_and(|category| self.category_matches_filter(category))
    }

    fn start_category_filter(&mut self) {
        self.pending_key_sequence.clear();
        self.category_filter_active = true;
        self.focus = Focus::Categories;
        self.error = None;
        self.sync_selected_category_to_filter();
        self.update_category_filter_status();
    }

    fn finish_category_filter(&mut self) {
        self.category_filter_active = false;
        if self.has_category_filter() {
            self.update_category_filter_status();
        } else {
            self.status = "Category filter cleared".to_owned();
        }
    }

    fn clear_category_filter(&mut self) {
        self.category_filter.clear();
        self.category_filter_active = false;
        self.status = "Category filter cleared".to_owned();
        self.error = None;
    }

    fn push_category_filter(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }

        self.category_filter.push(ch);
        self.sync_selected_category_to_filter();
        self.update_category_filter_status();
    }

    fn pop_category_filter(&mut self) {
        self.category_filter.pop();
        self.sync_selected_category_to_filter();
        self.update_category_filter_status();
    }

    fn clear_category_filter_input(&mut self) {
        self.category_filter.clear();
        self.sync_selected_category_to_filter();
        self.update_category_filter_status();
    }

    fn sync_selected_category_to_filter(&mut self) {
        if self.selected_category_matches_filter() {
            return;
        }

        if let Some(index) = self.filtered_category_indices().first().copied() {
            self.selected_category = index;
        }
    }

    fn update_category_filter_status(&mut self) {
        let filter = self.category_filter.trim();
        if filter.is_empty() {
            self.status = "Type to filter categories".to_owned();
            return;
        }

        let matches = self.filtered_category_indices().len();
        self.status = match matches {
            0 => format!("No categories match /{filter}"),
            1 => format!("1 category matches /{filter}"),
            _ => format!("{matches} categories match /{filter}"),
        };
    }

    fn open_settings(&mut self) {
        self.pending_key_sequence.clear();
        self.settings_open = true;
        self.settings_section = SettingsSection::Categories;
        self.config_selected_category = self.selected_category;
        self.config_filter_active = false;
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.category_filter_active = false;
        self.detail_open = false;
        self.detail_scroll = 0;
        self.error = None;
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    fn close_settings(&mut self) {
        self.settings_open = false;
        self.config_filter_active = false;
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = format!(
            "{} categories shown, {} hidden",
            self.enabled_category_count(),
            self.hidden_category_count()
        );
        self.sync_selected_category_to_filter();
    }

    fn next_settings_section(&mut self) {
        self.settings_section = self.settings_section.next();
        self.config_filter_active = false;
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.update_settings_status();
    }

    fn previous_settings_section(&mut self) {
        self.settings_section = self.settings_section.previous();
        self.config_filter_active = false;
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.update_settings_status();
    }

    fn open_help(&mut self) {
        self.pending_key_sequence.clear();
        self.help_open = true;
        self.error = None;
    }

    fn close_help(&mut self) {
        self.help_open = false;
    }

    fn start_config_filter(&mut self) {
        self.config_filter_active = true;
        self.error = None;
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    fn finish_config_filter(&mut self) {
        self.config_filter_active = false;
        self.update_category_config_status();
    }

    fn clear_config_filter(&mut self) {
        self.config_filter.clear();
        self.config_filter_active = false;
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    fn push_config_filter(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }

        self.config_filter.push(ch);
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    fn pop_config_filter(&mut self) {
        self.config_filter.pop();
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    fn clear_config_filter_input(&mut self) {
        self.config_filter.clear();
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    fn sync_config_selected_category_to_filter(&mut self) {
        if self
            .categories
            .get(self.config_selected_category)
            .is_some_and(|category| category_matches_filter(category, &self.config_filter))
        {
            return;
        }

        if let Some(index) = self.filtered_config_category_indices().first().copied() {
            self.config_selected_category = index;
        }
    }

    fn update_category_config_status(&mut self) {
        let shown = self.enabled_category_count();
        let hidden = self.hidden_category_count();
        let filter = self.config_filter.trim();

        self.status = if filter.is_empty() {
            format!("{shown} categories shown, {hidden} hidden")
        } else {
            let matches = self.filtered_config_category_indices().len();
            format!("{shown} shown, {hidden} hidden, {matches} match /{filter}")
        };
    }

    fn update_settings_status(&mut self) {
        match self.settings_section {
            SettingsSection::Categories => self.update_category_config_status(),
            SettingsSection::Keybinds => self.update_keybind_settings_status(),
        }
    }

    fn update_keybind_settings_status(&mut self) {
        if let Some(action) = self.editing_keybind {
            if self.keybind_input.is_empty() {
                self.status = format!("Type a key sequence for {}", action.label());
            } else {
                self.status = format!(
                    "Editing {}: {}",
                    action.label(),
                    key_sequence_label(&self.keybind_input)
                );
            }
        } else {
            self.status = "Select a keybind and press Enter to edit".to_owned();
        }
    }

    fn move_config_category_by(&mut self, step: isize, wrap: bool) {
        let indices = self.filtered_config_category_indices();
        if indices.is_empty() {
            self.update_category_config_status();
            return;
        }

        let current = indices
            .iter()
            .position(|index| *index == self.config_selected_category)
            .unwrap_or(0) as isize;
        let last = indices.len() as isize - 1;
        let next = if wrap {
            (current + step).rem_euclid(indices.len() as isize)
        } else {
            (current + step).clamp(0, last)
        };

        self.config_selected_category = indices[next as usize];
        self.status = self
            .categories
            .get(self.config_selected_category)
            .map(|category| format!("Selected {}", category.name))
            .unwrap_or_default();
    }

    fn toggle_config_category(&mut self) {
        let Some(category_name) = self
            .categories
            .get(self.config_selected_category)
            .map(|category| category.name.clone())
        else {
            return;
        };
        let enabled_count = self.enabled_category_count();

        let Some(enabled) = self
            .enabled_categories
            .get_mut(self.config_selected_category)
        else {
            return;
        };

        if *enabled && enabled_count == 1 {
            self.status = "At least one category must stay shown".to_owned();
            return;
        }

        *enabled = !*enabled;
        self.status = if *enabled {
            format!("Showing {category_name}")
        } else {
            format!("Hiding {category_name}")
        };
        self.sync_selected_category_to_filter();
        self.persist_settings();
    }

    fn reset_default_category_config(&mut self) {
        self.enabled_categories = default_enabled_categories(&self.categories);
        self.sync_selected_category_to_filter();
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
        self.persist_settings();
    }

    fn move_keybind_by(&mut self, step: isize, wrap: bool) {
        let len = KeyBindingAction::ALL.len();
        let current = self.selected_keybind.min(len.saturating_sub(1)) as isize;
        let last = len as isize - 1;
        let next = if wrap {
            (current + step).rem_euclid(len as isize)
        } else {
            (current + step).clamp(0, last)
        };

        self.selected_keybind = next as usize;
        if let Some(action) = self.selected_keybind_action() {
            self.status = format!("Selected {}", action.label());
        }
    }

    fn start_keybind_edit(&mut self) {
        let Some(action) = self.selected_keybind_action() else {
            return;
        };

        self.editing_keybind = Some(action);
        self.keybind_input.clear();
        self.update_keybind_settings_status();
    }

    fn cancel_keybind_edit(&mut self) {
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = "Keybind edit cancelled".to_owned();
    }

    fn push_keybind_input(&mut self, ch: char) {
        if !valid_key_sequence_char(ch) {
            self.status = "Keybinds must use printable keys".to_owned();
            return;
        }

        self.keybind_input.push(ch);
        self.update_keybind_settings_status();
    }

    fn pop_keybind_input(&mut self) {
        self.keybind_input.pop();
        self.update_keybind_settings_status();
    }

    fn clear_keybind_input(&mut self) {
        self.keybind_input.clear();
        self.update_keybind_settings_status();
    }

    fn finish_keybind_edit(&mut self, action: KeyBindingAction) {
        let sequence = self.keybind_input.clone();
        if !valid_key_sequence(&sequence) {
            self.status = "Keybinds must not be empty".to_owned();
            return;
        }
        if !action.supports_sequences() && sequence.chars().count() != 1 {
            self.status = format!("{} requires a single key", action.label());
            return;
        }

        if let Some(conflict) = self.keybinds.conflicting_action(action, &sequence) {
            self.status = format!(
                "{} is already used for {}",
                key_sequence_label(&sequence),
                conflict.label()
            );
            return;
        }

        self.keybinds.set_action_sequence(action, sequence.clone());
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = format!(
            "{} keybind set to {}",
            action.label(),
            key_sequence_label(&sequence)
        );
        self.persist_settings();
    }

    fn reset_default_keybinds(&mut self) {
        self.keybinds = KeyBindings::default();
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = "Keybinds restored to defaults".to_owned();
        self.persist_settings();
    }

    fn selected_keybind_action(&self) -> Option<KeyBindingAction> {
        KeyBindingAction::ALL.get(self.selected_keybind).copied()
    }

    fn persist_settings(&mut self) {
        if let Err(error) = self.current_settings().save() {
            self.error = Some(error.to_string());
        }
    }

    fn current_settings(&self) -> Settings {
        Settings {
            categories: CategorySettings {
                enabled: self
                    .enabled_category_indices()
                    .into_iter()
                    .filter_map(|index| self.categories.get(index).map(settings::category_key))
                    .collect(),
            },
            keybinds: self.keybinds.as_settings(),
        }
    }

    fn move_category_by(&mut self, step: isize, wrap: bool) {
        let indices = self.filtered_category_indices();
        if indices.is_empty() {
            self.update_category_filter_status();
            return;
        }

        let current = indices
            .iter()
            .position(|index| *index == self.selected_category)
            .unwrap_or(0) as isize;
        let last = indices.len() as isize - 1;
        let next = if wrap {
            (current + step).rem_euclid(indices.len() as isize)
        } else {
            (current + step).clamp(0, last)
        };

        self.selected_category = indices[next as usize];
        self.status = self
            .selected_category()
            .map(|category| format!("Selected {}", category.name))
            .unwrap_or_default();
    }

    fn move_next(&mut self) {
        if self.detail_open {
            self.detail_scroll = self.detail_scroll.saturating_add(1);
            return;
        }

        match self.focus {
            Focus::Categories => {
                self.move_category_by(1, true);
            }
            Focus::Articles => {
                if !self.articles.is_empty() {
                    self.selected_article =
                        (self.selected_article + 1).min(self.articles.len() - 1);
                    self.detail_scroll = 0;
                }
            }
        }
    }

    fn move_previous(&mut self) {
        if self.detail_open {
            self.detail_scroll = self.detail_scroll.saturating_sub(1);
            return;
        }

        match self.focus {
            Focus::Categories => {
                self.move_category_by(-1, true);
            }
            Focus::Articles => {
                self.selected_article = self.selected_article.saturating_sub(1);
                self.detail_scroll = 0;
            }
        }
    }

    fn page_next(&mut self) {
        if self.detail_open {
            self.detail_scroll = self.detail_scroll.saturating_add(10);
            return;
        }

        match self.focus {
            Focus::Articles => {
                if !self.articles.is_empty() {
                    self.selected_article =
                        (self.selected_article + 10).min(self.articles.len() - 1);
                    self.detail_scroll = 0;
                }
            }
            Focus::Categories => {
                self.move_category_by(10, false);
            }
        }
    }

    fn page_previous(&mut self) {
        if self.detail_open {
            self.detail_scroll = self.detail_scroll.saturating_sub(10);
            return;
        }

        match self.focus {
            Focus::Articles => {
                self.selected_article = self.selected_article.saturating_sub(10);
                self.detail_scroll = 0;
            }
            Focus::Categories => {
                self.move_category_by(-10, false);
            }
        }
    }

    fn jump_to_top(&mut self) {
        if self.detail_open {
            self.detail_scroll = 0;
            self.status = "Top of article".to_owned();
            return;
        }

        if self.focus == Focus::Articles && !self.articles.is_empty() {
            self.selected_article = 0;
            self.detail_scroll = 0;
            self.status = "Selected first article".to_owned();
        }
    }

    fn jump_to_bottom(&mut self) {
        if self.detail_open {
            self.detail_scroll = self
                .selected_article_line_count()
                .saturating_sub(1)
                .min(usize::from(u16::MAX)) as u16;
            self.status = "Bottom of article".to_owned();
            return;
        }

        if self.focus == Focus::Articles && !self.articles.is_empty() {
            self.selected_article = self.articles.len() - 1;
            self.detail_scroll = 0;
            self.status = "Selected last article".to_owned();
        }
    }

    fn selected_article_line_count(&self) -> usize {
        let Some(article) = self.selected_article() else {
            return 0;
        };

        let mut lines = 3;
        if article.summary_blocks.is_empty() {
            lines += article.summary.lines().count();
        } else {
            lines += summary_block_line_count(&article.summary_blocks);
        }

        if article.link.is_some() {
            lines += 2;
        }

        lines
    }

    fn next_focus(&mut self) {
        if self.detail_open {
            return;
        }

        self.focus = self.focus.next();
    }

    fn open_detail(&mut self) {
        if !self.articles.is_empty() {
            self.pending_key_sequence.clear();
            self.detail_open = true;
            self.detail_scroll = 0;
        }
    }

    fn close_detail(&mut self) {
        self.pending_key_sequence.clear();
        self.detail_open = false;
        self.detail_scroll = 0;
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }
}

pub async fn run(args: Args) -> Result<()> {
    let client = KagiClient::new(args.base_url, Duration::from_secs(args.timeout_seconds))?;
    let mut state = AppState::bootstrap(&client, args.category.as_deref()).await?;

    terminal::enable_raw_mode()?;
    let _restore = TerminalRestore;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = run_event_loop(&mut terminal, &client, &mut state).await;
    let cursor_result = terminal.show_cursor().map_err(KiteError::from);

    run_result.and(cursor_result)
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    client: &KagiClient,
    state: &mut AppState,
) -> Result<()> {
    while !state.should_quit {
        terminal.draw(|frame| ui::draw(frame, state))?;

        if event::poll(EVENT_POLL_INTERVAL)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(state, client, key).await;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

async fn handle_key(state: &mut AppState, client: &KagiClient, key: KeyEvent) {
    if state.help_open {
        handle_help_key(state, key);
        return;
    }

    if state.settings_open {
        if state.editing_keybind.is_none()
            && !state.config_filter_active
            && state.keybinds.matches_help(key)
        {
            state.open_help();
            return;
        }

        handle_settings_key(state, key);
        return;
    }

    if state.category_filter_active {
        handle_category_filter_key(state, key);
        return;
    }

    if state.keybinds.matches_help(key) {
        state.open_help();
        return;
    }

    if state.detail_open {
        if handle_article_sequence_key(state, key) {
            return;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
            KeyCode::Esc | KeyCode::Enter => state.close_detail(),
            KeyCode::Down | KeyCode::Char('j') => state.move_next(),
            KeyCode::Up | KeyCode::Char('k') => state.move_previous(),
            KeyCode::PageDown => state.page_next(),
            KeyCode::PageUp => state.page_previous(),
            _ => {}
        }
        if state.keybinds.matches_quit(key) {
            state.quit();
        }
        return;
    }

    if state.focus == Focus::Articles && handle_article_sequence_key(state, key) {
        return;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Esc if state.has_category_filter() => state.clear_category_filter(),
        KeyCode::Esc => state.quit(),
        KeyCode::Tab => state.next_focus(),
        KeyCode::Down | KeyCode::Char('j') => state.move_next(),
        KeyCode::Up | KeyCode::Char('k') => state.move_previous(),
        KeyCode::PageDown => state.page_next(),
        KeyCode::PageUp => state.page_previous(),
        KeyCode::Enter => match state.focus {
            Focus::Categories if state.selected_category_matches_filter() => {
                state.load_selected_category(client).await
            }
            Focus::Categories => state.update_category_filter_status(),
            Focus::Articles => state.open_detail(),
        },
        _ => {}
    }

    if state.keybinds.matches_settings(key) {
        state.open_settings();
    } else if state.keybinds.matches_category_filter(key) {
        state.start_category_filter();
    } else if state.keybinds.matches_quit(key) {
        state.quit();
    } else if state.keybinds.matches_refresh(key) {
        if state.selected_category_matches_filter() {
            state.load_selected_category(client).await;
        } else {
            state.update_category_filter_status();
        }
    }
}

fn handle_settings_key(state: &mut AppState, key: KeyEvent) {
    if let Some(action) = state.editing_keybind {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
            KeyCode::Esc => state.cancel_keybind_edit(),
            KeyCode::Enter => state.finish_keybind_edit(action),
            KeyCode::Backspace => state.pop_keybind_input(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.clear_keybind_input();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.push_keybind_input(ch);
            }
            _ => {
                state.status = "Type printable keys, Enter to save, or Esc to cancel".to_owned();
            }
        }
        return;
    }

    if state.config_filter_active {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.clear_config_filter_input();
            }
            KeyCode::Esc | KeyCode::Enter => state.finish_config_filter(),
            KeyCode::Backspace => state.pop_config_filter(),
            KeyCode::Down => state.move_config_category_by(1, true),
            KeyCode::Up => state.move_config_category_by(-1, true),
            KeyCode::PageDown => state.move_config_category_by(10, false),
            KeyCode::PageUp => state.move_config_category_by(-10, false),
            KeyCode::Char(' ') => state.toggle_config_category(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.push_config_filter(ch);
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Esc => state.close_settings(),
        KeyCode::Tab | KeyCode::Right => state.next_settings_section(),
        KeyCode::Left => state.previous_settings_section(),
        _ => match state.settings_section {
            SettingsSection::Categories => handle_category_settings_key(state, key),
            SettingsSection::Keybinds => handle_keybind_settings_key(state, key),
        },
    }
}

fn handle_category_settings_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => state.close_settings(),
        KeyCode::Backspace if state.has_config_filter() => state.clear_config_filter(),
        KeyCode::Down | KeyCode::Char('j') => state.move_config_category_by(1, true),
        KeyCode::Up | KeyCode::Char('k') => state.move_config_category_by(-1, true),
        KeyCode::PageDown => state.move_config_category_by(10, false),
        KeyCode::PageUp => state.move_config_category_by(-10, false),
        KeyCode::Char(' ') => state.toggle_config_category(),
        _ => {}
    }

    if state.keybinds.matches_quit(key) {
        state.close_settings();
    } else if state.keybinds.matches_category_filter(key) {
        state.start_config_filter();
    } else if state.keybinds.matches_reset_defaults(key) {
        state.reset_default_category_config();
    }
}

fn handle_keybind_settings_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => state.start_keybind_edit(),
        KeyCode::Down | KeyCode::Char('j') => state.move_keybind_by(1, true),
        KeyCode::Up | KeyCode::Char('k') => state.move_keybind_by(-1, true),
        KeyCode::PageDown => state.move_keybind_by(4, false),
        KeyCode::PageUp => state.move_keybind_by(-4, false),
        _ => {}
    }

    if state.keybinds.matches_quit(key) {
        state.close_settings();
    } else if state.keybinds.matches_reset_defaults(key) {
        state.reset_default_keybinds();
    }
}

fn handle_help_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Esc | KeyCode::Enter => state.close_help(),
        _ => {
            if state.keybinds.matches_help(key) || state.keybinds.matches_quit(key) {
                state.close_help();
            }
        }
    }
}

fn handle_category_filter_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.clear_category_filter_input();
        }
        KeyCode::Esc | KeyCode::Enter => state.finish_category_filter(),
        KeyCode::Backspace => state.pop_category_filter(),
        KeyCode::Down => state.move_next(),
        KeyCode::Up => state.move_previous(),
        KeyCode::PageDown => state.page_next(),
        KeyCode::PageUp => state.page_previous(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.push_category_filter(ch);
        }
        _ => {}
    }
}

fn handle_article_sequence_key(state: &mut AppState, key: KeyEvent) -> bool {
    let Some(ch) = key_sequence_part(key) else {
        state.pending_key_sequence.clear();
        return false;
    };

    let mut sequence = state.pending_key_sequence.clone();
    sequence.push(ch);

    if state.keybinds.matches_article_jump_top(&sequence) {
        state.pending_key_sequence.clear();
        state.jump_to_top();
        return true;
    }

    if state.keybinds.matches_article_jump_bottom(&sequence) {
        state.pending_key_sequence.clear();
        state.jump_to_bottom();
        return true;
    }

    if state.keybinds.has_article_sequence_prefix(&sequence) {
        state.pending_key_sequence = sequence;
        state.status = format!(
            "Awaiting key sequence after {}",
            key_sequence_label(&state.pending_key_sequence)
        );
        return true;
    }

    state.pending_key_sequence.clear();
    false
}

fn find_category(categories: &[Category], requested: &str) -> Option<usize> {
    let requested = requested.trim().to_ascii_lowercase();

    categories.iter().position(|category| {
        category.name.to_ascii_lowercase() == requested
            || category.file.to_ascii_lowercase() == requested
            || category.file_stem().to_ascii_lowercase() == requested
    })
}

fn configured_key_sequence(value: &str, default: &str, allow_multi: bool) -> String {
    let char_count = value.chars().count();
    if valid_key_sequence(value) && (allow_multi || char_count == 1) {
        value.to_owned()
    } else {
        default.to_owned()
    }
}

fn key_matches_sequence(key: KeyEvent, configured: &str) -> bool {
    let mut chars = configured.chars();
    match (chars.next(), chars.next()) {
        (Some(configured), None) => key_sequence_part(key) == Some(configured),
        _ => false,
    }
}

fn key_sequence_part(key: KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => Some(ch),
        _ => None,
    }
}

fn valid_key_sequence(value: &str) -> bool {
    !value.is_empty() && value.chars().all(valid_key_sequence_char)
}

fn valid_key_sequence_char(key: char) -> bool {
    !key.is_control()
}

fn key_sequences_conflict(existing: &str, proposed: &str) -> bool {
    existing == proposed || existing.starts_with(proposed) || proposed.starts_with(existing)
}

fn key_sequence_label(sequence: &str) -> String {
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

fn summary_block_line_count(blocks: &[SummaryBlock]) -> usize {
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let spacer = usize::from(index > 0);
            let lines = match block {
                SummaryBlock::Heading { .. } | SummaryBlock::Paragraph(_) => 1,
                SummaryBlock::List { items, .. } => items.len(),
                SummaryBlock::Quote(text) => text.lines().count(),
            };
            spacer + lines
        })
        .sum()
}

fn load_settings() -> (Settings, Option<String>) {
    match Settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => (Settings::default(), Some(error.to_string())),
    }
}

fn enabled_categories_from_settings(
    categories: &[Category],
    settings: &Settings,
) -> Option<Vec<bool>> {
    if settings.categories.enabled.is_empty() {
        return None;
    }

    let enabled_categories = categories
        .iter()
        .map(|category| {
            settings
                .categories
                .enabled
                .iter()
                .any(|key| settings::category_matches_key(category, key))
        })
        .collect::<Vec<_>>();

    enabled_categories
        .iter()
        .any(|enabled| *enabled)
        .then_some(enabled_categories)
}

fn select_initial_category(
    categories: &[Category],
    enabled_categories: &mut [bool],
    requested: Option<&str>,
) -> Result<usize> {
    if let Some(requested) = requested {
        let selected_category = find_category(categories, requested)
            .ok_or_else(|| KiteError::CategoryNotFound(requested.to_owned()))?;
        if let Some(enabled) = enabled_categories.get_mut(selected_category) {
            *enabled = true;
        }
        return Ok(selected_category);
    }

    if let Some(world) = find_category(categories, "World")
        && enabled_categories.get(world).copied().unwrap_or(false)
    {
        return Ok(world);
    }

    Ok(enabled_categories
        .iter()
        .position(|enabled| *enabled)
        .unwrap_or(0))
}

fn default_enabled_categories(categories: &[Category]) -> Vec<bool> {
    let mut enabled_categories = categories
        .iter()
        .map(|category| {
            DEFAULT_ENABLED_CATEGORY_KEYS
                .iter()
                .any(|default| category_matches_default_key(category, default))
        })
        .collect::<Vec<_>>();

    if !enabled_categories.iter().any(|enabled| *enabled)
        && let Some(first_category) = enabled_categories.first_mut()
    {
        *first_category = true;
    }

    enabled_categories
}

fn category_matches_default_key(category: &Category, key: &str) -> bool {
    let key = settings::normalize_category_key(key);
    [
        category.name.as_str(),
        category.file.as_str(),
        category.file_stem(),
    ]
    .into_iter()
    .any(|value| settings::normalize_category_key(value) == key)
}

fn category_matches_filter(category: &Category, filter: &str) -> bool {
    let filter = filter.trim().to_ascii_lowercase();
    filter.is_empty()
        || category.name.to_ascii_lowercase().contains(&filter)
        || category.file.to_ascii_lowercase().contains(&filter)
        || category.file_stem().to_ascii_lowercase().contains(&filter)
}

struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn categories() -> Vec<Category> {
        vec![
            Category {
                name: "World".to_owned(),
                file: "world.json".to_owned(),
                feed_url: Url::parse("https://news.kagi.com/world.xml").unwrap(),
            },
            Category {
                name: "Technology".to_owned(),
                file: "tech.json".to_owned(),
                feed_url: Url::parse("https://news.kagi.com/tech.xml").unwrap(),
            },
        ]
    }

    use url::Url;

    fn category(name: &str, file: &str) -> Category {
        Category {
            name: name.to_owned(),
            file: file.to_owned(),
            feed_url: Url::parse(&format!(
                "https://news.kagi.com/{}.xml",
                file.strip_suffix(".json").unwrap()
            ))
            .unwrap(),
        }
    }

    fn state_with_categories(categories: Vec<Category>) -> AppState {
        AppState {
            enabled_categories: vec![true; categories.len()],
            categories,
            keybinds: KeyBindings::default(),
            selected_category: 0,
            loaded_category: Some(0),
            articles: Vec::new(),
            selected_article: 0,
            focus: Focus::Categories,
            status: String::new(),
            error: None,
            category_filter: String::new(),
            category_filter_active: false,
            settings_open: false,
            settings_section: SettingsSection::Categories,
            config_selected_category: 0,
            config_filter: String::new(),
            config_filter_active: false,
            selected_keybind: 0,
            editing_keybind: None,
            keybind_input: String::new(),
            help_open: false,
            detail_open: false,
            detail_scroll: 0,
            pending_key_sequence: String::new(),
            should_quit: false,
        }
    }

    fn article(title: &str) -> Article {
        Article {
            id: uuid::Uuid::nil(),
            title: title.to_owned(),
            link: None,
            summary: "First line.\nSecond line.".to_owned(),
            summary_blocks: Vec::new(),
            published_at: None,
            categories: vec!["World".to_owned()],
        }
    }

    fn key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    #[test]
    fn finds_category_by_name_file_or_stem() {
        let categories = categories();

        assert_eq!(find_category(&categories, "technology"), Some(1));
        assert_eq!(find_category(&categories, "tech.json"), Some(1));
        assert_eq!(find_category(&categories, "tech"), Some(1));
        assert_eq!(find_category(&categories, "missing"), None);
    }

    #[test]
    fn focus_cycles_between_categories_and_articles() {
        let mut state = state_with_categories(categories());

        state.next_focus();
        assert_eq!(state.focus, Focus::Articles);

        state.next_focus();
        assert_eq!(state.focus, Focus::Categories);
    }

    #[test]
    fn category_filter_matches_name_file_or_stem() {
        let mut state = state_with_categories(categories());

        state.category_filter = "tech".to_owned();
        assert_eq!(state.filtered_category_indices(), vec![1]);

        state.category_filter = "json".to_owned();
        assert_eq!(state.filtered_category_indices(), vec![0, 1]);
    }

    #[test]
    fn category_navigation_uses_filtered_matches() {
        let mut state = state_with_categories(vec![
            category("World", "world.json"),
            category("Technology", "tech.json"),
            category("Top Stories", "top.json"),
        ]);
        state.enabled_categories = vec![false, true, true];
        state.category_filter = "t".to_owned();

        state.sync_selected_category_to_filter();
        assert_eq!(state.selected_category, 1);

        state.move_next();
        assert_eq!(state.selected_category, 2);

        state.move_next();
        assert_eq!(state.selected_category, 1);
    }

    #[test]
    fn default_category_config_matches_web_defaults() {
        let categories = vec![
            category("World", "world.json"),
            category("Gaming", "gaming.json"),
            category("Science", "science.json"),
            category("AI", "ai.json"),
            category("Technology", "technology.json"),
            category("Business", "business.json"),
            category("Sports", "sports.json"),
            category("Today in History", "today_in_history.json"),
            category("USA", "usa.json"),
            category("Climate Change", "climate_change.json"),
        ];

        let enabled = default_enabled_categories(&categories);
        let enabled_names = categories
            .iter()
            .zip(enabled)
            .filter_map(|(category, enabled)| enabled.then_some(category.name.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            enabled_names,
            vec![
                "World",
                "Gaming",
                "Science",
                "AI",
                "Technology",
                "Business",
                "Sports",
                "Today in History",
                "USA"
            ]
        );
    }

    #[test]
    fn initial_category_outside_defaults_is_enabled() {
        let categories = vec![
            category("World", "world.json"),
            category("Climate Change", "climate_change.json"),
        ];

        let mut enabled = default_enabled_categories(&categories);
        let selected_category =
            select_initial_category(&categories, &mut enabled, Some("Climate Change")).unwrap();

        assert_eq!(selected_category, 1);
        assert_eq!(enabled, vec![true, true]);
    }

    #[test]
    fn default_initial_category_prefers_world_when_visible() {
        let categories = vec![
            category("Technology", "technology.json"),
            category("World", "world.json"),
        ];
        let mut enabled = default_enabled_categories(&categories);

        let selected_category = select_initial_category(&categories, &mut enabled, None).unwrap();

        assert_eq!(selected_category, 1);
    }

    #[test]
    fn default_initial_category_uses_first_enabled_when_world_is_hidden() {
        let categories = vec![
            category("Technology", "technology.json"),
            category("World", "world.json"),
        ];
        let mut enabled = vec![true, false];

        let selected_category = select_initial_category(&categories, &mut enabled, None).unwrap();

        assert_eq!(selected_category, 0);
    }

    #[test]
    fn category_config_keeps_one_category_enabled() {
        let mut state = state_with_categories(vec![category("World", "world.json")]);

        state.toggle_config_category();

        assert!(state.is_category_enabled(0));
        assert_eq!(state.status, "At least one category must stay shown");
    }

    #[test]
    fn saved_category_config_loads_matching_categories() {
        let categories = vec![
            category("World", "world.json"),
            category("Technology", "technology.json"),
            category("Today in History", "today_in_history.json"),
        ];
        let settings = Settings {
            categories: CategorySettings {
                enabled: vec!["technology".to_owned(), "todayinhistory".to_owned()],
            },
            ..Settings::default()
        };

        let enabled = enabled_categories_from_settings(&categories, &settings).unwrap();

        assert_eq!(enabled, vec![false, true, true]);
    }

    #[test]
    fn empty_saved_category_config_falls_back_to_defaults() {
        let categories = vec![category("World", "world.json")];
        let settings = Settings::default();

        assert!(enabled_categories_from_settings(&categories, &settings).is_none());
    }

    #[test]
    fn current_settings_uses_stable_category_keys() {
        let mut state = state_with_categories(vec![
            category("World", "world.json"),
            category("Today in History", "today_in_history.json"),
        ]);
        state.enabled_categories = vec![false, true];

        assert_eq!(
            state.current_settings(),
            Settings {
                categories: CategorySettings {
                    enabled: vec!["todayinhistory".to_owned()]
                },
                keybinds: KeyBindingSettings::default(),
            }
        );
    }

    #[test]
    fn default_settings_keybind_is_comma() {
        assert_eq!(KeyBindings::default().settings, ",");
    }

    #[test]
    fn keybinds_use_configured_single_character_values() {
        let settings = KeyBindingSettings {
            help: "h".to_owned(),
            settings: ";".to_owned(),
            category_filter: "f".to_owned(),
            refresh: "u".to_owned(),
            quit: "x".to_owned(),
            reset_defaults: "D".to_owned(),
            jump_top: "tt".to_owned(),
            jump_bottom: "B".to_owned(),
        };

        assert_eq!(
            KeyBindings::from_settings(&settings),
            KeyBindings {
                help: "h".to_owned(),
                settings: ";".to_owned(),
                category_filter: "f".to_owned(),
                refresh: "u".to_owned(),
                quit: "x".to_owned(),
                reset_defaults: "D".to_owned(),
                jump_top: "tt".to_owned(),
                jump_bottom: "B".to_owned(),
            }
        );
    }

    #[test]
    fn invalid_keybind_values_fall_back_to_defaults() {
        let settings = KeyBindingSettings {
            help: String::new(),
            settings: "two".to_owned(),
            category_filter: "\n".to_owned(),
            refresh: "u".to_owned(),
            quit: "x".to_owned(),
            reset_defaults: "D".to_owned(),
            jump_top: String::new(),
            jump_bottom: "\n".to_owned(),
        };

        let keybinds = KeyBindings::from_settings(&settings);

        assert_eq!(keybinds.help, "?");
        assert_eq!(keybinds.settings, ",");
        assert_eq!(keybinds.category_filter, "/");
        assert_eq!(keybinds.refresh, "u");
        assert_eq!(keybinds.quit, "x");
        assert_eq!(keybinds.reset_defaults, "D");
        assert_eq!(keybinds.jump_top, "gg");
        assert_eq!(keybinds.jump_bottom, "G");
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
    }

    #[test]
    fn article_sequence_jumps_to_first_and_last_article() {
        let mut state = state_with_categories(categories());
        state.focus = Focus::Articles;
        state.articles = vec![article("One"), article("Two"), article("Three")];
        state.selected_article = 1;

        assert!(handle_article_sequence_key(&mut state, key('g')));
        assert_eq!(state.selected_article, 1);
        assert!(handle_article_sequence_key(&mut state, key('g')));
        assert_eq!(state.selected_article, 0);

        assert!(handle_article_sequence_key(&mut state, key('G')));
        assert_eq!(state.selected_article, 2);
    }

    #[test]
    fn article_sequence_jumps_within_detail_view() {
        let mut state = state_with_categories(categories());
        state.focus = Focus::Articles;
        state.articles = vec![article("One")];
        state.detail_open = true;
        state.detail_scroll = 4;

        assert!(handle_article_sequence_key(&mut state, key('g')));
        assert!(handle_article_sequence_key(&mut state, key('g')));
        assert_eq!(state.detail_scroll, 0);

        assert!(handle_article_sequence_key(&mut state, key('G')));
        assert!(state.detail_scroll > 0);
    }
}
