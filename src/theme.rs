use std::{
    fs, io,
    path::{Path, PathBuf},
};

use ratatui::style::Color;
use serde::Deserialize;
use thiserror::Error;

use crate::settings;

pub const ANSI_THEME_ID: &str = "ansi";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub colors: ThemeColors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    pub text: Color,
    pub muted: Color,
    pub subtle: Color,
    pub title: Color,
    pub accent: Color,
    pub success: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub settings_selected_bg: Color,
    pub editing_bg: Color,
    pub link: Color,
    pub focus: Color,
    pub border: Color,
    pub status: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            text: Color::White,
            muted: Color::DarkGray,
            subtle: Color::Gray,
            title: Color::Cyan,
            accent: Color::Yellow,
            success: Color::Green,
            selected_fg: Color::Black,
            selected_bg: Color::Green,
            settings_selected_bg: Color::Cyan,
            editing_bg: Color::Yellow,
            link: Color::Cyan,
            focus: Color::Cyan,
            border: Color::DarkGray,
            status: Color::Magenta,
        }
    }
}

impl Theme {
    pub fn ansi() -> Self {
        Self {
            id: ANSI_THEME_ID.to_owned(),
            name: "ANSI".to_owned(),
            colors: ThemeColors::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCatalog {
    themes: Vec<Theme>,
}

impl ThemeCatalog {
    pub fn load() -> (Self, Vec<String>) {
        let mut themes = built_in_themes();
        let mut errors = Vec::new();

        match settings::themes_dir() {
            Ok(path) => load_custom_themes(&path, &mut themes, &mut errors),
            Err(error) => errors.push(error.to_string()),
        }

        (Self { themes }, errors)
    }

    pub fn built_in() -> Self {
        Self {
            themes: built_in_themes(),
        }
    }

    pub fn themes(&self) -> &[Theme] {
        &self.themes
    }

    pub fn selected_theme(&self, requested: &str) -> (Theme, usize, Option<String>) {
        let requested = normalize_theme_id(requested);
        let requested = if requested.is_empty() {
            ANSI_THEME_ID
        } else {
            requested.as_str()
        };

        if let Some(index) = self.index_of(requested) {
            return (self.themes[index].clone(), index, None);
        }

        let fallback_index = self.ansi_index();
        (
            self.themes[fallback_index].clone(),
            fallback_index,
            Some(format!("theme `{requested}` was not found; using ansi")),
        )
    }

    pub fn default_theme(&self) -> (Theme, usize) {
        let index = self.ansi_index();
        (self.themes[index].clone(), index)
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        let id = normalize_theme_id(id);
        self.themes.iter().position(|theme| theme.id == id)
    }

    fn ansi_index(&self) -> usize {
        self.index_of(ANSI_THEME_ID)
            .expect("built-in ANSI theme must be present")
    }
}

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("theme I/O failed at `{}`: {source}", display_path(path))]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse theme TOML at `{}`: {source}", display_path(path))]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("theme file `{}` does not have a valid file name", display_path(path))]
    InvalidThemeId { path: PathBuf },

    #[error(
        "theme `{}` has invalid color `{value}` for `{field}`",
        display_path(path)
    )]
    InvalidColor {
        path: PathBuf,
        field: &'static str,
        value: String,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ThemeFile {
    name: Option<String>,
    colors: ThemeColorOverrides,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ThemeColorOverrides {
    text: Option<String>,
    muted: Option<String>,
    subtle: Option<String>,
    title: Option<String>,
    accent: Option<String>,
    success: Option<String>,
    selected_fg: Option<String>,
    selected_bg: Option<String>,
    settings_selected_bg: Option<String>,
    editing_bg: Option<String>,
    link: Option<String>,
    focus: Option<String>,
    border: Option<String>,
    status: Option<String>,
}

fn built_in_themes() -> Vec<Theme> {
    vec![
        Theme::ansi(),
        theme(
            "catppuccin-mocha",
            "Catppuccin Mocha",
            ThemeColors {
                text: rgb(0xcdd6f4),
                muted: rgb(0x6c7086),
                subtle: rgb(0xa6adc8),
                title: rgb(0x89dceb),
                accent: rgb(0xf9e2af),
                success: rgb(0xa6e3a1),
                selected_fg: rgb(0x11111b),
                selected_bg: rgb(0xa6e3a1),
                settings_selected_bg: rgb(0x89dceb),
                editing_bg: rgb(0xf9e2af),
                link: rgb(0x89dceb),
                focus: rgb(0x89dceb),
                border: rgb(0x6c7086),
                status: rgb(0xcba6f7),
            },
        ),
        theme(
            "catppuccin-latte",
            "Catppuccin Latte",
            ThemeColors {
                text: rgb(0x4c4f69),
                muted: rgb(0x9ca0b0),
                subtle: rgb(0x6c6f85),
                title: rgb(0x04a5e5),
                accent: rgb(0xdf8e1d),
                success: rgb(0x40a02b),
                selected_fg: rgb(0xeff1f5),
                selected_bg: rgb(0x40a02b),
                settings_selected_bg: rgb(0x04a5e5),
                editing_bg: rgb(0xdf8e1d),
                link: rgb(0x1e66f5),
                focus: rgb(0x04a5e5),
                border: rgb(0x9ca0b0),
                status: rgb(0x8839ef),
            },
        ),
        theme(
            "dracula",
            "Dracula",
            ThemeColors {
                text: rgb(0xf8f8f2),
                muted: rgb(0x6272a4),
                subtle: rgb(0xbfbfbf),
                title: rgb(0x8be9fd),
                accent: rgb(0xf1fa8c),
                success: rgb(0x50fa7b),
                selected_fg: rgb(0x282a36),
                selected_bg: rgb(0x50fa7b),
                settings_selected_bg: rgb(0x8be9fd),
                editing_bg: rgb(0xf1fa8c),
                link: rgb(0x8be9fd),
                focus: rgb(0xff79c6),
                border: rgb(0x6272a4),
                status: rgb(0xbd93f9),
            },
        ),
        theme(
            "gruvbox-dark",
            "Gruvbox Dark",
            ThemeColors {
                text: rgb(0xebdbb2),
                muted: rgb(0x928374),
                subtle: rgb(0xa89984),
                title: rgb(0x83a598),
                accent: rgb(0xfabd2f),
                success: rgb(0xb8bb26),
                selected_fg: rgb(0x282828),
                selected_bg: rgb(0xb8bb26),
                settings_selected_bg: rgb(0x83a598),
                editing_bg: rgb(0xfabd2f),
                link: rgb(0x83a598),
                focus: rgb(0x83a598),
                border: rgb(0x665c54),
                status: rgb(0xd3869b),
            },
        ),
    ]
}

fn theme(id: &str, name: &str, colors: ThemeColors) -> Theme {
    Theme {
        id: id.to_owned(),
        name: name.to_owned(),
        colors,
    }
}

fn load_custom_themes(path: &Path, themes: &mut Vec<Theme>, errors: &mut Vec<String>) {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return,
        Err(source) => {
            errors.push(
                ThemeError::Io {
                    path: path.to_owned(),
                    source,
                }
                .to_string(),
            );
            return;
        }
    };

    let mut files = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) if is_toml_file(&entry.path()) => files.push(entry.path()),
            Ok(_) => {}
            Err(source) => errors.push(
                ThemeError::Io {
                    path: path.to_owned(),
                    source,
                }
                .to_string(),
            ),
        }
    }

    files.sort();
    for file in files {
        match custom_theme_from_path(&file) {
            Ok(theme) => upsert_theme(themes, theme),
            Err(error) => errors.push(error.to_string()),
        }
    }
}

