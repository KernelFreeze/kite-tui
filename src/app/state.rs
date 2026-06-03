//! [`AppState`]: the entire mutable state of the TUI plus the methods that query
//! and mutate it. Input handling lives in [`super::events`]; this module only
//! owns the state transitions those handlers call.

use crate::app::categories::{
    category_matches_filter, default_enabled_categories, enabled_categories_from_settings,
    find_category_by_settings_key, select_initial_category,
};
use crate::app::keybindings::{
    KeyBindingAction, KeyBindings, is_named_key_binding, key_sequence_label,
    normalize_key_sequence, valid_key_sequence, valid_key_sequence_char, valid_single_key_binding,
};
use crate::app::theme_select::{
    ThemeSelectionMode, ThemeSlot, theme_id_for_settings, theme_id_for_variants,
    theme_slot_for_mode, theme_slot_for_selection,
};
use crate::error::Result;
use crate::kagi::KagiClient;
use crate::models::{Article, Category, SummaryBlock};
use crate::read_state::ReadArticles;
use crate::settings::{
    self, CategorySettings, Settings, ThemeMode, ThemeSettings, ThemeVariantSettings,
};
use crate::theme::{
    ANSI_THEME_ID, PlatformColorScheme, Theme, ThemeCatalog, detect_platform_color_scheme,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Categories,
    Articles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Categories,
    Keybinds,
    Themes,
}

impl SettingsSection {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Categories => Self::Keybinds,
            Self::Keybinds => Self::Themes,
            Self::Themes => Self::Categories,
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Categories => Self::Themes,
            Self::Keybinds => Self::Categories,
            Self::Themes => Self::Keybinds,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Categories => "Categories",
            Self::Keybinds => "Keybinds",
            Self::Themes => "Themes",
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub categories: Vec<Category>,
    pub enabled_categories: Vec<bool>,
    pub keybinds: KeyBindings,
    pub theme: Theme,
    pub theme_settings: ThemeSettings,
    pub themes: ThemeCatalog,
    pub platform_color_scheme: PlatformColorScheme,
    pub selected_category: usize,
    pub loaded_category: Option<usize>,
    pub articles: Vec<Article>,
    pub read_articles: ReadArticles,
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
    pub selected_theme: usize,
    pub selected_theme_mode: ThemeSelectionMode,
    pub theme_dropdown_open: bool,
    pub help_open: bool,
    pub detail_open: bool,
    pub detail_scroll: u16,
    pub(crate) pending_key_sequence: String,
    pub(crate) should_quit: bool,
}

