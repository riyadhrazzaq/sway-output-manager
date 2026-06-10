//! Sway output discovery and layout application.

use std::fmt;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// Arrangement choices exposed in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arrangement {
    Mirror,
    LeftOf,
    RightOf,
    Above,
    Below,
}

impl Arrangement {
    /// Map a shortcut key to an arrangement.
    pub fn from_shortcut(shortcut: char) -> Option<Self> {
        match shortcut {
            'h' | 'H' => Some(Self::LeftOf),
            'l' | 'L' => Some(Self::RightOf),
            'k' | 'K' => Some(Self::Above),
            'j' | 'J' => Some(Self::Below),
            _ => None,
        }
    }

    /// Convert unsupported legacy choices to the closest supported layout.
    pub fn normalize(self) -> Self {
        match self {
            Self::Mirror => Self::RightOf,
            other => other,
        }
    }

    /// Human-friendly label for the arrangement.
    pub fn label(self) -> &'static str {
        match self {
            Self::Mirror => "mirror (not supported by sway)",
            Self::LeftOf => "left of",
            Self::RightOf => "right of",
            Self::Above => "above",
            Self::Below => "below",
        }
    }

    /// Shortcut hint for rendering the UI.
    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Mirror => "m",
            Self::LeftOf => "h",
            Self::RightOf => "l",
            Self::Above => "k",
            Self::Below => "j",
        }
    }
}

impl fmt::Display for Arrangement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Workspace information returned by `swaymsg -t get_workspaces`.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceInfo {
    #[serde(default)]
    pub num: i64,
    pub name: String,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub focused: bool,
}

/// Output geometry as reported by Sway.
#[derive(Debug, Clone, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The currently selected output mode reported by Sway.
#[derive(Debug, Clone, Deserialize)]
pub struct OutputMode {
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub refresh: f64,
}

/// Output information returned by `swaymsg -t get_outputs`.
#[derive(Debug, Clone, Deserialize)]
pub struct OutputInfo {
    pub name: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub focused: bool,
    pub rect: Rect,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default = "default_transform")]
    pub transform: String,
    #[serde(default)]
    pub current_mode: Option<OutputMode>,
}

impl OutputInfo {
    /// Compact one-line description for the output list.
    pub fn summary(&self) -> String {
        let mode = self.current_mode.as_ref().map_or_else(
            || String::from("unknown mode"),
            |mode| {
                if mode.refresh > 0.0 {
                    format!("{}x{} @ {:.2}Hz", mode.width, mode.height, mode.refresh)
                } else {
                    format!("{}x{}", mode.width, mode.height)
                }
            },
        );
        format!(
            "{} [{}] {}x{} at ({}, {}) {} scale {:.2}",
            self.name,
            if self.active { "active" } else { "inactive" },
            self.rect.width,
            self.rect.height,
            self.rect.x,
            self.rect.y,
            mode,
            self.scale
        )
    }
}