fn custom_theme_from_path(path: &Path) -> Result<Theme, ThemeError> {
    let id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(normalize_theme_id)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| ThemeError::InvalidThemeId {
            path: path.to_owned(),
        })?;
    let contents = fs::read_to_string(path).map_err(|source| ThemeError::Io {
        path: path.to_owned(),
        source,
    })?;
    let theme_file =
        toml::from_str::<ThemeFile>(&contents).map_err(|source| ThemeError::Parse {
            path: path.to_owned(),
            source,
        })?;
    theme_from_file(id, path, theme_file)
}

fn theme_from_file(id: String, path: &Path, theme_file: ThemeFile) -> Result<Theme, ThemeError> {
    let mut colors = ThemeColors::default();
    let overrides = theme_file.colors;

    apply_color(&mut colors.text, overrides.text, path, "text")?;
    apply_color(&mut colors.muted, overrides.muted, path, "muted")?;
    apply_color(&mut colors.subtle, overrides.subtle, path, "subtle")?;
    apply_color(&mut colors.title, overrides.title, path, "title")?;
    apply_color(&mut colors.accent, overrides.accent, path, "accent")?;
    apply_color(&mut colors.success, overrides.success, path, "success")?;
    apply_color(
        &mut colors.selected_fg,
        overrides.selected_fg,
        path,
        "selected_fg",
    )?;
    apply_color(
        &mut colors.selected_bg,
        overrides.selected_bg,
        path,
        "selected_bg",
    )?;
    apply_color(
        &mut colors.settings_selected_bg,
        overrides.settings_selected_bg,
        path,
        "settings_selected_bg",
    )?;
    apply_color(
        &mut colors.editing_bg,
        overrides.editing_bg,
        path,
        "editing_bg",
    )?;
    apply_color(&mut colors.link, overrides.link, path, "link")?;
    apply_color(&mut colors.focus, overrides.focus, path, "focus")?;
    apply_color(&mut colors.border, overrides.border, path, "border")?;
    apply_color(&mut colors.status, overrides.status, path, "status")?;

    Ok(Theme {
        name: theme_file.name.unwrap_or_else(|| display_name_from_id(&id)),
        id,
        colors,
    })
}

