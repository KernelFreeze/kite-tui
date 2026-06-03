//! Theme selection: the user-facing mode (Device/Light/Dark) and the resolution
//! of a concrete theme id from the stored settings and the platform color
//! scheme.

use crate::settings::{ThemeMode, ThemeSettings, ThemeVariantSettings};
use crate::theme::PlatformColorScheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSelectionMode {
    Device,
    Light,
    Dark,
}

impl ThemeSelectionMode {
    pub const ALL: [Self; 3] = [Self::Device, Self::Light, Self::Dark];

    pub fn title(self) -> &'static str {
        match self {
            Self::Device => "Device",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub(crate) fn from_settings(settings: &ThemeSettings) -> Self {
        match settings {
            ThemeSettings::Fixed(_) => Self::Device,
            ThemeSettings::Variants(variants) => Self::from_theme_mode(variants.mode),
        }
    }

    pub(crate) fn from_theme_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Device => Self::Device,
            ThemeMode::Light => Self::Light,
            ThemeMode::Dark => Self::Dark,
        }
    }

    pub(crate) fn theme_mode(self) -> ThemeMode {
        match self {
            Self::Device => ThemeMode::Device,
            Self::Light => ThemeMode::Light,
            Self::Dark => ThemeMode::Dark,
        }
    }

    pub(crate) fn move_by(self, step: isize) -> Self {
        let current = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0) as isize;
        let next = (current + step).rem_euclid(Self::ALL.len() as isize);
        Self::ALL[next as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeSlot {
    Light,
    Dark,
    Unspecified,
}

pub(crate) fn theme_id_for_settings(
    settings: &ThemeSettings,
    platform_color_scheme: PlatformColorScheme,
) -> &str {
    match settings {
        ThemeSettings::Fixed(theme) => theme,
        ThemeSettings::Variants(variants) => theme_id_for_variants(
            variants,
            ThemeSelectionMode::from_theme_mode(variants.mode),
            platform_color_scheme,
        ),
    }
}

pub(crate) fn theme_id_for_variants(
    variants: &ThemeVariantSettings,
    mode: ThemeSelectionMode,
    platform_color_scheme: PlatformColorScheme,
) -> &str {
    match theme_slot_for_selection(mode, platform_color_scheme) {
        ThemeSlot::Light => &variants.light,
        ThemeSlot::Dark => &variants.dark,
        ThemeSlot::Unspecified => &variants.unspecified,
    }
}

pub(crate) fn theme_slot_for_mode(
    mode: ThemeMode,
    platform_color_scheme: PlatformColorScheme,
) -> ThemeSlot {
    theme_slot_for_selection(
        ThemeSelectionMode::from_theme_mode(mode),
        platform_color_scheme,
    )
}

pub(crate) fn theme_slot_for_selection(
    mode: ThemeSelectionMode,
    platform_color_scheme: PlatformColorScheme,
) -> ThemeSlot {
    match mode {
        ThemeSelectionMode::Light => ThemeSlot::Light,
        ThemeSelectionMode::Dark => ThemeSlot::Dark,
        ThemeSelectionMode::Device => match platform_color_scheme {
            PlatformColorScheme::Light => ThemeSlot::Light,
            PlatformColorScheme::Dark => ThemeSlot::Dark,
            PlatformColorScheme::Unspecified => ThemeSlot::Unspecified,
        },
    }
}
