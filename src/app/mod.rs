//! Application state and the terminal event loop.
//!
//! Split into focused submodules: [`keybindings`] (configurable bindings and
//! key sequence parsing), [`theme_select`] (theme mode resolution),
//! [`categories`] (category lookup and defaults), [`state`] (the [`AppState`]
//! data model), and [`events`] (the run loop and input handling). The
//! re-exports below keep the public surface flat at `crate::app::…`.

mod categories;
mod events;
mod keybindings;
mod state;
mod theme_select;

#[cfg(test)]
mod test_support;

pub use events::run;
pub use keybindings::{KeyBindingAction, KeyBindings};
pub use state::{AppState, Focus, SettingsSection};
pub use theme_select::ThemeSelectionMode;