fn apply_color(
    target: &mut Color,
    value: Option<String>,
    path: &Path,
    field: &'static str,
) -> Result<(), ThemeError> {
    let Some(value) = value else {
        return Ok(());
    };

    *target = parse_color(&value).map_err(|()| ThemeError::InvalidColor {
        path: path.to_owned(),
        field,
        value,
    })?;
    Ok(())
}

fn upsert_theme(themes: &mut Vec<Theme>, theme: Theme) {
    if let Some(existing) = themes.iter_mut().find(|existing| existing.id == theme.id) {
        *existing = theme;
    } else {
        themes.push(theme);
    }
}

fn is_toml_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
}

pub fn normalize_theme_id(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn display_name_from_id(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_color(value: &str) -> Result<Color, ()> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    if trimmed.len() == 6 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return parse_hex_color(trimmed);
    }
    if let Some(index) = trimmed.strip_prefix("indexed:") {
        return index.parse::<u8>().map(Color::Indexed).map_err(|_| ());
    }

    match trimmed
        .to_ascii_lowercase()
        .replace(['_', ' '], "-")
        .as_str()
    {
        "reset" => Ok(Color::Reset),
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "darkgray" | "darkgrey" | "dark-gray" | "dark-grey" => Ok(Color::DarkGray),
        "lightred" | "light-red" => Ok(Color::LightRed),
        "lightgreen" | "light-green" => Ok(Color::LightGreen),
        "lightyellow" | "light-yellow" => Ok(Color::LightYellow),
        "lightblue" | "light-blue" => Ok(Color::LightBlue),
        "lightmagenta" | "light-magenta" => Ok(Color::LightMagenta),
        "lightcyan" | "light-cyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        _ => Err(()),
    }
}

fn parse_hex_color(hex: &str) -> Result<Color, ()> {
    if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(());
    }

    let red = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ())?;
    let green = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ())?;
    let blue = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ())?;
    Ok(Color::Rgb(red, green, blue))
}

fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_custom_theme(contents: &str) -> Theme {
        let theme_file = toml::from_str::<ThemeFile>(contents).unwrap();
        theme_from_file(
            "my-theme".to_owned(),
            Path::new("my-theme.toml"),
            theme_file,
        )
        .unwrap()
    }

    #[test]
    fn ansi_theme_matches_previous_default_colors() {
        assert_eq!(
            Theme::ansi().colors,
            ThemeColors {
                text: Color::White,
                muted: Color::DarkGray,
                subtle: Color::Gray,
                title: Color::Cyan,
                accent: Color::Yellow,
                success: Color::Green,
                selected_fg: Color::Black,
                selected_bg: Color::Green,
                settings_selected_bg: Color::Cyan,
                editing_bg: Color::Yellow,
                link: Color::Cyan,
                focus: Color::Cyan,
                border: Color::DarkGray,
                status: Color::Magenta,
            }
        );
    }

    #[test]
    fn parses_hex_and_named_theme_colors() {
        let theme = parse_custom_theme(
            r##"
            name = "My Theme"

            [colors]
            text = "#cdd6f4"
            muted = "darkgray"
            focus = "light-cyan"
            selected_bg = "indexed:42"
            "##,
        );

        assert_eq!(theme.name, "My Theme");
        assert_eq!(theme.colors.text, Color::Rgb(0xcd, 0xd6, 0xf4));
        assert_eq!(theme.colors.muted, Color::DarkGray);
        assert_eq!(theme.colors.focus, Color::LightCyan);
        assert_eq!(theme.colors.selected_bg, Color::Indexed(42));
        assert_eq!(theme.colors.accent, Color::Yellow);
    }

    #[test]
    fn invalid_theme_color_reports_field() {
        let theme_file = toml::from_str::<ThemeFile>(
            r#"
            [colors]
            accent = "not-a-color"
            "#,
        )
        .unwrap();
        let error =
            theme_from_file("bad".to_owned(), Path::new("bad.toml"), theme_file).unwrap_err();

        assert_eq!(
            error.to_string(),
            "theme `bad.toml` has invalid color `not-a-color` for `accent`"
        );
    }

    #[test]
    fn catalog_falls_back_to_ansi_for_unknown_theme() {
        let catalog = ThemeCatalog::built_in();
        let (theme, index, error) = catalog.selected_theme("missing");

        assert_eq!(theme.id, ANSI_THEME_ID);
        assert_eq!(index, catalog.index_of(ANSI_THEME_ID).unwrap());
        assert_eq!(
            error.as_deref(),
            Some("theme `missing` was not found; using ansi")
        );
    }

    #[test]
    fn custom_theme_names_fall_back_to_file_id() {
        let theme = parse_custom_theme(
            r#"
            [colors]
            text = "white"
            "#,
        );

        assert_eq!(theme.name, "My Theme");
    }
}
