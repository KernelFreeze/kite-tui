use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use time::{OffsetDateTime, macros::format_description};

use crate::{
    app::{AppState, Focus},
    models::{Article, SummaryBlock},
};

pub fn draw(frame: &mut Frame<'_>, app: &AppState) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, app, vertical[0]);
    render_body(frame, app, vertical[1]);
    render_status(frame, app, vertical[2]);

    if app.config_open {
        render_category_config_popup(frame, app, area);
    }
    if app.help_open {
        render_help_popup(frame, app, area);
    }
}

fn render_header(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let category = app
        .loaded_category()
        .map(|category| category.name.as_str())
        .or_else(|| {
            app.selected_category()
                .map(|category| category.name.as_str())
        })
        .unwrap_or("Kagi News");
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Kite",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(category.to_owned(), Style::default().fg(Color::White)),
    ]));

    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(8)])
        .split(area);

    render_categories(frame, app, rows[0]);
    if app.detail_open {
        render_article_detail(frame, app, rows[1]);
    } else {
        render_articles(frame, app, rows[1]);
    }
}

fn render_categories(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let visible_categories = app.filtered_category_indices();
    if visible_categories.is_empty() {
        let message = if app.has_category_filter() {
            "No matching categories."
        } else {
            "No categories loaded."
        };
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    frame.render_widget(
        Paragraph::new(category_tabs(app, &visible_categories, area.width)),
        area,
    );
}

fn category_tabs(app: &AppState, visible_categories: &[usize], width: u16) -> Text<'static> {
    let window = category_tab_window(app, visible_categories, width);
    let mut labels = Vec::new();
    let mut underlines = Vec::new();

    for (position, index) in visible_categories[window].iter().copied().enumerate() {
        let Some(category) = app.categories.get(index) else {
            continue;
        };

        if position > 0 {
            labels.push(Span::raw("   "));
            underlines.push(Span::raw("   "));
        }

        let name_width = category.name.chars().count();
        labels.push(Span::styled(
            category.name.clone(),
            category_tab_style(app, index),
        ));

        if index == app.selected_category {
            underlines.push(Span::styled(
                "-".repeat(name_width),
                Style::default().fg(Color::Yellow),
            ));
        } else {
            underlines.push(Span::raw(" ".repeat(name_width)));
        }
    }

    Text::from(vec![Line::from(labels), Line::from(underlines)])
}

fn category_tab_window(
    app: &AppState,
    visible_categories: &[usize],
    width: u16,
) -> std::ops::Range<usize> {
    let selected_position = visible_categories
        .iter()
        .position(|index| *index == app.selected_category)
        .unwrap_or(0);
    let max_width = width as usize;
    let mut start = selected_position;
    let mut end = selected_position + 1;

    loop {
        let mut expanded = false;
        if end < visible_categories.len()
            && category_tabs_width(app, visible_categories, start..end + 1) <= max_width
        {
            end += 1;
            expanded = true;
        }
        if start > 0 && category_tabs_width(app, visible_categories, start - 1..end) <= max_width {
            start -= 1;
            expanded = true;
        }
        if !expanded {
            break;
        }
    }

    start..end
}

fn category_tabs_width(
    app: &AppState,
    visible_categories: &[usize],
    window: std::ops::Range<usize>,
) -> usize {
    let tab_count = window.end.saturating_sub(window.start);
    let gap_width = tab_count.saturating_sub(1) * 3;
    let labels_width = visible_categories[window]
        .iter()
        .filter_map(|index| app.categories.get(*index))
        .map(|category| category.name.chars().count())
        .sum::<usize>();

    labels_width + gap_width
}

