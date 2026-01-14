# cc-sakura-line

A minimal statusline for Claude Code — written in Rust, designed to bloom quietly in your terminal.

![preview](./docs/imgs/preview.png)

**Sakura** (桜) are Japanese cherry blossoms: beautiful, fleeting, and understated. This statusline aims for the same feeling — present, but never loud.

## Layout

Two rows, four segments each:

**Row 1**

- Model
- Branch
- Git changes (`+n -m`)
- Local time (`HH:MM:SS`)

**Row 2**

- Claude Code version
- Context used (`used/total`)
- Context remaining (`85% left`)
- Session duration (`<1m`, `32m`, `5h32m`)

## Build

```
cargo build --release
```

## Preview (TUI)

```sh
cargo run -- --preview
```

Press `q` or `Esc` to exit.

## Claude Code settings

Add to `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "/absolute/path/to/cc-sakura-line",
    "padding": 0
  }
}
```

## Width behavior

Claude Code can show system notices on the right side. By default, **cc-sakura-line does not fill full width** to avoid collisions.

- Force full width:
  ```
  /path/to/cc-sakura-line --fill
  ```
- Reserve right-side space:
  ```
  /path/to/cc-sakura-line --reserved=24
  ```
- Fix a width (useful when width cannot be detected):
  ```
  /path/to/cc-sakura-line --width=120
  ```

## CLI flags

- `--preview` / `-p`: TUI preview
- `--fill`: fill full width
- `--no-fill`: do not fill full width (default)
- `--width=NUM`: override detected width
- `--reserved=NUM`: keep space for right-side system notices

## Data sources (JSON from Claude Code)

This tool reads from stdin:

- `model.display_name` (or `model.id`)
- `version`
- `context_window.context_window_size`
- `context_window.current_usage.*`
- `cost.total_duration_ms`

Git info is read from the current repository via `git`.

## Optional env overrides

- `CC_MODEL`: model name
- `CC_VERSION`: version label
- `CC_CONTEXT_LABEL`: context text (overrides used/total display)
- `CC_CONTEXT_USED`: used context (number)
- `CC_CONTEXT_TOTAL`: total context (number)
- `CC_CONTEXT_REMAINING`: remaining context text (overrides computed percent)
- `CC_STATUSLINE_WIDTH`: width override (same as `--width`)
- `CC_STATUSLINE_RESERVED`: reserved right-space (same as `--reserved`)
- `CC_STATUSLINE_FILL`: `1` to fill full width (same as `--fill`)

## Fonts

The rounded ends use Powerline glyphs (`` ``). A Nerd Font (or Powerline-compatible font) is recommended.
