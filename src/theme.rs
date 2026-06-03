use std::path::{Path, PathBuf};
use std::{fs, io};

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
                text: rgb(0xCDD6F4),
                muted: rgb(0x6C7086),
                subtle: rgb(0xA6ADC8),
                title: rgb(0x89DCEB),
                accent: rgb(0xF9E2AF),
                success: rgb(0xA6E3A1),
                selected_fg: rgb(0x11111B),
                selected_bg: rgb(0xA6E3A1),
                settings_selected_bg: rgb(0x89DCEB),
                editing_bg: rgb(0xF9E2AF),
                link: rgb(0x89DCEB),
                focus: rgb(0x89DCEB),
                border: rgb(0x6C7086),
                status: rgb(0xCBA6F7),
            },
        ),
        theme(
            "catppuccin-latte",
            "Catppuccin Latte",
            ThemeColors {
                text: rgb(0x4C4F69),
                muted: rgb(0x9CA0B0),
                subtle: rgb(0x6C6F85),
                title: rgb(0x04A5E5),
                accent: rgb(0xDF8E1D),
                success: rgb(0x40A02B),
                selected_fg: rgb(0xEFF1F5),
                selected_bg: rgb(0x40A02B),
                settings_selected_bg: rgb(0x04A5E5),
                editing_bg: rgb(0xDF8E1D),
                link: rgb(0x1E66F5),
                focus: rgb(0x04A5E5),
                border: rgb(0x9CA0B0),
                status: rgb(0x8839EF),
            },
        ),
        terminal_theme(
            "catppuccin-frappe",
            "Catppuccin Frappe",
            0xC6D0F5,
            0x626880,
            0xA5ADCE,
            0xA6D189,
            0xE5C890,
            0x8CAAEE,
            0xF4B8E4,
            0x81C8BE,
            0xC6D0F5,
            0x626880,
        ),
        terminal_theme(
            "catppuccin-macchiato",
            "Catppuccin Macchiato",
            0xCAD3F5,
            0x5B6078,
            0xA5ADCB,
            0xA6DA95,
            0xEED49F,
            0x8AADF4,
            0xF5BDE6,
            0x8BD5CA,
            0xCAD3F5,
            0x5B6078,
        ),
        theme(
            "dracula",
            "Dracula",
            ThemeColors {
                text: rgb(0xF8F8F2),
                muted: rgb(0x6272A4),
                subtle: rgb(0xBFBFBF),
                title: rgb(0x8BE9FD),
                accent: rgb(0xF1FA8C),
                success: rgb(0x50FA7B),
                selected_fg: rgb(0x282A36),
                selected_bg: rgb(0x50FA7B),
                settings_selected_bg: rgb(0x8BE9FD),
                editing_bg: rgb(0xF1FA8C),
                link: rgb(0x8BE9FD),
                focus: rgb(0xFF79C6),
                border: rgb(0x6272A4),
                status: rgb(0xBD93F9),
            },
        ),
        theme(
            "gruvbox-dark",
            "Gruvbox Dark",
            ThemeColors {
                text: rgb(0xEBDBB2),
                muted: rgb(0x928374),
                subtle: rgb(0xA89984),
                title: rgb(0x83A598),
                accent: rgb(0xFABD2F),
                success: rgb(0xB8BB26),
                selected_fg: rgb(0x282828),
                selected_bg: rgb(0xB8BB26),
                settings_selected_bg: rgb(0x83A598),
                editing_bg: rgb(0xFABD2F),
                link: rgb(0x83A598),
                focus: rgb(0x83A598),
                border: rgb(0x665C54),
                status: rgb(0xD3869B),
            },
        ),
        terminal_theme(
            "gruvbox-light",
            "Gruvbox Light",
            0x3C3836,
            0x928374,
            0x7C6F64,
            0x98971A,
            0xD79921,
            0x458588,
            0xB16286,
            0x689D6A,
            0xFBF1C7,
            0x3C3836,
        ),
        terminal_theme(
            "tokyo-night",
            "Tokyo Night",
            0xC0CAF5,
            0x414868,
            0xA9B1D6,
            0x9ECE6A,
            0xE0AF68,
            0x7AA2F7,
            0xBB9AF7,
            0x7DCFFF,
            0xC0CAF5,
            0x33467C,
        ),
        terminal_theme(
            "tokyo-night-storm",
            "Tokyo Night Storm",
            0xC0CAF5,
            0x4E5575,
            0xA9B1D6,
            0x9ECE6A,
            0xE0AF68,
            0x7AA2F7,
            0xBB9AF7,
            0x7DCFFF,
            0xC0CAF5,
            0x364A82,
        ),
        terminal_theme(
            "tokyo-night-moon",
            "Tokyo Night Moon",
            0xC8D3F5,
            0x444A73,
            0x828BB8,
            0xC3E88D,
            0xFFC777,
            0x82AAFF,
            0xC099FF,
            0x86E1FC,
            0xC8D3F5,
            0x2D3F76,
        ),
        terminal_theme(
            "rose-pine",
            "Rose Pine",
            0xE0DEF4,
            0x6E6A86,
            0xE0DEF4,
            0x31748F,
            0xF6C177,
            0x9CCFD8,
            0xC4A7E7,
            0xEBBCBA,
            0xE0DEF4,
            0x403D52,
        ),
        terminal_theme(
            "rose-pine-moon",
            "Rose Pine Moon",
            0xE0DEF4,
            0x6E6A86,
            0xE0DEF4,
            0x3E8FB0,
            0xF6C177,
            0x9CCFD8,
            0xC4A7E7,
            0xEA9A97,
            0xE0DEF4,
            0x44415A,
        ),
        terminal_theme(
            "rose-pine-dawn",
            "Rose Pine Dawn",
            0x575279,
            0x9893A5,
            0x575279,
            0x286983,
            0xEA9D34,
            0x56949F,
            0x907AA9,
            0xD7827E,
            0x575279,
            0xDFDAD9,
        ),
        terminal_theme(
            "nord", "Nord", 0xD8DEE9, 0x596377, 0xE5E9F0, 0xA3BE8C, 0xEBCB8B, 0x81A1C1, 0xB48EAD,
            0x88C0D0, 0x4C566A, 0xECEFF4,
        ),
        terminal_theme(
            "solarized-dark",
            "Solarized Dark",
            0x839496,
            0x335E69,
            0xEEE8D5,
            0x859900,
            0xB58900,
            0x268BD2,
            0xD33682,
            0x2AA198,
            0x93A1A1,
            0x073642,
        ),
        terminal_theme(
            "solarized-light",
            "Solarized Light",
            0x657B83,
            0x839496,
            0x586E75,
            0x859900,
            0xB58900,
            0x268BD2,
            0xD33682,
            0x2AA198,
            0x586E75,
            0xEEE8D5,
        ),
        terminal_theme(
            "everforest-dark-hard",
            "Everforest Dark Hard",
            0xD3C6AA,
            0xA6B0A0,
            0xF2EFDF,
            0xA7C080,
            0xDBBC7F,
            0x7FBBB3,
            0xD699B6,
            0x83C092,
            0xD3C6AA,
            0x4C3743,
        ),
        terminal_theme(
            "everforest-light-medium",
            "Everforest Light Medium",
            0x5C6A72,
            0xA6B0A0,
            0xB2AF9F,
            0x9AB373,
            0xC1A266,
            0x7FBBB3,
            0xD699B6,
            0x83C092,
            0x5C6A72,
            0xEAEDC8,
        ),
        terminal_theme(
            "kanagawa-wave",
            "Kanagawa Wave",
            0xDCD7BA,
            0x727169,
            0xC8C093,
            0x76946A,
            0xC0A36E,
            0x7E9CD8,
            0x957FB8,
            0x6A9589,
            0x1F1F28,
            0xDCD7BA,
        ),
        terminal_theme(
            "kanagawa-dragon",
            "Kanagawa Dragon",
            0xC5C9C5,
            0xA6A69C,
            0xC8C093,
            0x8A9A7B,
            0xC4B28A,
            0x8BA4B0,
            0xA292A3,
            0x8EA4A2,
            0x181616,
            0xC5C9C5,
        ),
        terminal_theme(
            "atom-one-dark",
            "Atom One Dark",
            0xABB2BF,
            0x767676,
            0xABB2BF,
            0x98C379,
            0xE5C07B,
            0x61AFEF,
            0xC678DD,
            0x56B6C2,
            0xABB2BF,
            0x323844,
        ),
        terminal_theme(
            "atom-one-light",
            "Atom One Light",
            0x2A2C33,
            0x000000,
            0xBBBBBB,
            0x3F953A,
            0xD2B67C,
            0x2F5AF3,
            0x950095,
            0x3F953A,
            0x2A2C33,
            0xEDEDED,
        ),
        terminal_theme(
            "monokai-pro",
            "Monokai Pro",
            0xFCFCFA,
            0x727072,
            0xFCFCFA,
            0xA9DC76,
            0xFFD866,
            0xFC9867,
            0xAB9DF2,
            0x78DCE8,
            0xFCFCFA,
            0x5B595C,
        ),
        terminal_theme(
            "monokai-remastered",
            "Monokai Remastered",
            0xD9D9D9,
            0x625E4C,
            0xC4C5B5,
            0x98E024,
            0xFD971F,
            0x9D65FF,
            0xF4005F,
            0x58D1EB,
            0xFFFFFF,
            0x343434,
        ),
        terminal_theme(
            "ayu-dark", "Ayu Dark", 0xBFBDB6, 0x686868, 0xC7C7C7, 0x7FD962, 0xF9AF4F, 0x53BDFA,
            0xCDA1FA, 0x90E1C6, 0x0B0E14, 0x409FFF,
        ),
        terminal_theme(
            "ayu-mirage",
            "Ayu Mirage",
            0xCCCAC2,
            0x686868,
            0xC7C7C7,
            0x87D96C,
            0xFACC6E,
            0x6DCBFA,
            0xDABAFA,
            0x90E1C6,
            0x1F2430,
            0x409FFF,
        ),
        terminal_theme(
            "ayu-light",
            "Ayu Light",
            0x5C6166,
            0x686868,
            0xBABABA,
            0x6CBF43,
            0xECA944,
            0x3199E1,
            0x9E75C7,
            0x46BA94,
            0xF8F9FA,
            0x035BD6,
        ),
        terminal_theme(
            "github-dark",
            "GitHub Dark",
            0xE6EDF3,
            0x6E7681,
            0xB1BAC4,
            0x3FB950,
            0xD29922,
            0x58A6FF,
            0xBC8CFF,
            0x39C5CF,
            0x0D1117,
            0xE6EDF3,
        ),
        terminal_theme(
            "github-dark-dimmed",
            "GitHub Dark Dimmed",
            0xADBAC7,
            0x636E7B,
            0x909DAB,
            0x57AB5A,
            0xC69026,
            0x539BF5,
            0xB083F0,
            0x39C5CF,
            0x22272E,
            0xADBAC7,
        ),
        terminal_theme(
            "github-light",
            "GitHub Light",
            0x1F2328,
            0x57606A,
            0x6E7781,
            0x116329,
            0x4D2D00,
            0x0969DA,
            0x8250DF,
            0x1B7C83,
            0xFFFFFF,
            0x1F2328,
        ),
        terminal_theme(
            "nightfox", "Nightfox", 0xCDCECF, 0x575860, 0xDFDFE0, 0x81B29A, 0xDBC074, 0x719CD6,
            0x9D79D6, 0x63CDCF, 0xCDCECF, 0x2B3B51,
        ),
        terminal_theme(
            "dayfox", "Dayfox", 0x3D2B5A, 0x534C45, 0xBFB6AE, 0x396847, 0xAC5402, 0x2848A9,
            0x6E33CE, 0x287980, 0x3D2B5A, 0xE7D2BE,
        ),
        terminal_theme(
            "duskfox", "Duskfox", 0xE0DEF4, 0x544D8A, 0xE0DEF4, 0xA3BE8C, 0xF6C177, 0x569FBA,
            0xC4A7E7, 0x9CCFD8, 0xE0DEF4, 0x433C59,
        ),
        terminal_theme(
            "dawnfox", "Dawnfox", 0x575279, 0x5F5695, 0xB2B6BD, 0x618774, 0xEA9D34, 0x286983,
            0x907AA9, 0x56949F, 0x575279, 0xD0D8D8,
        ),
        terminal_theme(
            "flexoki-dark",
            "Flexoki Dark",
            0xCECDC3,
            0x575653,
            0x878580,
            0x879A39,
            0xD0A215,
            0x4385BE,
            0xCE5D97,
            0x3AA99F,
            0xCECDC3,
            0x403E3C,
        ),
        terminal_theme(
            "flexoki-light",
            "Flexoki Light",
            0x100F0F,
            0xB7B5AC,
            0x6F6E69,
            0x66800B,
            0xAD8301,
            0x205EA6,
            0xA02F6F,
            0x24837B,
            0x100F0F,
            0xCECDC3,
        ),
        terminal_theme(
            "material-dark",
            "Material Dark",
            0xE5E5E5,
            0x4F4F4F,
            0xEFEFEF,
            0x457B24,
            0xF6981E,
            0x134EB2,
            0x701AA2,
            0x0E717C,
            0x3D3D3D,
            0xDFDFDF,
        ),
        terminal_theme(
            "sonokai", "Sonokai", 0xE2E2E3, 0x7F8490, 0xE2E2E3, 0x9ED072, 0xE7C664, 0x76CCE0,
            0xB39DF3, 0xF39660, 0xE2E2E3, 0x414550,
        ),
        terminal_theme(
            "synthwave",
            "Synthwave",
            0xDAD9C7,
            0x7F7094,
            0xFFFFFF,
            0x1EBB2B,
            0xFDF834,
            0x2186EC,
            0xF85A21,
            0x12C3E2,
            0x000000,
            0x19CDE6,
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
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
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
        assert_eq!(theme.colors.text, Color::Rgb(0xCD, 0xD6, 0xF4));
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
