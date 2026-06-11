//! Application state and event loop.

use std::io;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::{AppConfig, OutputPreference};
use crate::sway::{self, Arrangement, OutputInfo, WorkspaceInfo};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListFocus {
    Workspaces,
    Outputs,
}

/// Top-level application state.
pub struct App {
    pub workspaces: Vec<WorkspaceInfo>,
    pub outputs: Vec<OutputInfo>,
    pub selected_workspace: usize,
    pub selected_output: usize,
    pub anchor_output: usize,
    pub selected_action: Arrangement,
    list_focus: ListFocus,
    move_target_output: Option<usize>,
    pub status: String,
    pub(crate) config: AppConfig,
    should_quit: bool,
}

impl App {
    /// Load configuration and fetch the current workspaces and outputs.
    pub fn new() -> Result<Self> {
        let config = AppConfig::load().unwrap_or_default();
        let workspaces = sway::fetch_workspaces().context("failed to load workspaces from Sway")?;
        let outputs = sway::fetch_outputs().context("failed to load outputs from Sway")?;

        let mut app = Self {
            workspaces,
            outputs,
            selected_workspace: 0,
            selected_output: 0,
            anchor_output: 0,
            selected_action: Arrangement::RightOf,
            list_focus: ListFocus::Workspaces,
            move_target_output: None,
            status: String::from("ready"),
            config,
            should_quit: false,
        };

        app.restore_selection_from_config();
        Ok(app)
    }

