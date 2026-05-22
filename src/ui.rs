use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use time::{OffsetDateTime, macros::format_description};

use crate::{
    app::{AppState, Focus},
    models::Article,
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
        .constraints([Constraint::Length(8), Constraint::Min(8)])
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
    let items = if visible_categories.is_empty() {
        let message = if app.has_category_filter() {
            "No matching categories."
        } else {
            "No categories loaded."
        };
        vec![ListItem::new(message)]
    } else {
        visible_categories
            .into_iter()
            .filter_map(|index| {
                app.categories.get(index).map(|category| {
                    let marker = if index == app.selected_category {
                        ">"
                    } else if Some(index) == app.loaded_category {
                        "*"
                    } else {
                        " "
                    };
                    let style = if index == app.selected_category {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else {
                        Style::default()
                    };

                    ListItem::new(Line::from(vec![
                        Span::raw(marker),
                        Span::raw(" "),
                        Span::raw(category.name.clone()),
                    ]))
                    .style(style)
                })
            })
            .collect::<Vec<_>>()
    };

    let title = if app.category_filter_active || app.has_category_filter() {
        format!("Categories /{}", app.category_filter)
    } else {
        "Categories".to_owned()
    };
    let block = focused_block(
        title,
        app.focus == Focus::Categories || app.category_filter_active,
    );
    frame.render_widget(List::new(items).block(block), area);
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
    frame.render_widget(List::new(items).block(block), area);
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

fn render_status(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let focus = if app.detail_open {
        "Article"
    } else {
        match app.focus {
            Focus::Categories => "Categories",
            Focus::Articles => "Articles",
        }
    };
    let help = if app.detail_open {
        "Esc/Enter returns to list, j/k scroll, q quit"
    } else if app.category_filter_active {
        "Type to filter, Enter/Esc accept, Backspace edits"
    } else {
        match app.focus {
            Focus::Categories => "/ filters categories, Enter loads category, Tab switches panes",
            Focus::Articles => "/ filters categories, Enter opens article, Tab switches panes",
        }
    };
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

    lines.extend(
        article
            .summary
            .lines()
            .map(|line| Line::from(line.to_owned())),
    );

    if let Some(link) = &article.link {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            link.as_str().to_owned(),
            Style::default().fg(Color::Cyan),
        )));
    }

    lines
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
