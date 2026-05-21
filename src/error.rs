use thiserror::Error;
use url::Url;

pub type Result<T> = std::result::Result<T, KiteError>;

#[derive(Debug, Error)]
pub enum KiteError {
    #[error("category index did not include any categories")]
    EmptyCategoryIndex,

    #[error("category `{0}` was not found")]
    CategoryNotFound(String),

    #[error("category file `{0}` must end with .json")]
    InvalidCategoryFile(String),

    #[error("failed to parse feed from {url}: {message}")]
    FeedParse { url: Url, message: String },

    #[error("failed to build URL `{value}`: {source}")]
    Url {
        value: String,
        source: url::ParseError,
    },

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("terminal I/O failed: {0}")]
    Io(#[from] std::io::Error),
}
