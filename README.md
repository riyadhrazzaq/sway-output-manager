# SwayWM Output Manager

A small TUI app for managing Sway outputs with Rust and Ratatui.

## MVP Plan

The MVP focuses on one workflow: inspect all connected outputs, choose a preferred arrangement for any selected output, and apply it through `swaymsg`.

### 1. Output discovery
- Query Sway for current outputs via `swaymsg -t get_outputs`.
- Parse every active output, including names, scale, resolution, position, transform, and current layout state.
- Present all connected outputs in the TUI, regardless of count.

### 2. Interactive layout selection
- Let the user move focus through the full output list.
- Support selecting any output, including when three or more outputs are connected.
- Provide a small set of arrangement actions for the selected output:
	- left of
	- right of
	- above
	- below
- Base placement decisions on the currently selected target output so the app can handle different monitor counts and topologies.
- Confirm the chosen action before applying it.

### 2b. Workspace transfer
- Move the focused workspace to the currently selected output.
- Use the selected output as the destination monitor, matching the same selection flow as the output arrangement actions.

### 3. Apply configuration
- Translate the chosen action into the corresponding `swaymsg` command.
- Execute the command and surface success or failure in the UI.
- Refresh the output list after applying changes.
- Recompute the affected topology so later selections stay consistent after an output is added, removed, or moved.

### 4. Remember the last choice
- Store the last applied arrangement per output in a local config file.
- Load the saved preference on startup and show it as the default selection.
- Keep preferences keyed by output name so new or extra outputs can be handled independently.

### 5. Basic UX and reliability
- Add keyboard navigation and a quit key.
- Show error messages for parse failures and command failures.
- Include module-level and function-level documentation for the core logic.

## MVP Acceptance Criteria

- The app starts in a terminal and shows the connected Sway outputs.
- The user can choose an arrangement for an output and apply it.
- The app uses `swaymsg` for all Sway interactions.
- The last selected arrangement is restored on the next launch.
- The code is documented enough to understand the control flow and main data types.

## Installation

### Prerequisites

- Rust toolchain with `cargo`
- Sway session running on Wayland
- `swaymsg` available on `PATH`

### Build from source

```bash
git clone <repo-url>
cd swaywm-output-manager
cargo build --release
```

### Run

```bash
cargo run --release
```

### Optional install step

If you want the binary on your `PATH`, copy the release build somewhere like `~/.local/bin`:

```bash
install -Dm755 target/release/swaywm-output-manager ~/.local/bin/swaywm-output-manager
```

## Supported Features

- [x] Manage how new monitor is handled (left, right, up, bottom)
- [x] Remembers the last preference for monitor
- [ ] Interactively select the preferred orientation
- [x] Support any number of connected outputs and update their topology correctly
- [x] Move the focused workspace to a selected output

# Note
entirely vibe-coded.