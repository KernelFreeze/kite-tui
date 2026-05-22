# Kite

Kite is a terminal viewer for [Kagi News](https://news.kagi.com/). It reads the
public Kagi News category index, fetches category RSS feeds, and renders the
latest summarized stories in a Ratatui interface.

## Run

```bash
cargo run
```

Useful options:

```bash
cargo run -- --category Technology
cargo run -- --category tech
cargo run -- --base-url https://news.kagi.com/
```

## Controls

- `Tab`: switch panes
- `j` / `k` or arrow keys: move selection
- `gg` / `G`: jump to the first or last article, or to the top or bottom of an open article
- `/`: filter categories
- `,`: open settings
- `?`: open help
- `Enter`: load the selected category, open an article, or return to the article list
- `Esc`: return from an article, accept an active category filter, or clear an existing category filter
- `Backspace`: edit an active category filter
- In settings: `Tab` switches sections, `Space` toggles categories, `/` searches categories, `Enter` edits a keybind, and `d` restores defaults for the current section
- `r`: refresh the selected category
- `q`: quit

## Settings

Kite stores category visibility and keybinds in a TOML settings file under the
platform configuration directory reported by the `directories` crate.

Keybinds can also be customized in the same file:

```toml
[keybinds]
help = "?"
settings = ","
category_filter = "/"
refresh = "r"
quit = "q"
reset_defaults = "d"
jump_top = "gg"
jump_bottom = "G"
```

## RSS Parser

Kite uses `feed-rs` instead of a narrower RSS-only parser. Kagi currently
publishes RSS feeds, but `feed-rs` gives the app one model for RSS, Atom, and
JSON Feed, which leaves room for feed format variance without changing the app
model.
