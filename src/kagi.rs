use std::{cmp::Reverse, time::Duration};

use feed_rs::{model::Entry, parser};
use serde::Deserialize;
use time::OffsetDateTime;
use tracing::{debug, instrument};
use url::Url;
use uuid::Uuid;

use crate::{
    error::{KiteError, Result},
    models::{Article, Category},
};

const INDEX_FILE: &str = "kite.json";

#[derive(Debug, Deserialize)]
struct KiteIndex {
    #[serde(rename = "timestamp")]
    _timestamp: i64,
    categories: Vec<KiteIndexCategory>,
}

#[derive(Debug, Deserialize)]
struct KiteIndexCategory {
    name: String,
    file: String,
}

#[derive(Debug, Clone)]
pub struct KagiClient {
    http: reqwest::Client,
    base_url: Url,
}

impl KagiClient {
    pub fn new(base_url: Url, timeout: Duration) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("kite/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            http,
            base_url: normalize_base_url(base_url),
        })
    }

    #[instrument(skip(self))]
    pub async fn categories(&self) -> Result<Vec<Category>> {
        let index_url = self.join(INDEX_FILE)?;
        debug!(%index_url, "fetching Kagi category index");

        let index = self
            .http
            .get(index_url)
            .send()
            .await?
            .error_for_status()?
            .json::<KiteIndex>()
            .await?;

        let categories = index
            .categories
            .into_iter()
            .map(|category| {
                let feed_url = feed_url_for_file(&self.base_url, &category.file)?;
                Ok(Category {
                    name: category.name,
                    file: category.file,
                    feed_url,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        if categories.is_empty() {
            return Err(KiteError::EmptyCategoryIndex);
        }

        Ok(categories)
    }

    #[instrument(skip(self, category), fields(category = %category.name, url = %category.feed_url))]
    pub async fn articles(&self, category: &Category) -> Result<Vec<Article>> {
        let feed_url = category.feed_url.clone();
        debug!("fetching category feed");

        let body = self
            .http
            .get(feed_url.clone())
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        let feed = parser::parse(body.as_ref()).map_err(|source| KiteError::FeedParse {
            url: feed_url,
            message: source.to_string(),
        })?;

        let mut articles = feed
            .entries
            .into_iter()
            .map(|entry| article_from_entry(category, &entry))
            .collect::<Vec<_>>();

        articles.sort_by_key(|article| Reverse(article.published_at));

        Ok(articles)
    }

    fn join(&self, path: &str) -> Result<Url> {
        self.base_url.join(path).map_err(|source| KiteError::Url {
            value: format!("{} + {path}", self.base_url),
            source,
        })
    }
}

pub fn feed_url_for_file(base_url: &Url, file: &str) -> Result<Url> {
    let stem = file
        .strip_suffix(".json")
        .ok_or_else(|| KiteError::InvalidCategoryFile(file.to_owned()))?;
    let feed_file = format!("{stem}.xml");

    base_url.join(&feed_file).map_err(|source| KiteError::Url {
        value: format!("{} + {feed_file}", base_url),
        source,
    })
}

pub fn article_from_entry(category: &Category, entry: &Entry) -> Article {
    let title = entry
        .title
        .as_ref()
        .map(|text| html_to_text(&text.content))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Untitled".to_owned());

    let link = primary_link(entry).and_then(|href| Url::parse(href).ok());
    let id_seed = link
        .as_ref()
        .map(|url| url.as_str().to_owned())
        .filter(|seed| !seed.trim().is_empty())
        .unwrap_or_else(|| fallback_article_id_seed(category, entry, &title));

    let summary_html = entry
        .summary
        .as_ref()
        .map(|summary| summary.content.as_str())
        .or_else(|| {
            entry
                .content
                .as_ref()
                .and_then(|content| content.body.as_deref())
        })
        .unwrap_or_default();

    let mut categories = entry
        .categories
        .iter()
        .map(|category| {
            category
                .label
                .clone()
                .unwrap_or_else(|| category.term.clone())
        })
        .filter(|label| !label.trim().is_empty())
        .collect::<Vec<_>>();

    if categories.is_empty() {
        categories.push(category.name.clone());
    }

    Article {
        id: Uuid::new_v5(&Uuid::NAMESPACE_URL, id_seed.as_bytes()),
        title,
        link,
        summary: html_to_text(summary_html),
        published_at: entry.published.or(entry.updated).and_then(|published| {
            let nanos = i128::from(published.timestamp()) * 1_000_000_000
                + i128::from(published.timestamp_subsec_nanos());
            OffsetDateTime::from_unix_timestamp_nanos(nanos).ok()
        }),
        categories,
    }
}

pub fn html_to_text(input: &str) -> String {
    let mut rendered = String::with_capacity(input.len());
    let mut tag = String::new();
    let mut in_tag = false;

    for character in input.chars() {
        if in_tag {
            if character == '>' {
                let normalized = tag.trim().trim_start_matches('/').to_ascii_lowercase();
                let tag_name = normalized.split_whitespace().next().unwrap_or_default();
                if matches!(
                    tag_name,
                    "blockquote" | "br" | "h1" | "h2" | "h3" | "li" | "ol" | "p" | "ul"
                ) {
                    rendered.push('\n');
                }
                tag.clear();
                in_tag = false;
            } else {
                tag.push(character);
            }
        } else if character == '<' {
            in_tag = true;
        } else {
            rendered.push(character);
        }
    }

    let decoded = html_escape::decode_html_entities(&rendered);
    decoded
        .lines()
        .map(collapse_whitespace)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn primary_link(entry: &Entry) -> Option<&str> {
    entry
        .links
        .iter()
        .find(|link| {
            link.rel
                .as_deref()
                .is_none_or(|rel| rel.eq_ignore_ascii_case("alternate"))
        })
        .or_else(|| entry.links.first())
        .map(|link| link.href.as_str())
}

fn fallback_article_id_seed(category: &Category, entry: &Entry, title: &str) -> String {
    if !entry.id.trim().is_empty() {
        return entry.id.clone();
    }

    let date = entry
        .published
        .as_ref()
        .or(entry.updated.as_ref())
        .map(|date| date.to_rfc3339())
        .unwrap_or_default();

    format!("{}:{title}:{date}", category.file)
}

fn collapse_whitespace(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut pending_space = false;

    for character in line.chars() {
        if character.is_whitespace() {
            pending_space = true;
        } else {
            if pending_space && !output.is_empty() {
                output.push(' ');
            }
            output.push(character);
            pending_space = false;
        }
    }

    output
}

fn normalize_base_url(mut base_url: Url) -> Url {
    if !base_url.path().ends_with('/') {
        let normalized_path = format!("{}/", base_url.path());
        base_url.set_path(&normalized_path);
    }

    base_url
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category() -> Category {
        Category {
            name: "World".to_owned(),
            file: "world.json".to_owned(),
            feed_url: Url::parse("https://news.kagi.com/world.xml").unwrap(),
        }
    }

    #[test]
    fn builds_feed_url_from_category_file() {
        let base_url = Url::parse("https://news.kagi.com/").unwrap();

        assert_eq!(
            feed_url_for_file(&base_url, "world.json").unwrap().as_str(),
            "https://news.kagi.com/world.xml"
        );
        assert_eq!(
            feed_url_for_file(&base_url, "germany_|_baden-württemberg.json")
                .unwrap()
                .as_str(),
            "https://news.kagi.com/germany_|_baden-w%C3%BCrttemberg.xml"
        );
    }

    #[test]
    fn rejects_non_json_category_file() {
        let base_url = Url::parse("https://news.kagi.com/").unwrap();

        assert!(matches!(
            feed_url_for_file(&base_url, "world.xml"),
            Err(KiteError::InvalidCategoryFile(file)) if file == "world.xml"
        ));
    }

    #[test]
    fn converts_kagi_rss_item_to_article() {
        let feed = parser::parse(
            &br#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Kagi News - World</title>
    <link>https://news.kagi.com/world.xml</link>
    <description>Latest news from Kagi News for World category.</description>
    <item>
      <guid>story-1</guid>
      <title>Test &amp; Story</title>
      <link>https://kite.kagi.com/story-1</link>
      <description><![CDATA[
        <p>First&nbsp;paragraph.</p>
        <h3>Sources:</h3>
        <ul><li><a href="https://example.com">Example</a> - example.com</li></ul>
      ]]></description>
      <category>World/Science</category>
      <pubDate>Tue, 14 Apr 2026 05:41:14 +0000</pubDate>
    </item>
  </channel>
</rss>"#[..],
        )
        .unwrap();

        let article = article_from_entry(&category(), &feed.entries[0]);
        let article_again = article_from_entry(&category(), &feed.entries[0]);

        assert_eq!(article.id, article_again.id);
        assert_eq!(article.title, "Test & Story");
        assert_eq!(
            article.link.as_ref().map(Url::as_str),
            Some("https://kite.kagi.com/story-1")
        );
        assert!(article.summary.contains("First paragraph."));
        assert!(article.summary.contains("Sources:"));
        assert_eq!(article.categories, ["World/Science"]);
        assert!(article.published_at.is_some());
    }

    #[test]
    fn strips_basic_html_and_decodes_entities() {
        assert_eq!(
            html_to_text("<p>Hello&nbsp;there.</p><br><ul><li>One &amp; two</li></ul>"),
            "Hello there.\nOne & two"
        );
    }
}
