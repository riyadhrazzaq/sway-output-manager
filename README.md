# SwayWM Output Manager

SwayWM Output Manager is a terminal UI for arranging Sway outputs and moving the focused workspace between monitors.

It queries the running Sway session with `swaymsg`, shows the available outputs, lets you pick an arrangement relative to an anchor output, and saves the last choice per display.

## Features

- View connected Sway outputs in a compact terminal UI
- Place the selected output left, right, above, or below another output
- Move the focused workspace to a chosen output through a small picker
- Restore the last saved arrangement for each output
- Store preferences in a local JSON config file

## Controls

- `Up` / `Down`: move through the active list
- `Left` / `Right`: switch between workspaces and outputs
- `Tab` / `Shift+Tab`: cycle the anchor output
- `h` / `l` / `k` / `j`: apply left, right, above, or below
- `Enter` on the workspace list: open the output picker for moving the focused workspace
- `Enter` on the output list: apply the saved action
- `Esc`: cancel the picker or quit when no picker is open

## Requirements

- Rust toolchain with `cargo`
- A running Sway session on Wayland
- `swaymsg` available on `PATH`

## Build and Run

```bash
cargo build --release
cargo run --release
```

If you want to install the binary somewhere on your `PATH`, copy it after building:

```bash
install -Dm755 target/release/swaywm-output-manager ~/.local/bin/swaywm-output-manager
```

## Configuration

Saved preferences live at:

```text
~/.config/swaywm-output-manager/config.json
```

The file is created automatically the first time you save a preference.

# AI Warning
Vibe coded with Copilot.
