# Kite TUI
<img width="1080" height="634" alt="image" src="https://github.com/user-attachments/assets/cd2421e0-ddad-4814-8564-03a27d18c174" />

Kite TUI is a terminal reader for [Kagi News](https://news.kagi.com/). It loads the
public category index, fetches category RSS feeds, and lets you browse the
latest Kagi summaries without leaving the terminal.

## Install

Kite TUI is published on crates.io as `kite-tui`:

```bash
cargo install kite-tui
```

Prebuilt binaries are also available from the
[latest GitHub release](https://github.com/KernelFreeze/kite-tui/releases/latest).

Install on macOS or Linux with the shell installer:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/KernelFreeze/kite-tui/releases/latest/download/kite-tui-installer.sh | sh
```

Install on Windows with the PowerShell installer:

```powershell
irm https://github.com/KernelFreeze/kite-tui/releases/latest/download/kite-tui-installer.ps1 | iex
```

You can also download a platform archive from the latest release, extract it,
and put the `kite-tui` binary somewhere on your `PATH`. On Windows, the binary
is `kite-tui.exe`.

After installing, run it with:

```bash
kite-tui
```

## Run from source

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

### Configuration location

The `settings.toml` file (and the `themes` directory) live in the platform
configuration directory:

| Platform | Path |
| -------- | ---- |
| Linux    | `$XDG_CONFIG_HOME/kite/settings.toml` or `~/.config/kite/settings.toml` |
| macOS    | `~/Library/Application Support/dev.CelesteLove.Kite/settings.toml` |
| Windows  | `%APPDATA%\CelesteLove\Kite\config\settings.toml` (e.g. `C:\Users\<You>\AppData\Roaming\CelesteLove\Kite\config\settings.toml`) |

Themes and keybinds can also be customized in the same file:

```toml
theme = "ansi"

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

To follow the platform color scheme, replace the fixed `theme` value with a
theme table:

```toml
[theme]
mode = "device"
light = "catppuccin-latte"
dark = "catppuccin-mocha"
unspecified = "ansi"
```

`mode` can be `device`, `light`, or `dark`. A fixed `theme = "ansi"` value uses
one theme for every color scheme.

Built-in themes are `ansi`, `catppuccin-mocha`, `catppuccin-latte`,
`catppuccin-frappe`, `catppuccin-macchiato`, `dracula`, `gruvbox-dark`,
`gruvbox-light`, `tokyo-night`, `tokyo-night-storm`, `tokyo-night-moon`,
`rose-pine`, `rose-pine-moon`, `rose-pine-dawn`, `nord`, `solarized-dark`,
`solarized-light`, `everforest-dark-hard`, `everforest-light-medium`,
`kanagawa-wave`, `kanagawa-dragon`, `atom-one-dark`, `atom-one-light`,
`monokai-pro`, `monokai-remastered`, `ayu-dark`, `ayu-mirage`, `ayu-light`,
`github-dark`, `github-dark-dimmed`, `github-light`, `nightfox`, `dayfox`,
`duskfox`, `dawnfox`, `flexoki-dark`, `flexoki-light`, `material-dark`,
`sonokai`, and `synthwave`.

Custom themes are TOML files in the `themes` directory next to
`settings.toml`. The file stem is the theme id, so
`themes/my-theme.toml` can be selected with `theme = "my-theme"` or used in a
theme table:

```toml
name = "My Theme"

[colors]
text = "#cdd6f4"
muted = "darkgray"
subtle = "gray"
title = "#89dceb"
accent = "#f9e2af"
success = "#a6e3a1"
selected_fg = "black"
selected_bg = "#a6e3a1"
settings_selected_bg = "#89dceb"
editing_bg = "#f9e2af"
link = "#89dceb"
focus = "#89dceb"
border = "darkgray"
status = "magenta"
```

Colors can use ANSI names such as `cyan`, `darkgray`, and `light-red`, RGB hex
strings such as `#cba6f7`, or indexed terminal colors such as `indexed:42`.
Omitted colors inherit from `ansi`.

## License

Kite TUI is distributed under the MIT License. See [LICENSE](LICENSE) for the full terms.
