//! Category lookup, default enablement, and filter matching.

use crate::error::{KiteError, Result};
use crate::models::Category;
use crate::settings::{self, Settings};

pub(crate) const DEFAULT_ENABLED_CATEGORY_KEYS: &[&str] = &[
    "world",
    "gaming",
    "science",
    "ai",
    "technology",
    "business",
    "sports",
    "todayinhistory",
    "usa",
];

pub(crate) fn find_category(categories: &[Category], requested: &str) -> Option<usize> {
    let requested = requested.trim().to_ascii_lowercase();

    categories.iter().position(|category| {
        category.name.to_ascii_lowercase() == requested
            || category.file.to_ascii_lowercase() == requested
            || category.file_stem().to_ascii_lowercase() == requested
    })
}

pub(crate) fn find_category_by_settings_key(categories: &[Category], key: &str) -> Option<usize> {
    categories
        .iter()
        .position(|category| settings::category_matches_key(category, key))
}

pub(crate) fn enabled_categories_from_settings(
    categories: &[Category],
    settings: &Settings,
) -> Option<Vec<bool>> {
    if settings.categories.enabled.is_empty() {
        return None;
    }

    let enabled_categories = categories
        .iter()
        .map(|category| {
            settings
                .categories
                .enabled
                .iter()
                .any(|key| settings::category_matches_key(category, key))
        })
        .collect::<Vec<_>>();

    enabled_categories
        .iter()
        .any(|enabled| *enabled)
        .then_some(enabled_categories)
}

pub(crate) fn select_initial_category(
    categories: &[Category],
    enabled_categories: &mut [bool],
    requested: Option<&str>,
) -> Result<usize> {
    if let Some(requested) = requested {
        let selected_category = find_category(categories, requested)
            .ok_or_else(|| KiteError::CategoryNotFound(requested.to_owned()))?;
        if let Some(enabled) = enabled_categories.get_mut(selected_category) {
            *enabled = true;
        }
        return Ok(selected_category);
    }

    if let Some(world) = find_category(categories, "World")
        && enabled_categories.get(world).copied().unwrap_or(false)
    {
        return Ok(world);
    }

    Ok(enabled_categories
        .iter()
        .position(|enabled| *enabled)
        .unwrap_or(0))
}

pub(crate) fn default_enabled_categories(categories: &[Category]) -> Vec<bool> {
    let mut enabled_categories = categories
        .iter()
        .map(|category| {
            DEFAULT_ENABLED_CATEGORY_KEYS
                .iter()
                .any(|default| category_matches_default_key(category, default))
        })
        .collect::<Vec<_>>();

    if !enabled_categories.iter().any(|enabled| *enabled)
        && let Some(first_category) = enabled_categories.first_mut()
    {
        *first_category = true;
    }

    enabled_categories
}

fn category_matches_default_key(category: &Category, key: &str) -> bool {
    let key = settings::normalize_category_key(key);
    [
        category.name.as_str(),
        category.file.as_str(),
        category.file_stem(),
    ]
    .into_iter()
    .any(|value| settings::normalize_category_key(value) == key)
}

pub(crate) fn category_matches_filter(category: &Category, filter: &str) -> bool {
    let filter = filter.trim().to_ascii_lowercase();
    filter.is_empty()
        || category.name.to_ascii_lowercase().contains(&filter)
        || category.file.to_ascii_lowercase().contains(&filter)
        || category.file_stem().to_ascii_lowercase().contains(&filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_support::{categories, category};
    use crate::settings::{CategorySettings, Settings};

    #[test]
    fn finds_category_by_name_file_or_stem() {
        let categories = categories();

        assert_eq!(find_category(&categories, "technology"), Some(1));
        assert_eq!(find_category(&categories, "tech.json"), Some(1));
        assert_eq!(find_category(&categories, "tech"), Some(1));
        assert_eq!(find_category(&categories, "missing"), None);
    }

    #[test]
    fn default_category_config_matches_web_defaults() {
        let categories = vec![
            category("World", "world.json"),
            category("Gaming", "gaming.json"),
            category("Science", "science.json"),
            category("AI", "ai.json"),
            category("Technology", "technology.json"),
            category("Business", "business.json"),
            category("Sports", "sports.json"),
            category("Today in History", "today_in_history.json"),
            category("USA", "usa.json"),
            category("Climate Change", "climate_change.json"),
        ];

        let enabled = default_enabled_categories(&categories);
        let enabled_names = categories
            .iter()
            .zip(enabled)
            .filter_map(|(category, enabled)| enabled.then_some(category.name.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(
            enabled_names,
            vec![
                "World",
                "Gaming",
                "Science",
                "AI",
                "Technology",
                "Business",
                "Sports",
                "Today in History",
                "USA"
            ]
        );
    }

    #[test]
    fn initial_category_outside_defaults_is_enabled() {
        let categories = vec![
            category("World", "world.json"),
            category("Climate Change", "climate_change.json"),
        ];

        let mut enabled = default_enabled_categories(&categories);
        let selected_category =
            select_initial_category(&categories, &mut enabled, Some("Climate Change")).unwrap();

        assert_eq!(selected_category, 1);
        assert_eq!(enabled, vec![true, true]);
    }

    #[test]
    fn default_initial_category_prefers_world_when_visible() {
        let categories = vec![
            category("Technology", "technology.json"),
            category("World", "world.json"),
        ];
        let mut enabled = default_enabled_categories(&categories);

        let selected_category = select_initial_category(&categories, &mut enabled, None).unwrap();

        assert_eq!(selected_category, 1);
    }

    #[test]
    fn default_initial_category_uses_first_enabled_when_world_is_hidden() {
        let categories = vec![
            category("Technology", "technology.json"),
            category("World", "world.json"),
        ];
        let mut enabled = vec![true, false];

        let selected_category = select_initial_category(&categories, &mut enabled, None).unwrap();

        assert_eq!(selected_category, 0);
    }

    #[test]
    fn saved_category_config_loads_matching_categories() {
        let categories = vec![
            category("World", "world.json"),
            category("Technology", "technology.json"),
            category("Today in History", "today_in_history.json"),
        ];
        let settings = Settings {
            categories: CategorySettings {
                enabled: vec!["technology".to_owned(), "todayinhistory".to_owned()],
            },
            ..Settings::default()
        };

        let enabled = enabled_categories_from_settings(&categories, &settings).unwrap();

        assert_eq!(enabled, vec![false, true, true]);
    }

    #[test]
    fn empty_saved_category_config_falls_back_to_defaults() {
        let categories = vec![category("World", "world.json")];
        let settings = Settings::default();

        assert!(enabled_categories_from_settings(&categories, &settings).is_none());
    }
}
