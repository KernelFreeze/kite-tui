use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub file: String,
    pub feed_url: Url,
}

impl Category {
    pub fn file_stem(&self) -> &str {
        self.file.strip_suffix(".json").unwrap_or(&self.file)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Article {
    pub id: Uuid,
    pub title: String,
    pub link: Option<Url>,
    pub summary: String,
    #[serde(default)]
    pub summary_blocks: Vec<SummaryBlock>,
    pub published_at: Option<OffsetDateTime>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SummaryBlock {
    Heading { level: u8, text: String },
    Paragraph(String),
    List { ordered: bool, items: Vec<String> },
    Quote(String),
}