fn category_tab_style(app: &AppState, index: usize) -> Style {
    if index == app.selected_category {
        let color = if app.focus == Focus::Categories || app.category_filter_active {
            Color::White
        } else {
            Color::Gray
        };
        return Style::default().fg(color).add_modifier(Modifier::BOLD);
    }

    if Some(index) == app.loaded_category {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn render_articles(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let items = if app.articles.is_empty() {
        vec![ListItem::new("No articles loaded.")]
    } else {
        app.articles
            .iter()
            .enumerate()
            .map(|(index, article)| {
                let style = if index == app.selected_article {
                    Style::default().fg(Color::Black).bg(Color::Green)
                } else {
                    Style::default()
                };
                let date = article
                    .published_at
                    .map(format_short_date)
                    .unwrap_or_else(|| "unknown".to_owned());

                ListItem::new(Line::from(vec![
                    Span::raw(if index == app.selected_article {
                        ">"
                    } else {
                        " "
                    }),
                    Span::raw(" "),
                    Span::styled(date, Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    Span::raw(article.title.clone()),
                ]))
                .style(style)
            })
            .collect::<Vec<_>>()
    };

    let block = focused_block("Articles", app.focus == Focus::Articles);
    let mut list_state = ListState::default();
    list_state.select((!app.articles.is_empty()).then_some(app.selected_article));
    frame.render_stateful_widget(List::new(items).block(block), area, &mut list_state);
}

fn render_article_detail(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let lines = app
        .selected_article()
        .map(article_detail)
        .unwrap_or_else(|| vec![Line::from("No article selected.")]);

    let paragraph = Paragraph::new(Text::from(lines))
        .block(focused_block("Article", true))
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    frame.render_widget(paragraph, area);
}

fn render_category_config_popup(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let popup = centered_rect(86, 86, area);
    frame.render_widget(Clear, popup);

    let block = focused_block("Category Settings", true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(5),
        ])
        .split(inner);

    render_enabled_category_summary(frame, app, chunks[0]);
    render_config_filter(frame, app, chunks[1]);
    render_config_category_list(frame, app, chunks[2]);
}

fn render_enabled_category_summary(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let mut spans = vec![Span::styled(
        "Shown ",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )];

    for index in app.enabled_category_indices() {
        if let Some(category) = app.categories.get(index) {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!(" {} ", category.name),
                Style::default().fg(Color::Black).bg(Color::Cyan),
            ));
        }
    }

    if app.enabled_category_count() == 0 {
        spans.push(Span::styled(" none", Style::default().fg(Color::DarkGray)));
    }

    let paragraph = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::BOTTOM))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_config_filter(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let style = if app.config_filter_active {
        Style::default().fg(Color::Cyan)
    } else if app.has_config_filter() {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let value = if app.has_config_filter() {
        app.config_filter.clone()
    } else {
        format!("press {} to search", app.keybinds.category_filter_label())
    };

    let paragraph = Paragraph::new(Line::from(vec![
        Span::styled("Search ", Style::default().fg(Color::White)),
        Span::styled(value, style),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(paragraph, area);
}

fn render_config_category_list(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let indices = app.filtered_config_category_indices();
    let items = if indices.is_empty() {
        vec![ListItem::new("No matching categories.")]
    } else {
        indices
            .iter()
            .filter_map(|index| {
                app.categories.get(*index).map(|category| {
                    let selected = *index == app.config_selected_category;
                    let enabled = app.is_category_enabled(*index);
                    let style = if selected {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else if enabled {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default()
                    };
                    let marker = if selected { ">" } else { " " };
                    let checkbox = if enabled { "[x]" } else { "[ ]" };

                    ListItem::new(Line::from(vec![
                        Span::raw(marker),
                        Span::raw(" "),
                        Span::raw(checkbox),
                        Span::raw(" "),
                        Span::raw(category.name.clone()),
                    ]))
                    .style(style)
                })
            })
            .collect()
    };

    let title = format!(
        "Available Categories ({}/{})",
        app.enabled_category_count(),
        app.categories.len()
    );
    let mut list_state = ListState::default();
    list_state.select(
        indices
            .iter()
            .position(|index| *index == app.config_selected_category),
    );

    frame.render_stateful_widget(
        List::new(items).block(focused_block(title, true)),
        area,
        &mut list_state,
    );
}

fn render_help_popup(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let popup = centered_rect(64, 72, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        help_line(app.keybinds.help_label(), "Open help"),
        help_line(app.keybinds.config_label(), "Configure categories"),
        help_line(app.keybinds.category_filter_label(), "Filter categories"),
        help_line(app.keybinds.refresh_label(), "Refresh category"),
        help_line(app.keybinds.quit_label(), "Quit or close popup"),
        help_line(
            app.keybinds.reset_defaults_label(),
            "Restore default categories in settings",
        ),
        Line::from(""),
        help_line("Tab", "Switch panes"),
        help_line("Enter", "Load category or open article"),
        help_line("Esc", "Close popup or clear filter"),
        help_line("j/k, arrows", "Move selection"),
        help_line("PageUp/PageDown", "Move by page"),
        help_line("Space", "Toggle category in settings"),
    ];

    let paragraph = Paragraph::new(Text::from(lines))
        .block(focused_block("Help", true))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup);
}

fn help_line(key: impl Into<String>, description: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<16}", key.into()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(description.into()),
    ])
}

fn render_status(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let focus = if app.help_open {
        "Help"
    } else if app.config_open {
        "Config"
    } else if app.detail_open {
        "Article"
    } else {
        match app.focus {
            Focus::Categories => "Categories",
            Focus::Articles => "Articles",
        }
    };
    let help = format!("{} help", app.keybinds.help_label());
    let status = app.error.as_deref().unwrap_or(&app.status);
    let paragraph = Paragraph::new(Line::from(vec![
        Span::styled(focus, Style::default().fg(Color::Magenta)),
        Span::raw(" | "),
        Span::raw(status.to_owned()),
        Span::raw(" | "),
        Span::styled(help, Style::default().fg(Color::DarkGray)),
    ]));

    frame.render_widget(paragraph, area);
}

fn article_detail(article: &Article) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            article.title.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{}  {}",
            article
                .published_at
                .map(format_long_date)
                .unwrap_or_else(|| "unknown date".to_owned()),
            article.categories.join(", ")
        )),
        Line::from(""),
    ];

    if article.summary_blocks.is_empty() {
        lines.extend(
            article
                .summary
                .lines()
                .map(|line| Line::from(line.to_owned())),
        );
    } else {
        append_summary_blocks(&mut lines, &article.summary_blocks);
    }

    if let Some(link) = &article.link {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            link.as_str().to_owned(),
            Style::default().fg(Color::Cyan),
        )));
    }

    lines
}

