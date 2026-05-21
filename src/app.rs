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
    ui,
};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(200);

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
    pub selected_category: usize,
    pub loaded_category: Option<usize>,
    pub articles: Vec<Article>,
    pub selected_article: usize,
    pub focus: Focus,
    pub status: String,
    pub error: Option<String>,
    pub detail_open: bool,
    pub detail_scroll: u16,
    should_quit: bool,
}

impl AppState {
    pub async fn bootstrap(client: &KagiClient, initial_category: &str) -> Result<Self> {
        let categories = client.categories().await?;
        let selected_category = find_category(&categories, initial_category)
            .ok_or_else(|| KiteError::CategoryNotFound(initial_category.to_owned()))?;

        let mut state = Self {
            categories,
            selected_category,
            loaded_category: None,
            articles: Vec::new(),
            selected_article: 0,
            focus: Focus::Articles,
            status: "Loading articles".to_owned(),
            error: None,
            detail_open: false,
            detail_scroll: 0,
            should_quit: false,
        };

        state.load_selected_category(client).await;
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

    fn move_next(&mut self) {
        if self.detail_open {
            self.detail_scroll = self.detail_scroll.saturating_add(1);
            return;
        }

        match self.focus {
            Focus::Categories => {
                if !self.categories.is_empty() {
                    self.selected_category = (self.selected_category + 1) % self.categories.len();
                    self.status = self
                        .selected_category()
                        .map(|category| format!("Selected {}", category.name))
                        .unwrap_or_default();
                }
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
                if !self.categories.is_empty() {
                    self.selected_category = self
                        .selected_category
                        .checked_sub(1)
                        .unwrap_or(self.categories.len() - 1);
                    self.status = self
                        .selected_category()
                        .map(|category| format!("Selected {}", category.name))
                        .unwrap_or_default();
                }
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
                if !self.categories.is_empty() {
                    self.selected_category =
                        (self.selected_category + 10).min(self.categories.len() - 1);
                }
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
                self.selected_category = self.selected_category.saturating_sub(10);
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
    let mut state = AppState::bootstrap(&client, &args.category).await?;

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
        KeyCode::Char('q') | KeyCode::Esc => state.quit(),
        KeyCode::Tab => state.next_focus(),
        KeyCode::Down | KeyCode::Char('j') => state.move_next(),
        KeyCode::Up | KeyCode::Char('k') => state.move_previous(),
        KeyCode::PageDown => state.page_next(),
        KeyCode::PageUp => state.page_previous(),
        KeyCode::Enter => match state.focus {
            Focus::Categories => state.load_selected_category(client).await,
            Focus::Articles => state.open_detail(),
        },
        KeyCode::Char('r') => state.load_selected_category(client).await,
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
        let mut state = AppState {
            categories: categories(),
            selected_category: 0,
            loaded_category: Some(0),
            articles: Vec::new(),
            selected_article: 0,
            focus: Focus::Categories,
            status: String::new(),
            error: None,
            detail_open: false,
            detail_scroll: 0,
            should_quit: false,
        };

        state.next_focus();
        assert_eq!(state.focus, Focus::Articles);

        state.next_focus();
        assert_eq!(state.focus, Focus::Categories);
    }
}
