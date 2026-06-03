use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::{fs, io};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::models::Article;

const QUALIFIER: &str = "dev";
const ORGANIZATION: &str = "CelesteLove";
const APPLICATION: &str = "Kite";
const READ_ARTICLES_FILE: &str = "read_articles.toml";

pub type Result<T> = std::result::Result<T, ReadStateError>;

#[derive(Debug, Clone)]
pub struct ReadArticles {
    day: String,
    ids: BTreeSet<Uuid>,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedReadArticles {
    #[serde(default)]
    day: String,

    #[serde(default)]
    articles: Vec<Uuid>,
}

impl ReadArticles {
    pub fn load() -> Result<Self> {
        Self::load_from_path(read_articles_file()?, current_day_key())
    }

    pub fn empty_for_today() -> Self {
        Self::empty_for_day(current_day_key(), None)
    }

    pub fn is_read(&self, article: &Article) -> bool {
        self.day == current_day_key() && self.ids.contains(&article.id)
    }

    pub fn mark_read_id(&mut self, article_id: Uuid) -> Result<bool> {
        self.clear_if_stale()?;
        let inserted = self.ids.insert(article_id);
        if inserted {
            self.save()?;
        }
        Ok(inserted)
    }

    fn load_from_path(path: PathBuf, today: String) -> Result<Self> {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::empty_for_day(today, Some(path)));
            }
            Err(source) => return Err(ReadStateError::Io { path, source }),
        };

        let persisted = toml::from_str::<PersistedReadArticles>(&contents).map_err(|source| {
            ReadStateError::Parse {
                path: path.clone(),
                source,
            }
        })?;

        if persisted.day != today {
            let state = Self::empty_for_day(today, Some(path));
            state.save()?;
            return Ok(state);
        }

        Ok(Self {
            day: persisted.day,
            ids: persisted.articles.into_iter().collect(),
            path: Some(path),
        })
    }

    fn empty_for_day(day: String, path: Option<PathBuf>) -> Self {
        Self {
            day,
            ids: BTreeSet::new(),
            path,
        }
    }

    fn clear_if_stale(&mut self) -> Result<()> {
        let today = current_day_key();
        if self.day != today {
            self.day = today;
            self.ids.clear();
            self.save()?;
        }
        Ok(())
    }

    fn save(&self) -> Result<Option<PathBuf>> {
        let Some(path) = &self.path else {
            return Ok(None);
        };
        let parent = path
            .parent()
            .ok_or_else(|| ReadStateError::MissingParent(path.clone()))?;
        fs::create_dir_all(parent).map_err(|source| ReadStateError::Io {
            path: parent.to_owned(),
            source,
        })?;

        let persisted = PersistedReadArticles {
            day: self.day.clone(),
            articles: self.ids.iter().copied().collect(),
        };
        let contents = toml::to_string_pretty(&persisted).map_err(ReadStateError::Serialize)?;
        fs::write(path, contents).map_err(|source| ReadStateError::Io {
            path: path.clone(),
            source,
        })?;

        Ok(Some(path.clone()))
    }

    #[cfg(test)]
    fn for_day(day: &str) -> Self {
        Self::empty_for_day(day.to_owned(), None)
    }
}

pub fn read_articles_file() -> Result<PathBuf> {
    Ok(project_dirs()?.data_local_dir().join(READ_ARTICLES_FILE))
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION).ok_or(ReadStateError::DataDir)
}

fn current_day_key() -> String {
    OffsetDateTime::now_utc().date().to_string()
}

#[derive(Debug, Error)]
pub enum ReadStateError {
    #[error("could not determine the platform data directory")]
    DataDir,

    #[error("read-state path `{0}` does not have a parent directory")]
    MissingParent(PathBuf),

    #[error("read-state I/O failed at `{}`: {source}", display_path(path))]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(
        "failed to parse read-state TOML at `{}`: {source}",
        display_path(path)
    )]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize read-state TOML: {0}")]
    Serialize(toml::ser::Error),
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::models::SummaryBlock;

    fn article(id: Uuid) -> Article {
        Article {
            id,
            title: "Story".to_owned(),
            link: None,
            summary: String::new(),
            summary_blocks: Vec::<SummaryBlock>::new(),
            published_at: None,
            categories: Vec::new(),
        }
    }

    fn temp_read_state_path(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kite-read-state-{}-{timestamp}-{name}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn read_articles_round_trip_as_toml() {
        let persisted = PersistedReadArticles {
            day: "2026-05-21".to_owned(),
            articles: vec![Uuid::nil()],
        };

        let encoded = toml::to_string(&persisted).unwrap();
        let decoded = toml::from_str::<PersistedReadArticles>(&encoded).unwrap();

        assert_eq!(decoded, persisted);
    }

    #[test]
    fn mark_read_tracks_article_ids() {
        let id = Uuid::nil();
        let mut read_articles = ReadArticles::for_day(&current_day_key());

        assert!(!read_articles.is_read(&article(id)));
        assert!(read_articles.mark_read_id(id).unwrap());
        assert!(read_articles.is_read(&article(id)));
        assert!(!read_articles.mark_read_id(id).unwrap());
    }

    #[test]
    fn stale_read_articles_are_cleared_on_load() {
        let path = temp_read_state_path("stale");
        let old_article = Uuid::nil();
        fs::write(
            &path,
            r#"
day = "2026-01-01"
articles = ["00000000-0000-0000-0000-000000000000"]
"#,
        )
        .unwrap();

        let read_articles = ReadArticles::load_from_path(path.clone(), "2026-05-21".to_owned())
            .expect("loads stale state");

        assert!(!read_articles.is_read(&article(old_article)));
        let persisted =
            toml::from_str::<PersistedReadArticles>(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            persisted,
            PersistedReadArticles {
                day: "2026-05-21".to_owned(),
                articles: Vec::new(),
            }
        );

        fs::remove_file(path).unwrap();
    }
}
