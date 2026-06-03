//! Shared fixtures for the `app` submodule unit tests.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use url::Url;

use crate::app::keybindings::KeyBindings;
use crate::app::state::{AppState, Focus, SettingsSection};
use crate::app::theme_select::ThemeSelectionMode;
use crate::models::{Article, Category};
use crate::read_state::ReadArticles;
use crate::settings::ThemeSettings;
use crate::theme::{ANSI_THEME_ID, PlatformColorScheme, Theme, ThemeCatalog};

pub(crate) fn categories() -> Vec<Category> {
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

pub(crate) fn category(name: &str, file: &str) -> Category {
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

pub(crate) fn state_with_categories(categories: Vec<Category>) -> AppState {
    AppState {
        enabled_categories: vec![true; categories.len()],
        categories,
        keybinds: KeyBindings::default(),
        theme: Theme::ansi(),
        theme_settings: ThemeSettings::Fixed(ANSI_THEME_ID.to_owned()),
        themes: ThemeCatalog::built_in(),
        platform_color_scheme: PlatformColorScheme::Unspecified,
        selected_category: 0,
        loaded_category: Some(0),
        articles: Vec::new(),
        read_articles: ReadArticles::empty_for_today(),
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
        selected_theme: 0,
        selected_theme_mode: ThemeSelectionMode::Device,
        theme_dropdown_open: false,
        help_open: false,
        detail_open: false,
        detail_scroll: 0,
        pending_key_sequence: String::new(),
        should_quit: false,
    }
}

pub(crate) fn article(title: &str) -> Article {
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

pub(crate) fn key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}
