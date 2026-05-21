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
- `Enter`: load the selected category, open an article story, or close the story popup
- `Esc`: close the story popup
- `r`: refresh the selected category
- `q`: quit

## RSS Parser

Kite uses `feed-rs` instead of a narrower RSS-only parser. Kagi currently
publishes RSS feeds, but `feed-rs` gives the app one model for RSS, Atom, and
JSON Feed, which leaves room for feed format variance without changing the app
model.
