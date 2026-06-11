//! Ratatui rendering for the output manager.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::App;
use crate::sway::Arrangement;

/// Render the complete UI.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(8),
        ])
        .split(area);

    draw_header(frame, layout[0], app);
    draw_body(frame, layout[1], app);
    draw_footer(frame, layout[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let selected_workspace = app.workspaces.get(app.selected_workspace);
    let selected_output = app.outputs.get(app.selected_output);
    let anchor_output = app
        .effective_anchor_output()
        .or_else(|| app.outputs.get(app.anchor_output));

    let selected_workspace_name = selected_workspace
        .map(|workspace| workspace.name.as_str())
        .unwrap_or("none");
    let selected_output_name = selected_output
        .map(|output| output.name.as_str())
        .unwrap_or("none");
    let anchor_output_name = anchor_output
        .map(|output| output.name.as_str())
        .unwrap_or("none");

    let text = vec![
        Line::from(vec![
            Span::styled("Workspace: ", Style::default().fg(Color::Magenta)),
            Span::raw(selected_workspace_name),
            Span::raw("    "),
            Span::styled("Output: ", Style::default().fg(Color::Cyan)),
            Span::raw(selected_output_name),
            Span::raw("    "),
            Span::styled("Anchor: ", Style::default().fg(Color::Blue)),
            Span::raw(anchor_output_name),
        ]),
        Line::from(vec![
            Span::styled("Default action: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.preference_label()),
            Span::raw("    "),
            Span::styled("Status: ", Style::default().fg(Color::Green)),
            Span::raw(app.status.as_str()),
        ]),
    ];

    let header = Paragraph::new(text)
        .block(
            Block::default()
                .title("SwayWM Output Manager")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(header, area);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(30),
        ])
        .split(area);

    let workspace_items = app
        .workspaces
        .iter()
        .enumerate()
        .map(|(index, workspace)| {
            let marker = if index == app.selected_workspace {
                "▶"
            } else {
                " "
            };
            let focused_marker = if workspace.focused { "*" } else { " " };
            let label = if workspace.num > 0 {
                format!("{}: {}", workspace.num, workspace.name)
            } else {
                workspace.name.clone()
            };
            let label = if workspace.output.is_empty() {
                label
            } else {
                format!("{} [{}]", label, workspace.output)
            };

            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::raw(" "),
                Span::styled(focused_marker, Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::raw(label),
            ]))
        })
        .collect::<Vec<_>>();

    let workspaces = List::new(workspace_items).block(
        Block::default()
            .title(format!("Workspaces [{}]", app.active_list_name()))
            .borders(Borders::ALL),
    );
    frame.render_widget(workspaces, columns[0]);

    let output_items = app
        .outputs
        .iter()
        .enumerate()
        .map(|(index, output)| {
            let marker = if index == app.selected_output {
                "▶"
            } else {
                " "
            };
            let anchor_marker = if index == app.anchor_output { "*" } else { " " };

            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(anchor_marker, Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::raw(output.summary()),
            ]))
        })
        .collect::<Vec<_>>();

    let outputs = List::new(output_items).block(
        Block::default()
            .title(format!("Outputs [{}]", app.active_list_name()))
            .borders(Borders::ALL),
    );
    frame.render_widget(outputs, columns[1]);

    let details = render_details(app);
    frame.render_widget(details, columns[2]);

    if app.is_move_target_picker_open() {
        render_move_target_picker(frame, area, app);
    }
}

fn render_move_target_picker(frame: &mut Frame, area: Rect, app: &App) {
    if app.outputs.is_empty() {
        return;
    }

    let popup_area = centered_rect(60, 52, area);
    frame.render_widget(Clear, popup_area);

    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(popup_area);

    let header = Paragraph::new("Select an output for the selected workspace")
        .block(Block::default().borders(Borders::ALL).title("Move workspace"))
        .wrap(Wrap { trim: true });
    frame.render_widget(header, popup[0]);

    let items = app
        .outputs
        .iter()
        .enumerate()
        .map(|(index, output)| {
            let marker = if Some(index) == app.move_target_output_index() {
                "▶"
            } else {
                " "
            };

            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::raw(output.summary()),
            ]))
        })
        .collect::<Vec<_>>();

    let picker = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Output")
            .border_style(Style::default().fg(Color::Yellow)),
    );

    let mut state = ListState::default();
    state.select(app.move_target_output_index());
    frame.render_stateful_widget(picker, popup[1], &mut state);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn render_details(app: &App) -> Paragraph<'_> {
    let selected_workspace = app.workspaces.get(app.selected_workspace);
    let selected_output = app.outputs.get(app.selected_output);
    let anchor_output = app
        .effective_anchor_output()
        .or_else(|| app.outputs.get(app.anchor_output));

    let mut lines = Vec::new();

    if let Some(workspace) = selected_workspace {
        lines.push(Line::from(format!(
            "Selected workspace: {}",
            workspace.name
        )));
        lines.push(Line::from(format!(
            "Workspace output: {}",
            workspace.output
        )));
        lines.push(Line::from(format!("Focused: {}", workspace.focused)));
    } else {
        lines.push(Line::from("No workspace selected"));
    }

    lines.push(Line::from(""));

    if let Some(output) = selected_output {
        lines.push(Line::from(format!("Selected output: {}", output.name)));
        lines.push(Line::from(format!(
            "Position: ({}, {})",
            output.rect.x, output.rect.y
        )));
        lines.push(Line::from(format!(
            "Size: {}x{}",
            output.rect.width, output.rect.height
        )));
        lines.push(Line::from(format!("Focused: {}", output.focused)));
        lines.push(Line::from(format!("Scale: {:.2}", output.scale)));
        lines.push(Line::from(format!("Transform: {}", output.transform)));
    } else {
        lines.push(Line::from("No output selected"));
    }

    lines.push(Line::from(""));

    if let Some(anchor) = anchor_output {
        lines.push(Line::from(format!("Anchor output: {}", anchor.name)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Shortcuts:"));
    lines.push(Line::from(format!(
        "  Left/Right  focus workspaces or outputs (current: {})",
        app.active_list_name()
    )));
    lines.push(Line::from("  Up/Down    move selection in active list"));
    lines.push(Line::from("  Tab        change anchor output"));
    lines.push(Line::from(format!(
        "  {}          {}",
        Arrangement::LeftOf.shortcut(),
        Arrangement::LeftOf.label()
    )));
    lines.push(Line::from(format!(
        "  {}          {}",
        Arrangement::RightOf.shortcut(),
        Arrangement::RightOf.label()
    )));
    lines.push(Line::from(format!(
        "  {}          {}",
        Arrangement::Above.shortcut(),
        Arrangement::Above.label()
    )));
    lines.push(Line::from(format!(
        "  {}          {}",
        Arrangement::Below.shortcut(),
        Arrangement::Below.label()
    )));
    lines.push(Line::from("  Enter      on workspace: choose move target"));
    lines.push(Line::from("              on output: apply saved action"));
    lines.push(Line::from("  Esc        cancel move picker / quit"));
    lines.push(Line::from("  q          quit"));

    Paragraph::new(lines)
        .block(Block::default().title("Details").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let footer = Paragraph::new(app.status.as_str())
        .block(Block::default().title("Message").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    frame.render_widget(footer, area);
}
