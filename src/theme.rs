use std::{
    fs, io,
    path::{Path, PathBuf},
};

use ratatui::style::Color;
use serde::Deserialize;
use thiserror::Error;

use crate::settings;

pub const ANSI_THEME_ID: &str = "ansi";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformColorScheme {
    Light,
    Dark,
    Unspecified,
}

impl PlatformColorScheme {
    pub fn title(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Unspecified => "Unspecified",
        }
    }
}

impl From<dark_light::Mode> for PlatformColorScheme {
    fn from(mode: dark_light::Mode) -> Self {
        match mode {
            dark_light::Mode::Light => Self::Light,
            dark_light::Mode::Dark => Self::Dark,
            dark_light::Mode::Unspecified => Self::Unspecified,
        }
    }
}

pub fn detect_platform_color_scheme() -> Result<PlatformColorScheme, dark_light::Error> {
    dark_light::detect().map(PlatformColorScheme::from)
}

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
        terminal_theme(
            "catppuccin-frappe",
            "Catppuccin Frappe",
            0xc6d0f5,
            0x626880,
            0xa5adce,
            0xa6d189,
            0xe5c890,
            0x8caaee,
            0xf4b8e4,
            0x81c8be,
            0xc6d0f5,
            0x626880,
        ),
        terminal_theme(
            "catppuccin-macchiato",
            "Catppuccin Macchiato",
            0xcad3f5,
            0x5b6078,
            0xa5adcb,
            0xa6da95,
            0xeed49f,
            0x8aadf4,
            0xf5bde6,
            0x8bd5ca,
            0xcad3f5,
            0x5b6078,
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
        terminal_theme(
            "gruvbox-light",
            "Gruvbox Light",
            0x3c3836,
            0x928374,
            0x7c6f64,
            0x98971a,
            0xd79921,
            0x458588,
            0xb16286,
            0x689d6a,
            0xfbf1c7,
            0x3c3836,
        ),
        terminal_theme(
            "tokyo-night",
            "Tokyo Night",
            0xc0caf5,
            0x414868,
            0xa9b1d6,
            0x9ece6a,
            0xe0af68,
            0x7aa2f7,
            0xbb9af7,
            0x7dcfff,
            0xc0caf5,
            0x33467c,
        ),
        terminal_theme(
            "tokyo-night-storm",
            "Tokyo Night Storm",
            0xc0caf5,
            0x4e5575,
            0xa9b1d6,
            0x9ece6a,
            0xe0af68,
            0x7aa2f7,
            0xbb9af7,
            0x7dcfff,
            0xc0caf5,
            0x364a82,
        ),
        terminal_theme(
            "tokyo-night-moon",
            "Tokyo Night Moon",
            0xc8d3f5,
            0x444a73,
            0x828bb8,
            0xc3e88d,
            0xffc777,
            0x82aaff,
            0xc099ff,
            0x86e1fc,
            0xc8d3f5,
            0x2d3f76,
        ),
        terminal_theme(
            "rose-pine",
            "Rose Pine",
            0xe0def4,
            0x6e6a86,
            0xe0def4,
            0x31748f,
            0xf6c177,
            0x9ccfd8,
            0xc4a7e7,
            0xebbcba,
            0xe0def4,
            0x403d52,
        ),
        terminal_theme(
            "rose-pine-moon",
            "Rose Pine Moon",
            0xe0def4,
            0x6e6a86,
            0xe0def4,
            0x3e8fb0,
            0xf6c177,
            0x9ccfd8,
            0xc4a7e7,
            0xea9a97,
            0xe0def4,
            0x44415a,
        ),
        terminal_theme(
            "rose-pine-dawn",
            "Rose Pine Dawn",
            0x575279,
            0x9893a5,
            0x575279,
            0x286983,
            0xea9d34,
            0x56949f,
            0x907aa9,
            0xd7827e,
            0x575279,
            0xdfdad9,
        ),
        terminal_theme(
            "nord", "Nord", 0xd8dee9, 0x596377, 0xe5e9f0, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead,
            0x88c0d0, 0x4c566a, 0xeceff4,
        ),
        terminal_theme(
            "solarized-dark",
            "Solarized Dark",
            0x839496,
            0x335e69,
            0xeee8d5,
            0x859900,
            0xb58900,
            0x268bd2,
            0xd33682,
            0x2aa198,
            0x93a1a1,
            0x073642,
        ),
        terminal_theme(
            "solarized-light",
            "Solarized Light",
            0x657b83,
            0x839496,
            0x586e75,
            0x859900,
            0xb58900,
            0x268bd2,
            0xd33682,
            0x2aa198,
            0x586e75,
            0xeee8d5,
        ),
        terminal_theme(
            "everforest-dark-hard",
            "Everforest Dark Hard",
            0xd3c6aa,
            0xa6b0a0,
            0xf2efdf,
            0xa7c080,
            0xdbbc7f,
            0x7fbbb3,
            0xd699b6,
            0x83c092,
            0xd3c6aa,
            0x4c3743,
        ),
        terminal_theme(
            "everforest-light-medium",
            "Everforest Light Medium",
            0x5c6a72,
            0xa6b0a0,
            0xb2af9f,
            0x9ab373,
            0xc1a266,
            0x7fbbb3,
            0xd699b6,
            0x83c092,
            0x5c6a72,
            0xeaedc8,
        ),
        terminal_theme(
            "kanagawa-wave",
            "Kanagawa Wave",
            0xdcd7ba,
            0x727169,
            0xc8c093,
            0x76946a,
            0xc0a36e,
            0x7e9cd8,
            0x957fb8,
            0x6a9589,
            0x1f1f28,
            0xdcd7ba,
        ),
        terminal_theme(
            "kanagawa-dragon",
            "Kanagawa Dragon",
            0xc5c9c5,
            0xa6a69c,
            0xc8c093,
            0x8a9a7b,
            0xc4b28a,
            0x8ba4b0,
            0xa292a3,
            0x8ea4a2,
            0x181616,
            0xc5c9c5,
        ),
        terminal_theme(
            "atom-one-dark",
            "Atom One Dark",
            0xabb2bf,
            0x767676,
            0xabb2bf,
            0x98c379,
            0xe5c07b,
            0x61afef,
            0xc678dd,
            0x56b6c2,
            0xabb2bf,
            0x323844,
        ),
        terminal_theme(
            "atom-one-light",
            "Atom One Light",
            0x2a2c33,
            0x000000,
            0xbbbbbb,
            0x3f953a,
            0xd2b67c,
            0x2f5af3,
            0x950095,
            0x3f953a,
            0x2a2c33,
            0xededed,
        ),
        terminal_theme(
            "monokai-pro",
            "Monokai Pro",
            0xfcfcfa,
            0x727072,
            0xfcfcfa,
            0xa9dc76,
            0xffd866,
            0xfc9867,
            0xab9df2,
            0x78dce8,
            0xfcfcfa,
            0x5b595c,
        ),
        terminal_theme(
            "monokai-remastered",
            "Monokai Remastered",
            0xd9d9d9,
            0x625e4c,
            0xc4c5b5,
            0x98e024,
            0xfd971f,
            0x9d65ff,
            0xf4005f,
            0x58d1eb,
            0xffffff,
            0x343434,
        ),
        terminal_theme(
            "ayu-dark", "Ayu Dark", 0xbfbdb6, 0x686868, 0xc7c7c7, 0x7fd962, 0xf9af4f, 0x53bdfa,
            0xcda1fa, 0x90e1c6, 0x0b0e14, 0x409fff,
        ),
        terminal_theme(
            "ayu-mirage",
            "Ayu Mirage",
            0xcccac2,
            0x686868,
            0xc7c7c7,
            0x87d96c,
            0xfacc6e,
            0x6dcbfa,
            0xdabafa,
            0x90e1c6,
            0x1f2430,
            0x409fff,
        ),
        terminal_theme(
            "ayu-light",
            "Ayu Light",
            0x5c6166,
            0x686868,
            0xbababa,
            0x6cbf43,
            0xeca944,
            0x3199e1,
            0x9e75c7,
            0x46ba94,
            0xf8f9fa,
            0x035bd6,
        ),
        terminal_theme(
            "github-dark",
            "GitHub Dark",
            0xe6edf3,
            0x6e7681,
            0xb1bac4,
            0x3fb950,
            0xd29922,
            0x58a6ff,
            0xbc8cff,
            0x39c5cf,
            0x0d1117,
            0xe6edf3,
        ),
        terminal_theme(
            "github-dark-dimmed",
            "GitHub Dark Dimmed",
            0xadbac7,
            0x636e7b,
            0x909dab,
            0x57ab5a,
            0xc69026,
            0x539bf5,
            0xb083f0,
            0x39c5cf,
            0x22272e,
            0xadbac7,
        ),
        terminal_theme(
            "github-light",
            "GitHub Light",
            0x1f2328,
            0x57606a,
            0x6e7781,
            0x116329,
            0x4d2d00,
            0x0969da,
            0x8250df,
            0x1b7c83,
            0xffffff,
            0x1f2328,
        ),
        terminal_theme(
            "nightfox", "Nightfox", 0xcdcecf, 0x575860, 0xdfdfe0, 0x81b29a, 0xdbc074, 0x719cd6,
            0x9d79d6, 0x63cdcf, 0xcdcecf, 0x2b3b51,
        ),
        terminal_theme(
            "dayfox", "Dayfox", 0x3d2b5a, 0x534c45, 0xbfb6ae, 0x396847, 0xac5402, 0x2848a9,
            0x6e33ce, 0x287980, 0x3d2b5a, 0xe7d2be,
        ),
        terminal_theme(
            "duskfox", "Duskfox", 0xe0def4, 0x544d8a, 0xe0def4, 0xa3be8c, 0xf6c177, 0x569fba,
            0xc4a7e7, 0x9ccfd8, 0xe0def4, 0x433c59,
        ),
        terminal_theme(
            "dawnfox", "Dawnfox", 0x575279, 0x5f5695, 0xb2b6bd, 0x618774, 0xea9d34, 0x286983,
            0x907aa9, 0x56949f, 0x575279, 0xd0d8d8,
        ),
        terminal_theme(
            "flexoki-dark",
            "Flexoki Dark",
            0xcecdc3,
            0x575653,
            0x878580,
            0x879a39,
            0xd0a215,
            0x4385be,
            0xce5d97,
            0x3aa99f,
            0xcecdc3,
            0x403e3c,
        ),
        terminal_theme(
            "flexoki-light",
            "Flexoki Light",
            0x100f0f,
            0xb7b5ac,
            0x6f6e69,
            0x66800b,
            0xad8301,
            0x205ea6,
            0xa02f6f,
            0x24837b,
            0x100f0f,
            0xcecdc3,
        ),
        terminal_theme(
            "material-dark",
            "Material Dark",
            0xe5e5e5,
            0x4f4f4f,
            0xefefef,
            0x457b24,
            0xf6981e,
            0x134eb2,
            0x701aa2,
            0x0e717c,
            0x3d3d3d,
            0xdfdfdf,
        ),
        terminal_theme(
            "sonokai", "Sonokai", 0xe2e2e3, 0x7f8490, 0xe2e2e3, 0x9ed072, 0xe7c664, 0x76cce0,
            0xb39df3, 0xf39660, 0xe2e2e3, 0x414550,
        ),
        terminal_theme(
            "synthwave",
            "Synthwave",
            0xdad9c7,
            0x7f7094,
            0xffffff,
            0x1ebb2b,
            0xfdf834,
            0x2186ec,
            0xf85a21,
            0x12c3e2,
            0x000000,
            0x19cde6,
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

#[allow(clippy::too_many_arguments)]
fn terminal_theme(
    id: &str,
    name: &str,
    foreground: u32,
    muted: u32,
    subtle: u32,
    green: u32,
    yellow: u32,
    blue: u32,
    magenta: u32,
    cyan: u32,
    selected_fg: u32,
    selected_bg: u32,
) -> Theme {
    theme(
        id,
        name,
        ThemeColors {
            text: rgb(foreground),
            muted: rgb(muted),
            subtle: rgb(subtle),
            title: rgb(cyan),
            accent: rgb(yellow),
            success: rgb(green),
            selected_fg: rgb(selected_fg),
            selected_bg: rgb(selected_bg),
            settings_selected_bg: rgb(cyan),
            editing_bg: rgb(yellow),
            link: rgb(blue),
            focus: rgb(cyan),
            border: rgb(muted),
            status: rgb(magenta),
        },
    )
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
    fn built_in_catalog_includes_common_terminal_theme_presets() {
        let catalog = ThemeCatalog::built_in();
        let ids = catalog
            .themes()
            .iter()
            .map(|theme| theme.id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"tokyo-night"));
        assert!(ids.contains(&"rose-pine"));
        assert!(ids.contains(&"nord"));
        assert!(ids.contains(&"solarized-dark"));
        assert!(ids.contains(&"kanagawa-wave"));
        assert!(ids.contains(&"github-dark"));
        assert!(ids.contains(&"flexoki-light"));
        assert!(ids.contains(&"synthwave"));
        assert!(ids.len() >= 40);
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
