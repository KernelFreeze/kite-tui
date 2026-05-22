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
    models::{Article, Category},
    settings::{self, CategorySettings, Settings},
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

#[derive(Debug)]
pub struct AppState {
    pub categories: Vec<Category>,
    pub enabled_categories: Vec<bool>,
    pub selected_category: usize,
    pub loaded_category: Option<usize>,
    pub articles: Vec<Article>,
    pub selected_article: usize,
    pub focus: Focus,
    pub status: String,
    pub error: Option<String>,
    pub category_filter: String,
    pub category_filter_active: bool,
    pub config_open: bool,
    pub config_selected_category: usize,
    pub config_filter: String,
    pub config_filter_active: bool,
    pub detail_open: bool,
    pub detail_scroll: u16,
    should_quit: bool,
}

impl AppState {
    pub async fn bootstrap(client: &KagiClient, initial_category: Option<&str>) -> Result<Self> {
        let categories = client.categories().await?;
        let (mut enabled_categories, settings_error) = load_enabled_categories(&categories);
        let selected_category =
            select_initial_category(&categories, &mut enabled_categories, initial_category)?;

        let mut state = Self {
            categories,
            enabled_categories,
            selected_category,
            loaded_category: None,
            articles: Vec::new(),
            selected_article: 0,
            focus: Focus::Articles,
            status: "Loading articles".to_owned(),
            error: None,
            category_filter: String::new(),
            category_filter_active: false,
            config_open: false,
            config_selected_category: selected_category,
            config_filter: String::new(),
            config_filter_active: false,
            detail_open: false,
            detail_scroll: 0,
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

    fn open_category_config(&mut self) {
        self.config_open = true;
        self.config_selected_category = self.selected_category;
        self.config_filter_active = false;
        self.category_filter_active = false;
        self.detail_open = false;
        self.detail_scroll = 0;
        self.error = None;
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
    }

    fn close_category_config(&mut self) {
        self.config_open = false;
        self.config_filter_active = false;
        self.status = format!(
            "{} categories shown, {} hidden",
            self.enabled_category_count(),
            self.hidden_category_count()
        );
        self.sync_selected_category_to_filter();
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
        self.persist_category_config();
    }

    fn reset_default_category_config(&mut self) {
        self.enabled_categories = default_enabled_categories(&self.categories);
        self.sync_selected_category_to_filter();
        self.sync_config_selected_category_to_filter();
        self.update_category_config_status();
        self.persist_category_config();
    }

    fn persist_category_config(&mut self) {
        if let Err(error) = self.category_settings().save() {
            self.error = Some(error.to_string());
        }
    }

    fn category_settings(&self) -> Settings {
        Settings {
            categories: CategorySettings {
                enabled: self
                    .enabled_category_indices()
                    .into_iter()
                    .filter_map(|index| self.categories.get(index).map(settings::category_key))
                    .collect(),
            },
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

    fn next_focus(&mut self) {
        if self.detail_open {
            return;
        }

        self.focus = self.focus.next();
    }

    fn open_detail(&mut self) {
        if !self.articles.is_empty() {
            self.detail_open = true;
            self.detail_scroll = 0;
        }
    }

    fn close_detail(&mut self) {
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
    if state.config_open {
        handle_category_config_key(state, key);
        return;
    }

    if state.category_filter_active {
        handle_category_filter_key(state, key);
        return;
    }

    if state.detail_open {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
            KeyCode::Char('q') => state.quit(),
            KeyCode::Esc | KeyCode::Enter => state.close_detail(),
            KeyCode::Down | KeyCode::Char('j') => state.move_next(),
            KeyCode::Up | KeyCode::Char('k') => state.move_previous(),
            KeyCode::PageDown => state.page_next(),
            KeyCode::PageUp => state.page_previous(),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Char('c') => state.open_category_config(),
        KeyCode::Char('/') => state.start_category_filter(),
        KeyCode::Char('q') => state.quit(),
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
        KeyCode::Char('r') if state.selected_category_matches_filter() => {
            state.load_selected_category(client).await
        }
        KeyCode::Char('r') => state.update_category_filter_status(),
        _ => {}
    }
}

fn handle_category_config_key(state: &mut AppState, key: KeyEvent) {
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
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => state.close_category_config(),
        KeyCode::Char('/') => state.start_config_filter(),
        KeyCode::Backspace if state.has_config_filter() => state.clear_config_filter(),
        KeyCode::Down | KeyCode::Char('j') => state.move_config_category_by(1, true),
        KeyCode::Up | KeyCode::Char('k') => state.move_config_category_by(-1, true),
        KeyCode::PageDown => state.move_config_category_by(10, false),
        KeyCode::PageUp => state.move_config_category_by(-10, false),
        KeyCode::Char(' ') => state.toggle_config_category(),
        KeyCode::Char('d') => state.reset_default_category_config(),
        _ => {}
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

fn find_category(categories: &[Category], requested: &str) -> Option<usize> {
    let requested = requested.trim().to_ascii_lowercase();

    categories.iter().position(|category| {
        category.name.to_ascii_lowercase() == requested
            || category.file.to_ascii_lowercase() == requested
            || category.file_stem().to_ascii_lowercase() == requested
    })
}

fn load_enabled_categories(categories: &[Category]) -> (Vec<bool>, Option<String>) {
    match Settings::load() {
        Ok(settings) => {
            let enabled_categories = enabled_categories_from_settings(categories, &settings)
                .unwrap_or_else(|| default_enabled_categories(categories));
            (enabled_categories, None)
        }
        Err(error) => (
            default_enabled_categories(categories),
            Some(error.to_string()),
        ),
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
            selected_category: 0,
            loaded_category: Some(0),
            articles: Vec::new(),
            selected_article: 0,
            focus: Focus::Categories,
            status: String::new(),
            error: None,
            category_filter: String::new(),
            category_filter_active: false,
            config_open: false,
            config_selected_category: 0,
            config_filter: String::new(),
            config_filter_active: false,
            detail_open: false,
            detail_scroll: 0,
            should_quit: false,
        }
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
    fn category_settings_uses_stable_category_keys() {
        let mut state = state_with_categories(vec![
            category("World", "world.json"),
            category("Today in History", "today_in_history.json"),
        ]);
        state.enabled_categories = vec![false, true];

        assert_eq!(
            state.category_settings(),
            Settings {
                categories: CategorySettings {
                    enabled: vec!["todayinhistory".to_owned()]
                }
            }
        );
    }
}
