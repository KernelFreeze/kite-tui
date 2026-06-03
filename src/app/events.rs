//! The terminal lifecycle (`run`), the event loop, and all keyboard input
//! handling. These translate raw key events into [`AppState`] transitions.

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::keybindings::{
    KEY_SHIFT_TAB, KEY_TAB, KeyBindingAction, key_sequence_label, key_sequence_part,
};
use crate::app::state::{AppState, Focus, SettingsSection};
use crate::args::Args;
use crate::error::{KiteError, Result};
use crate::kagi::KagiClient;
use crate::ui;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const THEME_MODE_POLL_INTERVAL: Duration = Duration::from_secs(3);

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
    let mut next_theme_mode_check = Instant::now() + THEME_MODE_POLL_INTERVAL;

    while !state.should_quit {
        if Instant::now() >= next_theme_mode_check {
            state.refresh_platform_color_scheme();
            next_theme_mode_check = Instant::now() + THEME_MODE_POLL_INTERVAL;
        }

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
            && state.keybinds.matches(KeyBindingAction::Help, key)
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

    if state.keybinds.matches(KeyBindingAction::Help, key) {
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
        if state.keybinds.matches(KeyBindingAction::Quit, key) {
            state.quit();
        }
        return;
    }

    if state.focus == Focus::Articles && handle_article_sequence_key(state, key) {
        return;
    }

    if state.keybinds.matches(KeyBindingAction::NextCategory, key) {
        load_next_category(state, client).await;
        return;
    }

    if state
        .keybinds
        .matches(KeyBindingAction::PreviousCategory, key)
    {
        load_previous_category(state, client).await;
        return;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Esc if state.has_category_filter() => state.clear_category_filter(),
        KeyCode::Esc => state.quit(),
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

    if state.keybinds.matches(KeyBindingAction::Settings, key) {
        state.open_settings();
    } else if state
        .keybinds
        .matches(KeyBindingAction::CategoryFilter, key)
    {
        state.start_category_filter();
    } else if state.keybinds.matches(KeyBindingAction::Quit, key) {
        state.quit();
    } else if state.keybinds.matches(KeyBindingAction::Refresh, key) {
        if state.selected_category_matches_filter() {
            state.load_selected_category(client).await;
        } else {
            state.update_category_filter_status();
        }
    } else if state.keybinds.matches(KeyBindingAction::RefreshAll, key) {
        state.refresh_all(client).await;
    }
}

async fn load_previous_category(state: &mut AppState, client: &KagiClient) {
    if state.select_previous_category() {
        state.load_selected_category(client).await;
    }
}

async fn load_next_category(state: &mut AppState, client: &KagiClient) {
    if state.select_next_category() {
        state.load_selected_category(client).await;
    }
}

fn handle_settings_key(state: &mut AppState, key: KeyEvent) {
    if let Some(action) = state.editing_keybind {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
            KeyCode::Esc => state.cancel_keybind_edit(),
            KeyCode::Enter => state.finish_keybind_edit(action),
            KeyCode::Backspace => state.pop_keybind_input(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                state.set_keybind_input(KEY_SHIFT_TAB);
            }
            KeyCode::Tab => state.set_keybind_input(KEY_TAB),
            KeyCode::BackTab => state.set_keybind_input(KEY_SHIFT_TAB),
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

    if state.theme_dropdown_open {
        handle_theme_mode_dropdown_key(state, key);
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
            SettingsSection::Themes => handle_theme_settings_key(state, key),
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

    if state.keybinds.matches(KeyBindingAction::Quit, key) {
        state.close_settings();
    } else if state
        .keybinds
        .matches(KeyBindingAction::CategoryFilter, key)
    {
        state.start_config_filter();
    } else if state.keybinds.matches(KeyBindingAction::ResetDefaults, key) {
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

    if state.keybinds.matches(KeyBindingAction::Quit, key) {
        state.close_settings();
    } else if state.keybinds.matches(KeyBindingAction::ResetDefaults, key) {
        state.reset_default_keybinds();
    }
}

fn handle_theme_settings_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => state.toggle_theme_mode_dropdown(),
        KeyCode::Char(' ') => state.select_theme(),
        KeyCode::Down | KeyCode::Char('j') => state.move_theme_by(1, true),
        KeyCode::Up | KeyCode::Char('k') => state.move_theme_by(-1, true),
        KeyCode::PageDown => state.move_theme_by(4, false),
        KeyCode::PageUp => state.move_theme_by(-4, false),
        KeyCode::Char('m') => state.toggle_theme_mode_dropdown(),
        _ => {}
    }

    if state.keybinds.matches(KeyBindingAction::Quit, key) {
        state.close_settings();
    } else if state.keybinds.matches(KeyBindingAction::ResetDefaults, key) {
        state.reset_default_theme();
    }
}

fn handle_theme_mode_dropdown_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => state.close_theme_mode_dropdown(),
        KeyCode::Down | KeyCode::Char('j') => state.move_theme_mode_by(1),
        KeyCode::Up | KeyCode::Char('k') => state.move_theme_mode_by(-1),
        _ => {}
    }
}

fn handle_help_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => state.quit(),
        KeyCode::Esc | KeyCode::Enter => state.close_help(),
        _ => {
            if state.keybinds.matches(KeyBindingAction::Help, key)
                || state.keybinds.matches(KeyBindingAction::Quit, key)
            {
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
    use crate::app::test_support::{article, categories, key, state_with_categories};

    #[test]
    fn keybind_editor_accepts_named_tab_keys() {
        let mut state = state_with_categories(categories());
        state.editing_keybind = Some(KeyBindingAction::NextCategory);

        handle_settings_key(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(state.keybind_input, KEY_TAB);
        assert_eq!(state.status, "Editing Next category: Tab");

        handle_settings_key(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(state.keybind_input, KEY_SHIFT_TAB);
        assert_eq!(state.status, "Editing Next category: Shift+Tab");
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
