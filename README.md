# Kite TUI

Kite TUI is a terminal reader for [Kagi News](https://news.kagi.com/). It loads the
public category index, fetches category RSS feeds, and lets you browse the
latest Kagi summaries without leaving the terminal.

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

- `Tab` / `Shift+Tab`: load the next or previous category
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
- `R`: refresh the category index and selected category
- `q`: quit

## Settings

Kite TUI stores category visibility and keybinds in a TOML settings file under
the platform configuration directory.

Keybinds can also be customized in the same file:

```toml
[keybinds]
help = "?"
settings = ","
category_filter = "/"
next_category = "tab"
previous_category = "shift+tab"
refresh = "r"
refresh_all = "R"
quit = "q"
reset_defaults = "d"
jump_top = "gg"
jump_bottom = "G"
```