fn append_summary_blocks(lines: &mut Vec<Line<'static>>, blocks: &[SummaryBlock]) {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }

        match block {
            SummaryBlock::Heading { level, text } => {
                let color = if *level <= 2 {
                    Color::Cyan
                } else {
                    Color::Yellow
                };
                lines.push(Line::from(Span::styled(
                    text.clone(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )));
            }
            SummaryBlock::Paragraph(text) => {
                lines.push(Line::from(text.clone()));
            }
            SummaryBlock::List { ordered, items } => {
                for (index, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        format!("{}. ", index + 1)
                    } else {
                        "- ".to_owned()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(marker, Style::default().fg(Color::Yellow)),
                        Span::raw(item.clone()),
                    ]));
                }
            }
            SummaryBlock::Quote(text) => {
                for line in text.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("> ", Style::default().fg(Color::DarkGray)),
                        Span::styled(line.to_owned(), Style::default().fg(Color::Gray)),
                    ]));
                }
            }
        }
    }
}

fn focused_block(title: impl Into<String>, focused: bool) -> Block<'static> {
    let style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(style)
        .title(title.into())
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical_margin = (100 - percent_y) / 2;
    let horizontal_margin = (100 - percent_x) / 2;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(vertical_margin),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(vertical_margin),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(horizontal_margin),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(horizontal_margin),
        ])
        .split(popup_layout[1])[1]
}

fn format_short_date(date: OffsetDateTime) -> String {
    let format = format_description!("[month repr:short] [day padding:none]");
    date.format(format).unwrap_or_else(|_| "unknown".to_owned())
}

fn format_long_date(date: OffsetDateTime) -> String {
    let format = format_description!(
        "[weekday repr:short], [month repr:short] [day padding:none], [year] [hour]:[minute] UTC"
    );
    date.format(format)
        .unwrap_or_else(|_| "unknown date".to_owned())
}