/// Query outputs from the running Sway session.
pub fn fetch_outputs() -> Result<Vec<OutputInfo>> {
    let output = swaymsg_json("-t", "get_outputs")?;

    if !output.status.success() {
        return Err(anyhow!(
            "swaymsg returned a failure while fetching outputs: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let outputs: Vec<OutputInfo> =
        serde_json::from_slice(&output.stdout).context("failed to parse sway output JSON")?;
    Ok(outputs)
}

/// Query workspaces from the running Sway session.
pub fn fetch_workspaces() -> Result<Vec<WorkspaceInfo>> {
    let output = swaymsg_json("-t", "get_workspaces")?;

    if !output.status.success() {
        return Err(anyhow!(
            "swaymsg returned a failure while fetching workspaces: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let workspaces: Vec<WorkspaceInfo> =
        serde_json::from_slice(&output.stdout).context("failed to parse sway workspace JSON")?;
    Ok(workspaces)
}

/// Compute the new top-left position for a target output.
pub fn position_for(action: Arrangement, target: &OutputInfo, anchor: &OutputInfo) -> (i32, i32) {
    match action {
        Arrangement::Mirror => (anchor.rect.x, anchor.rect.y),
        Arrangement::LeftOf => (anchor.rect.x - target.rect.width, anchor.rect.y),
        Arrangement::RightOf => (anchor.rect.x + anchor.rect.width, anchor.rect.y),
        Arrangement::Above => (anchor.rect.x, anchor.rect.y - target.rect.height),
        Arrangement::Below => (anchor.rect.x, anchor.rect.y + anchor.rect.height),
    }
}

/// Apply a layout change by issuing a `swaymsg` position command.
pub fn apply_arrangement(
    target: &OutputInfo,
    anchor: &OutputInfo,
    action: Arrangement,
) -> Result<()> {
    let action = action.normalize();

    let (x, y) = position_for(action, target, anchor);
    let command = format!(
        "output \"{}\" position {} {}",
        escape_sway_identifier(&target.name),
        x,
        y
    );

    run_swaymsg(command, "applying an arrangement")
}

/// Move a workspace to the selected output.
pub fn move_workspace_to_output(workspace: &WorkspaceInfo, target: &OutputInfo) -> Result<()> {
    let command = workspace_move_command(&workspace.name, &target.name);

    run_swaymsg(
        command,
        &format!("moving workspace {} to {}", workspace.name, target.name),
    )
}

fn default_scale() -> f64 {
    1.0
}

fn default_transform() -> String {
    String::from("normal")
}

fn workspace_move_command(workspace_name: &str, output_name: &str) -> String {
    format!(
        "workspace \"{}\"; move workspace to output \"{}\"",
        escape_sway_identifier(workspace_name),
        escape_sway_identifier(output_name)
    )
}

fn escape_sway_identifier(identifier: &str) -> String {
    identifier.replace('"', "\\\"")
}

fn run_swaymsg(command: String, context: &str) -> Result<()> {
    let output = Command::new("swaymsg")
        .arg(command)
        .output()
        .context("failed to run swaymsg")?;

    if !output.status.success() {
        return Err(anyhow!(
            "swaymsg returned a failure while {}: {}",
            context,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn swaymsg_json(flag: &str, command: &str) -> Result<std::process::Output> {
    Command::new("swaymsg")
        .args([flag, command])
        .output()
        .context("failed to run swaymsg")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(name: &str, x: i32, y: i32, width: i32, height: i32) -> OutputInfo {
        OutputInfo {
            name: name.to_string(),
            active: true,
            focused: false,
            rect: Rect {
                x,
                y,
                width,
                height,
            },
            scale: 1.0,
            transform: String::from("normal"),
            current_mode: Some(OutputMode {
                width,
                height,
                refresh: 60.0,
            }),
        }
    }

    #[test]
    fn computes_positions_relative_to_anchor() {
        let target = output("DP-1", 100, 100, 1920, 1080);
        let anchor = output("HDMI-A-1", 0, 0, 2560, 1440);

        assert_eq!(
            position_for(Arrangement::LeftOf, &target, &anchor),
            (-1920, 0)
        );
        assert_eq!(
            position_for(Arrangement::RightOf, &target, &anchor),
            (2560, 0)
        );
        assert_eq!(
            position_for(Arrangement::Above, &target, &anchor),
            (0, -1080)
        );
        assert_eq!(
            position_for(Arrangement::Below, &target, &anchor),
            (0, 1440)
        );
        assert_eq!(position_for(Arrangement::Mirror, &target, &anchor), (0, 0));
    }

    #[test]
    fn normalizes_legacy_mirror_to_supported_layout() {
        assert_eq!(Arrangement::Mirror.normalize(), Arrangement::RightOf);
        assert_eq!(Arrangement::LeftOf.normalize(), Arrangement::LeftOf);
    }

    #[test]
    fn builds_workspace_move_command() {
        assert_eq!(
            workspace_move_command("1: dev", "HDMI-A-1"),
            "workspace \"1: dev\"; move workspace to output \"HDMI-A-1\""
        );
    }
}