    /// Run the TUI loop until the user quits.
    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| ui::draw(frame, self))?;

            if event::poll(Duration::from_millis(150))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code)?;
                    }
                }
            }
        }

        self.config.save().context("failed to save config")?;
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) -> Result<()> {
        if self.move_target_output.is_some() {
            return self.handle_move_target_key(code);
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Left | KeyCode::Right => self.toggle_list_focus(),
            KeyCode::Up | KeyCode::Down => self.navigate_active_list(code),
            KeyCode::Tab => self.next_anchor(),
            KeyCode::BackTab => self.previous_anchor(),
            KeyCode::Enter => {
                if matches!(self.list_focus, ListFocus::Workspaces) {
                    self.open_move_target_picker()?;
                } else {
                    self.apply_selected_action()?;
                }
            }
            KeyCode::Char(ch) => {
                if let Some(action) = Arrangement::from_shortcut(ch) {
                    self.apply_action(action)?;
                } else {
                    self.status = format!("unhandled key: {ch}");
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_move_target_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc => self.move_target_output = None,
            KeyCode::Enter => self.move_selected_workspace_to_output_index()?,
            KeyCode::Up => self.previous_move_target_output(),
            KeyCode::Down => self.next_move_target_output(),
            _ => {}
        }

        Ok(())
    }

    fn apply_selected_action(&mut self) -> Result<()> {
        self.apply_action(self.selected_action)
    }

    fn open_move_target_picker(&mut self) -> Result<()> {
        if self.outputs.is_empty() {
            self.status = String::from("no outputs are available");
            return Ok(());
        }

        self.move_target_output = Some(self.selected_output.min(self.outputs.len() - 1));
        self.status = String::from("select an output, then press Enter to move the workspace");
        Ok(())
    }

    fn move_selected_workspace_to_output_index(&mut self) -> Result<()> {
        let output_index = self
            .move_target_output
            .ok_or_else(|| anyhow!("no move target is selected"))?;

        let workspace = self
            .selected_workspace()
            .ok_or_else(|| anyhow!("no workspaces are available"))?
            .clone();
        let selected_output = self
            .outputs
            .get(output_index)
            .ok_or_else(|| anyhow!("no outputs are available"))?
            .clone();

        sway::move_workspace_to_output(&workspace, &selected_output).with_context(|| {
            format!(
                "failed to move workspace {} to {}",
                workspace.name, selected_output.name
            )
        })?;

        let workspace_name = workspace.name;
        let output_name = selected_output.name;
        self.move_target_output = None;
        self.refresh_state(Some(&workspace_name), Some(&output_name), None)?;
        self.status = format!("moved workspace {} to {}", workspace_name, output_name);

        Ok(())
    }

    fn apply_action(&mut self, action: Arrangement) -> Result<()> {
        let selected = self
            .selected_output()
            .ok_or_else(|| anyhow!("no outputs are available"))?
            .clone();
        let anchor = self
            .effective_anchor_output()
            .ok_or_else(|| anyhow!("no anchor output is available"))?
            .clone();
        let workspace_name = self.current_workspace_name().map(str::to_owned);

        sway::apply_arrangement(&selected, &anchor, action)
            .with_context(|| format!("failed to apply {} to {}", action, selected.name))?;

        self.selected_action = action;
        self.config
            .set_preference(selected.name.clone(), action, Some(anchor.name.clone()));
        self.config.save().context("failed to persist config")?;

        let selected_name = selected.name;
        let anchor_name = anchor.name;
        self.refresh_state(
            workspace_name.as_deref(),
            Some(&selected_name),
            Some(&anchor_name),
        )?;
        self.status = format!(
            "applied {} for {} relative to {}",
            action, selected_name, anchor_name
        );

        Ok(())
    }

    fn refresh_state(
        &mut self,
        selected_workspace_name: Option<&str>,
        selected_output_name: Option<&str>,
        anchor_output_name: Option<&str>,
    ) -> Result<()> {
        self.workspaces = sway::fetch_workspaces().context("failed to refresh workspaces")?;
        self.outputs = sway::fetch_outputs().context("failed to refresh outputs")?;

        if self.workspaces.is_empty() && self.outputs.is_empty() {
            self.selected_workspace = 0;
            self.selected_output = 0;
            self.anchor_output = 0;
            self.status = String::from("no workspaces or outputs reported by sway");
            return Ok(());
        }

        if self.workspaces.is_empty() {
            self.selected_workspace = 0;
        } else {
            self.selected_workspace = selected_workspace_name
                .and_then(|name| self.index_for_workspace_name(name))
                .or_else(|| self.focused_workspace_index())
                .unwrap_or(0);
        }

        if self.outputs.is_empty() {
            self.selected_output = 0;
            self.anchor_output = 0;
            self.status = String::from("no outputs reported by sway");
            return Ok(());
        }

        self.selected_output = selected_output_name
            .and_then(|name| self.index_for_output_name(name))
            .unwrap_or_else(|| self.active_output_index().unwrap_or(0));

        self.anchor_output = anchor_output_name
            .and_then(|name| self.index_for_output_name(name))
            .unwrap_or_else(|| self.default_anchor_index());

        if self.anchor_output == self.selected_output && self.outputs.len() > 1 {
            self.anchor_output = self.default_anchor_index();
        }

        self.sync_selected_workspace_state();
        self.sync_selected_output_state();
        Ok(())
    }

    fn restore_selection_from_config(&mut self) {
        if self.workspaces.is_empty() && self.outputs.is_empty() {
            self.status = String::from("no workspaces or outputs reported by sway");
            return;
        }

        if !self.workspaces.is_empty() {
            self.selected_workspace = self.focused_workspace_index().unwrap_or(0);
        }

        if self.outputs.is_empty() {
            self.status = String::from("no outputs reported by sway");
            return;
        }

        if let Some((output_name, preference)) = self
            .config
            .preferences
            .iter()
            .find(|(name, _)| self.index_for_output_name(name).is_some())
        {
            self.selected_output = self.index_for_output_name(output_name).unwrap_or(0);
            self.selected_action = preference.action.normalize();
            self.anchor_output = preference
                .anchor_output
                .as_deref()
                .and_then(|name| self.index_for_output_name(name))
                .unwrap_or_else(|| self.default_anchor_index());

            if self.anchor_output == self.selected_output && self.outputs.len() > 1 {
                self.anchor_output = self.default_anchor_index();
            }

            self.status = format!("restored saved preference for {}", output_name);
            return;
        }

        self.selected_output = self.active_output_index().unwrap_or(0);
        self.anchor_output = self.default_anchor_index();
        self.sync_selected_output_state();
    }

    fn sync_selected_workspace_state(&mut self) {
        if self.workspaces.is_empty() {
            return;
        }

        if self.selected_workspace >= self.workspaces.len() {
            self.selected_workspace = self.focused_workspace_index().unwrap_or(0);
        }
    }

    fn sync_selected_output_state(&mut self) {
        if self.outputs.is_empty() {
            return;
        }

        if let Some(selected) = self.selected_output() {
            if let Some(preference) = self.config.preference_for(&selected.name).cloned() {
                self.selected_action = preference.action.normalize();

                if let Some(anchor_index) = preference
                    .anchor_output
                    .as_deref()
                    .and_then(|name| self.index_for_output_name(name))
                {
                    self.anchor_output = anchor_index;
                }
            } else {
                self.selected_action = Arrangement::RightOf;
            }
        }

        if self.anchor_output == self.selected_output && self.outputs.len() > 1 {
            self.anchor_output = self.default_anchor_index();
        }
    }

    fn focused_workspace_index(&self) -> Option<usize> {
        self.workspaces
            .iter()
            .position(|workspace| workspace.focused)
    }

    fn active_output_index(&self) -> Option<usize> {
        self.outputs.iter().position(|output| output.active)
    }

    fn default_anchor_index(&self) -> usize {
        if self.outputs.len() <= 1 {
            0
        } else {
            (self.selected_output + 1) % self.outputs.len()
        }
    }

    pub(crate) fn effective_anchor_output(&self) -> Option<&OutputInfo> {
        if self.outputs.is_empty() {
            return None;
        }

        let anchor_index = if self.anchor_output >= self.outputs.len() {
            self.default_anchor_index()
        } else {
            self.anchor_output
        };

        if self.outputs.len() > 1 && anchor_index == self.selected_output {
            return self
                .outputs
                .get((self.selected_output + 1) % self.outputs.len());
        }

        self.outputs.get(anchor_index)
    }

    fn selected_workspace(&self) -> Option<&WorkspaceInfo> {
        self.workspaces.get(self.selected_workspace)
    }

    fn selected_output(&self) -> Option<&OutputInfo> {
        self.outputs.get(self.selected_output)
    }

    fn current_preference(&self) -> Option<&OutputPreference> {
        self.selected_output()
            .and_then(|output| self.config.preference_for(&output.name))
    }

    fn current_workspace_name(&self) -> Option<&str> {
        self.selected_workspace()
            .map(|workspace| workspace.name.as_str())
    }

    fn index_for_workspace_name(&self, name: &str) -> Option<usize> {
        self.workspaces
            .iter()
            .position(|workspace| workspace.name == name)
    }

    fn index_for_output_name(&self, name: &str) -> Option<usize> {
        self.outputs.iter().position(|output| output.name == name)
    }

    fn toggle_list_focus(&mut self) {
        self.list_focus = match self.list_focus {
            ListFocus::Workspaces => ListFocus::Outputs,
            ListFocus::Outputs => ListFocus::Workspaces,
        };
    }

    fn active_list_label(&self) -> &'static str {
        match self.list_focus {
            ListFocus::Workspaces => "workspaces",
            ListFocus::Outputs => "outputs",
        }
    }

    fn navigate_active_list(&mut self, code: KeyCode) {
        match self.list_focus {
            ListFocus::Workspaces => match code {
                KeyCode::Up => self.previous_workspace(),
                KeyCode::Down => self.next_workspace(),
                _ => {}
            },
            ListFocus::Outputs => match code {
                KeyCode::Up => self.previous_output(),
                KeyCode::Down => self.next_output(),
                _ => {}
            },
        }
    }

    fn next_move_target_output(&mut self) {
        if !self.outputs.is_empty() {
            let current = self.move_target_output.unwrap_or(self.selected_output);
            self.move_target_output = Some((current + 1) % self.outputs.len());
        }
    }

    fn previous_move_target_output(&mut self) {
        if !self.outputs.is_empty() {
            let current = self.move_target_output.unwrap_or(self.selected_output);
            self.move_target_output = Some(if current == 0 {
                self.outputs.len() - 1
            } else {
                current - 1
            });
        }
    }

    fn next_workspace(&mut self) {
        if !self.workspaces.is_empty() {
            self.selected_workspace = (self.selected_workspace + 1) % self.workspaces.len();
        }
    }

    fn previous_workspace(&mut self) {
        if !self.workspaces.is_empty() {
            self.selected_workspace = if self.selected_workspace == 0 {
                self.workspaces.len() - 1
            } else {
                self.selected_workspace - 1
            };
        }
    }

    fn next_output(&mut self) {
        if !self.outputs.is_empty() {
            self.selected_output = (self.selected_output + 1) % self.outputs.len();
            self.anchor_output = self.default_anchor_index();
            self.sync_selected_output_state();
        }
    }

    fn previous_output(&mut self) {
        if !self.outputs.is_empty() {
            self.selected_output = if self.selected_output == 0 {
                self.outputs.len() - 1
            } else {
                self.selected_output - 1
            };
            self.anchor_output = self.default_anchor_index();
            self.sync_selected_output_state();
        }
    }

    fn next_anchor(&mut self) {
        if self.outputs.len() > 1 {
            self.anchor_output = (self.anchor_output + 1) % self.outputs.len();
            if self.anchor_output == self.selected_output {
                self.anchor_output = (self.anchor_output + 1) % self.outputs.len();
            }
        }
    }

    fn previous_anchor(&mut self) {
        if self.outputs.len() > 1 {
            self.anchor_output = if self.anchor_output == 0 {
                self.outputs.len() - 1
            } else {
                self.anchor_output - 1
            };
            if self.anchor_output == self.selected_output {
                self.anchor_output = if self.anchor_output == 0 {
                    self.outputs.len() - 1
                } else {
                    self.anchor_output - 1
                };
            }
        }
    }

    /// Return the remembered action label for the selected output.
    pub(crate) fn preference_label(&self) -> String {
        self.current_preference()
            .map(|pref| pref.action.to_string())
            .unwrap_or_else(|| String::from("none"))
    }

    pub(crate) fn active_list_name(&self) -> &'static str {
        self.active_list_label()
    }

    pub(crate) fn is_move_target_picker_open(&self) -> bool {
        self.move_target_output.is_some()
    }

    pub(crate) fn move_target_output_index(&self) -> Option<usize> {
        self.move_target_output
    }
}
