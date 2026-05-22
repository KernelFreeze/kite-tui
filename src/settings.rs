use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::Category;

const QUALIFIER: &str = "dev";
const ORGANIZATION: &str = "CelesteLove";
const APPLICATION: &str = "Kite";
const SETTINGS_FILE: &str = "settings.toml";

pub type Result<T> = std::result::Result<T, SettingsError>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub categories: CategorySettings,

    #[serde(default)]
    pub keybinds: KeyBindingSettings,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategorySettings {
    #[serde(default)]
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBindingSettings {
    #[serde(default = "default_help_key")]
    pub help: String,

    #[serde(default = "default_settings_key", alias = "config")]
    pub settings: String,

    #[serde(default = "default_category_filter_key")]
    pub category_filter: String,

    #[serde(default = "default_next_category_key")]
    pub next_category: String,

    #[serde(default = "default_previous_category_key")]
    pub previous_category: String,

    #[serde(default = "default_refresh_key")]
    pub refresh: String,

    #[serde(default = "default_refresh_all_key")]
    pub refresh_all: String,

    #[serde(default = "default_quit_key")]
    pub quit: String,

    #[serde(default = "default_reset_defaults_key")]
    pub reset_defaults: String,

    #[serde(default = "default_jump_top_key")]
    pub jump_top: String,

    #[serde(default = "default_jump_bottom_key")]
    pub jump_bottom: String,
}

impl Default for KeyBindingSettings {
    fn default() -> Self {
        Self {
            help: default_help_key(),
            settings: default_settings_key(),
            category_filter: default_category_filter_key(),
            next_category: default_next_category_key(),
            previous_category: default_previous_category_key(),
            refresh: default_refresh_key(),
            refresh_all: default_refresh_all_key(),
            quit: default_quit_key(),
            reset_defaults: default_reset_defaults_key(),
            jump_top: default_jump_top_key(),
            jump_bottom: default_jump_bottom_key(),
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self> {
        let path = settings_file()?;
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(SettingsError::Io { path, source }),
        };

        toml::from_str(&contents).map_err(|source| SettingsError::Parse { path, source })
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = settings_file()?;
        let parent = path
            .parent()
            .ok_or_else(|| SettingsError::MissingParent(path.clone()))?;
        fs::create_dir_all(parent).map_err(|source| SettingsError::Io {
            path: parent.to_owned(),
            source,
        })?;

        let contents = toml::to_string_pretty(self).map_err(SettingsError::Serialize)?;
        fs::write(&path, contents).map_err(|source| SettingsError::Io {
            path: path.clone(),
            source,
        })?;

        Ok(path)
    }
}

pub fn settings_file() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join(SETTINGS_FILE))
}

pub fn category_key(category: &Category) -> String {
    normalize_category_key(category.file_stem())
}

pub fn category_matches_key(category: &Category, key: &str) -> bool {
    let key = normalize_category_key(key);
    [
        category.name.as_str(),
        category.file.as_str(),
        category.file_stem(),
    ]
    .into_iter()
    .any(|value| normalize_category_key(value) == key)
}

pub fn normalize_category_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION).ok_or(SettingsError::ConfigDir)
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("could not determine the platform config directory")]
    ConfigDir,

    #[error("settings path `{0}` does not have a parent directory")]
    MissingParent(PathBuf),

    #[error("settings I/O failed at `{}`: {source}", display_path(path))]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse settings TOML at `{}`: {source}", display_path(path))]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize settings TOML: {0}")]
    Serialize(toml::ser::Error),
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn default_help_key() -> String {
    "?".to_owned()
}

fn default_settings_key() -> String {
    ",".to_owned()
}

fn default_category_filter_key() -> String {
    "/".to_owned()
}

fn default_next_category_key() -> String {
    "tab".to_owned()
}

fn default_previous_category_key() -> String {
    "shift+tab".to_owned()
}

fn default_refresh_key() -> String {
    "r".to_owned()
}

fn default_refresh_all_key() -> String {
    "R".to_owned()
}

fn default_quit_key() -> String {
    "q".to_owned()
}

fn default_reset_defaults_key() -> String {
    "d".to_owned()
}

fn default_jump_top_key() -> String {
    "gg".to_owned()
}

fn default_jump_bottom_key() -> String {
    "G".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    fn category(name: &str, file: &str) -> Category {
        Category {
            name: name.to_owned(),
            file: file.to_owned(),
            feed_url: Url::parse("https://news.kagi.com/world.xml").unwrap(),
        }
    }

    #[test]
    fn category_keys_normalize_file_stems() {
        let category = category("Today in History", "today_in_history.json");

        assert_eq!(category_key(&category), "todayinhistory");
    }

    #[test]
    fn category_key_matching_accepts_name_file_or_stem() {
        let category = category("Today in History", "today_in_history.json");

        assert!(category_matches_key(&category, "Today in History"));
        assert!(category_matches_key(&category, "today_in_history.json"));
        assert!(category_matches_key(&category, "todayinhistory"));
        assert!(!category_matches_key(&category, "technology"));
    }

    #[test]
    fn settings_round_trip_as_toml() {
        let settings = Settings {
            categories: CategorySettings {
                enabled: vec!["world".to_owned(), "technology".to_owned()],
            },
            keybinds: KeyBindingSettings::default(),
        };

        let encoded = toml::to_string(&settings).unwrap();
        let decoded = toml::from_str::<Settings>(&encoded).unwrap();

        assert_eq!(decoded, settings);
    }

    #[test]
    fn missing_keybinds_use_defaults() {
        let decoded = toml::from_str::<Settings>(
            r#"
            [categories]
            enabled = ["world"]
            "#,
        )
        .unwrap();

        assert_eq!(
            decoded.keybinds,
            KeyBindingSettings {
                help: "?".to_owned(),
                settings: ",".to_owned(),
                category_filter: "/".to_owned(),
                next_category: "tab".to_owned(),
                previous_category: "shift+tab".to_owned(),
                refresh: "r".to_owned(),
                refresh_all: "R".to_owned(),
                quit: "q".to_owned(),
                reset_defaults: "d".to_owned(),
                jump_top: "gg".to_owned(),
                jump_bottom: "G".to_owned(),
            }
        );
    }

    #[test]
    fn legacy_config_keybind_name_loads_as_settings() {
        let decoded = toml::from_str::<Settings>(
            r#"
            [keybinds]
            config = ";"
            "#,
        )
        .unwrap();

        assert_eq!(decoded.keybinds.settings, ";");
    }
}