impl AppState {
    pub async fn bootstrap(client: &KagiClient, initial_category: Option<&str>) -> Result<Self> {
        let categories = client.categories().await?;
        let (settings, settings_error) = load_settings();
        let (read_articles, read_articles_error) = load_read_articles();
        let (themes, mut theme_errors) = ThemeCatalog::load();
        let (platform_color_scheme, platform_error) = load_platform_color_scheme();
        if let Some(error) = platform_error {
            theme_errors.push(error);
        }
        let (theme, selected_theme, theme_error) = themes.selected_theme(theme_id_for_settings(
            &settings.theme,
            platform_color_scheme,
        ));
        if let Some(error) = theme_error {
            theme_errors.push(error);
        }
        let selected_theme_mode = ThemeSelectionMode::from_settings(&settings.theme);
        let mut enabled_categories = enabled_categories_from_settings(&categories, &settings)
            .unwrap_or_else(|| default_enabled_categories(&categories));
        let keybinds = KeyBindings::from_settings(&settings.keybinds);
        let selected_category =
            select_initial_category(&categories, &mut enabled_categories, initial_category)?;

        let mut state = Self {
            categories,
            enabled_categories,
            keybinds,
            theme,
            theme_settings: settings.theme,
            themes,
            platform_color_scheme,
            selected_category,
            loaded_category: None,
            articles: Vec::new(),
            read_articles,
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
            selected_theme,
            selected_theme_mode,
            theme_dropdown_open: false,
            help_open: false,
            detail_open: false,
            detail_scroll: 0,
            pending_key_sequence: String::new(),
            should_quit: false,
        };

        state.load_selected_category(client).await;
        if state.error.is_none() {
            state.error = startup_error(settings_error, read_articles_error, theme_errors);
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

    pub(crate) async fn refresh_all(&mut self, client: &KagiClient) {
        self.status = "Refreshing categories".to_owned();
        self.error = None;

        match client.categories().await {
            Ok(categories) => {
                self.apply_refreshed_categories(categories);
                if self.selected_category_matches_filter() {
                    self.load_selected_category(client).await;
                } else {
                    self.update_category_filter_status();
                }
            }
            Err(error) => {
                self.status = "Could not refresh categories".to_owned();
                self.error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn apply_refreshed_categories(&mut self, categories: Vec<Category>) {
        if categories.is_empty() {
            return;
        }

        let settings = self.current_settings();
        let selected_key = self.selected_category().map(settings::category_key);
        let mut enabled_categories = enabled_categories_from_settings(&categories, &settings)
            .unwrap_or_else(|| default_enabled_categories(&categories));

        let selected_category = selected_key
            .as_deref()
            .and_then(|key| find_category_by_settings_key(&categories, key))
            .filter(|index| enabled_categories.get(*index).copied().unwrap_or(false))
            .unwrap_or_else(|| {
                select_initial_category(&categories, &mut enabled_categories, None).unwrap_or(0)
            });

        self.categories = categories;
        self.enabled_categories = enabled_categories;
        self.selected_category = selected_category;
        self.loaded_category = None;
        self.articles.clear();
        self.selected_article = 0;
        self.detail_open = false;
        self.detail_scroll = 0;
        self.config_selected_category = self.selected_category;
        self.sync_selected_category_to_filter();
        self.sync_config_selected_category_to_filter();
        self.status = format!("Refreshed {} categories", self.categories.len());
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

    pub fn is_article_read(&self, article: &Article) -> bool {
        self.read_articles.is_read(article)
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

    pub(crate) fn selected_category_matches_filter(&self) -> bool {
        self.is_category_enabled(self.selected_category)
            && self
                .selected_category()
                .is_some_and(|category| self.category_matches_filter(category))
    }

    pub(crate) fn start_category_filter(&mut self) {
        self.pending_key_sequence.clear();
        self.category_filter_active = true;
        self.focus = Focus::Categories;
        self.error = None;
        self.sync_selected_category_to_filter();
        self.update_category_filter_status();
    }

    pub(crate) fn finish_category_filter(&mut self) {
        self.category_filter_active = false;
        if self.has_category_filter() {
            self.update_category_filter_status();
        } else {
            self.status = "Category filter cleared".to_owned();
        }
    }

    pub(crate) fn clear_category_filter(&mut self) {
        self.category_filter.clear();
        self.category_filter_active = false;
        self.status = "Category filter cleared".to_owned();
        self.error = None;
    }

    pub(crate) fn push_category_filter(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }

        self.category_filter.push(ch);
        self.sync_selected_category_to_filter();
        self.update_category_filter_status();
    }

    pub(crate) fn pop_category_filter(&mut self) {
        self.category_filter.pop();
        self.sync_selected_category_to_filter();
        self.update_category_filter_status();
    }

    pub(crate) fn clear_category_filter_input(&mut self) {
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

    pub(crate) fn update_category_filter_status(&mut self) {
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

    pub(crate) fn open_settings(&mut self) {
        self.pending_key_sequence.clear();
        self.settings_open = true;
        self.settings_section = SettingsSection::Categories;
        self.config_selected_category = self.selected_category;
        self.theme_dropdown_open = false;
        self.sync_selected_theme();
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

    pub(crate) fn close_settings(&mut self) {
        self.settings_open = false;
        self.config_filter_active = false;
        self.theme_dropdown_open = false;
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = format!(
            "{} categories shown, {} hidden",
            self.enabled_category_count(),
            self.hidden_category_count()
        );
        self.sync_selected_category_to_filter();
    }

    pub(crate) fn next_settings_section(&mut self) {
        self.settings_section = self.settings_section.next();
        self.config_filter_active = false;
        self.theme_dropdown_open = false;
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.update_settings_status();
    }

    pub(crate) fn previous_settings_section(&mut self) {
        self.settings_section = self.settings_section.previous();
        self.config_filter_active = false;
        self.theme_dropdown_open = false;
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.update_settings_status();
    }

    pub(crate) fn open_help(&mut self) {
        self.pending_key_sequence.clear();
        self.help_open = true;
        self.error = None;
    }

    pub(crate) fn close_help(&mut self) {
        self.help_open = false;
    }

    pub(crate) fn start_config_filter(&mut self) {
        self.config_filter_active = true;
        self.error = None;
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    pub(crate) fn finish_config_filter(&mut self) {
        self.config_filter_active = false;
        self.update_category_config_status();
    }

    pub(crate) fn clear_config_filter(&mut self) {
        self.config_filter.clear();
        self.config_filter_active = false;
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    pub(crate) fn push_config_filter(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }

        self.config_filter.push(ch);
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    pub(crate) fn pop_config_filter(&mut self) {
        self.config_filter.pop();
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    pub(crate) fn clear_config_filter_input(&mut self) {
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
            SettingsSection::Themes => self.update_theme_settings_status(),
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

    pub(crate) fn move_config_category_by(&mut self, step: isize, wrap: bool) {
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

    pub(crate) fn toggle_config_category(&mut self) {
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

    pub(crate) fn reset_default_category_config(&mut self) {
        self.enabled_categories = default_enabled_categories(&self.categories);
        self.sync_selected_category_to_filter();
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
        self.persist_settings();
    }

    pub(crate) fn move_keybind_by(&mut self, step: isize, wrap: bool) {
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

    pub(crate) fn start_keybind_edit(&mut self) {
        let Some(action) = self.selected_keybind_action() else {
            return;
        };

        self.editing_keybind = Some(action);
        self.keybind_input.clear();
        self.update_keybind_settings_status();
    }

    pub(crate) fn cancel_keybind_edit(&mut self) {
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = "Keybind edit cancelled".to_owned();
    }

    pub(crate) fn push_keybind_input(&mut self, ch: char) {
        if !valid_key_sequence_char(ch) {
            self.status = "Keybinds must use printable keys, Tab, or Shift+Tab".to_owned();
            return;
        }

        if is_named_key_binding(&self.keybind_input) {
            self.keybind_input.clear();
        }
        self.keybind_input.push(ch);
        self.update_keybind_settings_status();
    }

    pub(crate) fn set_keybind_input(&mut self, sequence: &str) {
        self.keybind_input = sequence.to_owned();
        self.update_keybind_settings_status();
    }

    pub(crate) fn pop_keybind_input(&mut self) {
        if is_named_key_binding(&self.keybind_input) {
            self.keybind_input.clear();
        } else {
            self.keybind_input.pop();
        }
        self.update_keybind_settings_status();
    }

    pub(crate) fn clear_keybind_input(&mut self) {
        self.keybind_input.clear();
        self.update_keybind_settings_status();
    }

    pub(crate) fn finish_keybind_edit(&mut self, action: KeyBindingAction) {
        let sequence = normalize_key_sequence(&self.keybind_input);
        if action.supports_sequences() && !valid_key_sequence(&sequence) {
            self.status = "Key sequences must use printable keys".to_owned();
            return;
        }
        if !action.supports_sequences() && !valid_single_key_binding(&sequence) {
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

        self.keybinds.set(action, sequence.clone());
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = format!(
            "{} keybind set to {}",
            action.label(),
            key_sequence_label(&sequence)
        );
        self.persist_settings();
    }

    pub(crate) fn reset_default_keybinds(&mut self) {
        self.keybinds = KeyBindings::default();
        self.editing_keybind = None;
        self.keybind_input.clear();
        self.status = "Keybinds restored to defaults".to_owned();
        self.persist_settings();
    }

    fn update_theme_settings_status(&mut self) {
        if let Some(theme) = self.themes.themes().get(self.selected_theme) {
            let marker = if theme.id == self.selected_theme_slot_id() {
                "current"
            } else {
                "available"
            };
            self.status = format!(
                "{} mode: selected {} for {} ({marker})",
                self.selected_theme_mode.title(),
                theme.name,
                self.selected_theme_slot_label()
            );
        } else {
            self.status = "No themes available".to_owned();
        }
    }

    pub(crate) fn move_theme_by(&mut self, step: isize, wrap: bool) {
        let len = self.themes.themes().len();
        if len == 0 {
            self.update_theme_settings_status();
            return;
        }

        let current = self.selected_theme.min(len.saturating_sub(1)) as isize;
        let last = len as isize - 1;
        let next = if wrap {
            (current + step).rem_euclid(len as isize)
        } else {
            (current + step).clamp(0, last)
        };

        self.selected_theme = next as usize;
        self.update_theme_settings_status();
    }

    pub(crate) fn toggle_theme_mode_dropdown(&mut self) {
        self.theme_dropdown_open = !self.theme_dropdown_open;
        self.update_theme_settings_status();
    }

    pub(crate) fn close_theme_mode_dropdown(&mut self) {
        self.theme_dropdown_open = false;
        self.update_theme_settings_status();
    }

    pub(crate) fn move_theme_mode_by(&mut self, step: isize) {
        let mode = self.selected_theme_mode.move_by(step);
        self.set_theme_selection_mode(mode);
    }

    fn set_theme_selection_mode(&mut self, mode: ThemeSelectionMode) {
        self.selected_theme_mode = mode;
        let mut variants = self.variant_theme_settings();
        variants.mode = mode.theme_mode();
        self.theme_settings = ThemeSettings::Variants(variants);
        self.apply_theme_settings();
        self.sync_selected_theme();
        self.status = format!("Theme mode set to {}", mode.title());
        self.persist_settings();
    }

    pub(crate) fn select_theme(&mut self) {
        let Some(theme) = self.themes.themes().get(self.selected_theme).cloned() else {
            self.status = "No themes available".to_owned();
            return;
        };

        let theme_id = theme.id.clone();
        let mut variants = self.variant_theme_settings();
        variants.mode = self.selected_theme_mode.theme_mode();
        match theme_slot_for_mode(variants.mode, self.platform_color_scheme) {
            ThemeSlot::Light => variants.light = theme_id,
            ThemeSlot::Dark => variants.dark = theme_id,
            ThemeSlot::Unspecified => variants.unspecified = theme_id,
        };
        self.theme_settings = ThemeSettings::Variants(variants);

        self.apply_theme_settings();
        self.sync_selected_theme();
        self.status = format!(
            "Theme set to {} for {}",
            theme.name,
            self.selected_theme_slot_label()
        );
        self.persist_settings();
    }

    pub(crate) fn reset_default_theme(&mut self) {
        self.theme_settings = ThemeSettings::Variants(ThemeVariantSettings::default());
        self.selected_theme_mode = ThemeSelectionMode::Device;
        self.theme_dropdown_open = false;
        self.apply_theme_settings();
        self.sync_selected_theme();
        self.status = "Theme restored to ANSI".to_owned();
        self.persist_settings();
    }

    fn sync_selected_theme(&mut self) {
        self.selected_theme = self
            .themes
            .index_of(self.selected_theme_slot_id())
            .unwrap_or_else(|| self.themes.index_of(ANSI_THEME_ID).unwrap_or(0));
    }

    pub fn selected_theme_slot_id(&self) -> &str {
        match &self.theme_settings {
            ThemeSettings::Fixed(theme) => theme,
            ThemeSettings::Variants(variants) => theme_id_for_variants(
                variants,
                self.selected_theme_mode,
                self.platform_color_scheme,
            ),
        }
    }

    pub fn selected_theme_slot_label(&self) -> &'static str {
        match self.selected_theme_mode {
            ThemeSelectionMode::Light => "light mode",
            ThemeSelectionMode::Dark => "dark mode",
            ThemeSelectionMode::Device => match self.platform_color_scheme {
                PlatformColorScheme::Light => "device light mode",
                PlatformColorScheme::Dark => "device dark mode",
                PlatformColorScheme::Unspecified => "unspecified device mode",
            },
        }
    }

    pub fn light_theme_name(&self) -> String {
        let variants = self.variant_theme_settings();
        self.theme_name_for_id(&variants.light)
    }

    pub fn dark_theme_name(&self) -> String {
        let variants = self.variant_theme_settings();
        self.theme_name_for_id(&variants.dark)
    }

    pub fn light_theme_selected(&self) -> bool {
        matches!(
            theme_slot_for_selection(self.selected_theme_mode, self.platform_color_scheme),
            ThemeSlot::Light
        )
    }

    pub fn dark_theme_selected(&self) -> bool {
        matches!(
            theme_slot_for_selection(self.selected_theme_mode, self.platform_color_scheme),
            ThemeSlot::Dark
        )
    }

    fn theme_name_for_id(&self, id: &str) -> String {
        self.themes
            .themes()
            .iter()
            .find(|theme| theme.id == id)
            .map(|theme| theme.name.clone())
            .unwrap_or_else(|| id.to_owned())
    }

    fn variant_theme_settings(&self) -> ThemeVariantSettings {
        match &self.theme_settings {
            ThemeSettings::Fixed(theme) => ThemeVariantSettings {
                light: theme.clone(),
                dark: theme.clone(),
                unspecified: theme.clone(),
                ..ThemeVariantSettings::default()
            },
            ThemeSettings::Variants(variants) => variants.clone(),
        }
    }

    fn apply_theme_settings(&mut self) {
        let requested = theme_id_for_settings(&self.theme_settings, self.platform_color_scheme);
        let (theme, selected_theme, theme_error) = self.themes.selected_theme(requested);
        self.theme = theme;
        self.selected_theme = selected_theme;
        if let Some(error) = theme_error {
            self.error = Some(error);
        }
    }

    pub(crate) fn refresh_platform_color_scheme(&mut self) {
        let Ok(color_scheme) = detect_platform_color_scheme() else {
            return;
        };
        if color_scheme == self.platform_color_scheme {
            return;
        }

        self.platform_color_scheme = color_scheme;
        if matches!(
            &self.theme_settings,
            ThemeSettings::Variants(ThemeVariantSettings {
                mode: ThemeMode::Device,
                ..
            })
        ) {
            self.apply_theme_settings();
            self.sync_selected_theme();
        }
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
            theme: self.theme_settings.clone(),
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

    pub(crate) fn move_next(&mut self) {
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

    pub(crate) fn move_previous(&mut self) {
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

    pub(crate) fn page_next(&mut self) {
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

    pub(crate) fn page_previous(&mut self) {
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

    pub(crate) fn jump_to_top(&mut self) {
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

    pub(crate) fn jump_to_bottom(&mut self) {
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

    pub(crate) fn select_next_category(&mut self) -> bool {
        self.select_category_by(1)
    }

    pub(crate) fn select_previous_category(&mut self) -> bool {
        self.select_category_by(-1)
    }

    fn select_category_by(&mut self, step: isize) -> bool {
        if self.detail_open {
            return false;
        }

        self.move_category_by(step, true);
        self.focus = Focus::Articles;
        self.selected_category_matches_filter()
            && self.loaded_category != Some(self.selected_category)
    }

    pub(crate) fn open_detail(&mut self) {
        let Some(article_id) = self.selected_article().map(|article| article.id) else {
            return;
        };

        self.pending_key_sequence.clear();
        self.detail_open = true;
        self.detail_scroll = 0;
        if let Err(error) = self.read_articles.mark_read_id(article_id) {
            self.error = Some(error.to_string());
        }
    }

    pub(crate) fn close_detail(&mut self) {
        self.pending_key_sequence.clear();
        self.detail_open = false;
        self.detail_scroll = 0;
    }

    pub(crate) fn quit(&mut self) {
        self.should_quit = true;
    }
}

fn load_settings() -> (Settings, Option<String>) {
    match Settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => (Settings::default(), Some(error.to_string())),
    }
}

fn load_platform_color_scheme() -> (PlatformColorScheme, Option<String>) {
    match detect_platform_color_scheme() {
        Ok(color_scheme) => (color_scheme, None),
        Err(error) => (
            PlatformColorScheme::Unspecified,
            Some(format!("could not detect platform color scheme: {error}")),
        ),
    }
}

fn load_read_articles() -> (ReadArticles, Option<String>) {
    match ReadArticles::load() {
        Ok(read_articles) => (read_articles, None),
        Err(error) => (ReadArticles::empty_for_today(), Some(error.to_string())),
    }
}

fn startup_error(
    settings_error: Option<String>,
    read_articles_error: Option<String>,
    theme_errors: Vec<String>,
) -> Option<String> {
    let mut errors = [settings_error, read_articles_error]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    errors.extend(theme_errors);

    (!errors.is_empty()).then(|| errors.join("; "))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_support::{article, categories, category, state_with_categories};
    use crate::settings::KeyBindingSettings;

    #[test]
    fn tab_category_navigation_advances_categories_from_articles() {
        let mut state = state_with_categories(categories());
        state.focus = Focus::Articles;

        assert!(state.select_next_category());
        assert_eq!(state.selected_category, 1);
        assert_eq!(state.focus, Focus::Articles);
        state.loaded_category = Some(state.selected_category);

        assert!(state.select_next_category());
        assert_eq!(state.selected_category, 0);
        assert_eq!(state.focus, Focus::Articles);
    }

    #[test]
    fn shift_tab_category_navigation_goes_to_previous_category() {
        let mut state = state_with_categories(categories());
        state.focus = Focus::Articles;

        assert!(state.select_previous_category());
        assert_eq!(state.selected_category, 1);
        assert_eq!(state.focus, Focus::Articles);
        state.loaded_category = Some(state.selected_category);

        assert!(state.select_previous_category());
        assert_eq!(state.selected_category, 0);
        assert_eq!(state.focus, Focus::Articles);
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
    fn category_config_keeps_one_category_enabled() {
        let mut state = state_with_categories(vec![category("World", "world.json")]);

        state.toggle_config_category();

        assert!(state.is_category_enabled(0));
        assert_eq!(state.status, "At least one category must stay shown");
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
                theme: ThemeSettings::Fixed("ansi".to_owned()),
                categories: CategorySettings {
                    enabled: vec!["todayinhistory".to_owned()]
                },
                keybinds: KeyBindingSettings::default(),
            }
        );
    }

    #[test]
    fn current_settings_persists_selected_theme() {
        let mut state = state_with_categories(categories());
        state.set_theme_selection_mode(ThemeSelectionMode::Dark);
        state.selected_theme = state.themes.index_of("dracula").unwrap();

        state.select_theme();

        assert_eq!(
            state.current_settings().theme,
            ThemeSettings::Variants(ThemeVariantSettings {
                mode: ThemeMode::Dark,
                light: "ansi".to_owned(),
                dark: "dracula".to_owned(),
                unspecified: "ansi".to_owned(),
            })
        );
    }

    #[test]
    fn current_settings_persists_device_theme_variants() {
        let mut state = state_with_categories(categories());
        state.theme_settings = ThemeSettings::Variants(ThemeVariantSettings {
            mode: ThemeMode::Device,
            light: "catppuccin-latte".to_owned(),
            dark: "catppuccin-mocha".to_owned(),
            unspecified: "ansi".to_owned(),
        });

        assert_eq!(
            state.current_settings().theme,
            ThemeSettings::Variants(ThemeVariantSettings {
                mode: ThemeMode::Device,
                light: "catppuccin-latte".to_owned(),
                dark: "catppuccin-mocha".to_owned(),
                unspecified: "ansi".to_owned(),
            })
        );
    }

    #[test]
    fn settings_sections_cycle_through_themes() {
        assert_eq!(
            SettingsSection::Categories.next(),
            SettingsSection::Keybinds
        );
        assert_eq!(SettingsSection::Keybinds.next(), SettingsSection::Themes);
        assert_eq!(SettingsSection::Themes.next(), SettingsSection::Categories);
        assert_eq!(
            SettingsSection::Categories.previous(),
            SettingsSection::Themes
        );
    }

    #[test]
    fn theme_settings_navigation_moves_through_available_themes() {
        let mut state = state_with_categories(categories());
        state.settings_section = SettingsSection::Themes;

        state.move_theme_by(1, true);

        assert_eq!(state.selected_theme, 1);
        assert_eq!(
            state.status,
            "Device mode: selected Catppuccin Mocha for unspecified device mode (available)"
        );

        state.move_theme_by(-1, true);

        assert_eq!(state.selected_theme, 0);
        assert_eq!(
            state.status,
            "Device mode: selected ANSI for unspecified device mode (current)"
        );
    }

    #[test]
    fn theme_mode_dropdown_has_no_fixed_option() {
        assert_eq!(
            ThemeSelectionMode::ALL
                .into_iter()
                .map(ThemeSelectionMode::title)
                .collect::<Vec<_>>(),
            vec!["Device", "Light", "Dark"]
        );
    }

    #[test]
    fn selecting_theme_in_device_dark_mode_updates_dark_variant() {
        let mut state = state_with_categories(categories());
        state.platform_color_scheme = PlatformColorScheme::Dark;

        state.set_theme_selection_mode(ThemeSelectionMode::Device);
        state.selected_theme = state.themes.index_of("dracula").unwrap();
        state.select_theme();

        assert_eq!(
            state.current_settings().theme,
            ThemeSettings::Variants(ThemeVariantSettings {
                mode: ThemeMode::Device,
                light: "ansi".to_owned(),
                dark: "dracula".to_owned(),
                unspecified: "ansi".to_owned(),
            })
        );
        assert_eq!(state.theme.id, "dracula");
    }

    #[test]
    fn selecting_theme_in_unspecified_device_mode_updates_only_unspecified_variant() {
        let mut state = state_with_categories(categories());
        state.platform_color_scheme = PlatformColorScheme::Unspecified;
        state.theme_settings = ThemeSettings::Variants(ThemeVariantSettings {
            mode: ThemeMode::Device,
            light: "catppuccin-latte".to_owned(),
            dark: "catppuccin-mocha".to_owned(),
            unspecified: "ansi".to_owned(),
        });
        state.selected_theme_mode = ThemeSelectionMode::Device;
        state.selected_theme = state.themes.index_of("dracula").unwrap();

        state.select_theme();

        assert_eq!(
            state.current_settings().theme,
            ThemeSettings::Variants(ThemeVariantSettings {
                mode: ThemeMode::Device,
                light: "catppuccin-latte".to_owned(),
                dark: "catppuccin-mocha".to_owned(),
                unspecified: "dracula".to_owned(),
            })
        );
    }

    #[test]
    fn refreshed_categories_preserve_enabled_selection_by_stable_key() {
        let mut state = state_with_categories(vec![
            category("World", "world.json"),
            category("Today in History", "today_in_history.json"),
        ]);
        state.enabled_categories = vec![false, true];
        state.selected_category = 1;
        state.loaded_category = Some(1);
        state.articles = vec![article("Old")];
        state.detail_open = true;
        state.config_selected_category = 0;

        state.apply_refreshed_categories(vec![
            category("Technology", "technology.json"),
            category("Today in History", "today_in_history.json"),
            category("Business", "business.json"),
        ]);

        assert_eq!(state.categories.len(), 3);
        assert_eq!(state.enabled_categories, vec![false, true, false]);
        assert_eq!(state.selected_category, 1);
        assert_eq!(state.config_selected_category, 1);
        assert_eq!(state.loaded_category, None);
        assert!(state.articles.is_empty());
        assert!(!state.detail_open);
        assert_eq!(state.status, "Refreshed 3 categories");
    }

    #[test]
    fn opening_article_marks_it_read() {
        let mut state = state_with_categories(categories());
        let article = article("One");
        let article_id = article.id;
        state.focus = Focus::Articles;
        state.articles = vec![article];

        assert!(!state.is_article_read(&state.articles[0]));
        state.open_detail();

        assert!(state.detail_open);
        assert!(state.is_article_read(&state.articles[0]));
        assert_eq!(state.articles[0].id, article_id);
    }
}
