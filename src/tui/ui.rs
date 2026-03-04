use std::collections::HashMap;

use chrono::{DateTime, Local, Utc};
use petgraph::visit::{EdgeRef, Topo};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, Tabs},
};

use super::app::{App, DetailViewType, Focus, PopupType};
use super::components::HelpPopup;

/// Format a timestamp (milliseconds since epoch) as a human-readable local time string
fn format_timestamp_ms(timestamp_ms: i64) -> String {
    DateTime::from_timestamp_millis(timestamp_ms)
        .map(|dt: DateTime<Utc>| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| format!("{}ms", timestamp_ms))
}

/// Format bytes into human-readable format (KB, MB, GB)
fn format_bytes(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Help text
            Constraint::Length(3),      // Server URL + user display
            Constraint::Percentage(40), // Workflows table
            Constraint::Length(3),      // Tabs
            Constraint::Min(10),        // Detail table + filter/url input
        ])
        .split(f.area());

    draw_help(f, main_chunks[0], app);
    draw_server_url(f, main_chunks[1], app);
    draw_workflows_table(f, main_chunks[2], app);
    draw_tabs(f, main_chunks[3], app);

    // Split the bottom section for detail table and input widgets
    let needs_input = app.focus == Focus::FilterInput
        || app.focus == Focus::ServerUrlInput
        || app.focus == Focus::WorkflowPathInput;

    let detail_chunks = if needs_input {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .split(main_chunks[4])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5)])
            .split(main_chunks[4])
    };

    draw_detail_table(f, detail_chunks[0], app);

    if app.focus == Focus::FilterInput {
        draw_filter_input(f, detail_chunks[1], app);
    } else if app.focus == Focus::ServerUrlInput {
        draw_server_url_input(f, detail_chunks[1], app);
    } else if app.focus == Focus::WorkflowPathInput {
        draw_workflow_path_input(f, detail_chunks[1], app);
    }

    // Draw popups on top of everything
    if let Some(ref popup) = app.popup {
        match popup {
            PopupType::Help => {
                HelpPopup::render(f, f.area(), "");
            }
            PopupType::Confirmation { dialog, .. } => {
                dialog.render(f, f.area());
            }
            PopupType::JobDetails(details) => {
                details.render(f, f.area());
            }
            PopupType::LogViewer(viewer) => {
                viewer.render(f, f.area());
            }
            PopupType::FileViewer(viewer) => {
                viewer.render(f, f.area());
            }
            PopupType::ProcessViewer(viewer) => {
                viewer.render(f, f.area());
            }
            PopupType::Error(dialog) => {
                dialog.render(f, f.area());
            }
        }
    }
}

fn draw_help(f: &mut Frame, area: Rect, app: &App) {
    let help_text = if app.focus == Focus::FilterInput {
        vec![Line::from(vec![
            Span::styled("Type", Style::default().fg(Color::Yellow)),
            Span::raw(": enter filter | "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(": change column | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": apply | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel"),
        ])]
    } else if app.focus == Focus::ServerUrlInput {
        vec![Line::from(vec![
            Span::styled("Type", Style::default().fg(Color::Yellow)),
            Span::raw(": enter URL | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": connect | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel"),
        ])]
    } else if app.focus == Focus::WorkflowPathInput {
        vec![Line::from(vec![
            Span::styled("Type", Style::default().fg(Color::Yellow)),
            Span::raw(": enter path to workflow spec file | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": create | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel"),
        ])]
    } else if app.focus == Focus::Details && app.detail_view == DetailViewType::Jobs {
        // Job-specific help when in Jobs tab
        vec![Line::from(vec![
            Span::styled("?", Style::default().fg(Color::Yellow)),
            Span::raw(": help | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": details | "),
            Span::styled("l", Style::default().fg(Color::Yellow)),
            Span::raw(": logs | "),
            Span::styled("c", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel | "),
            Span::styled("t", Style::default().fg(Color::Yellow)),
            Span::raw(": terminate | "),
            Span::styled("y", Style::default().fg(Color::Yellow)),
            Span::raw(": retry | "),
            Span::styled("f", Style::default().fg(Color::Yellow)),
            Span::raw(": filter | "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(": next tab"),
        ])]
    } else if app.focus == Focus::Details && app.detail_view == DetailViewType::Files {
        // File-specific help when in Files tab
        vec![Line::from(vec![
            Span::styled("?", Style::default().fg(Color::Yellow)),
            Span::raw(": help | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": view file | "),
            Span::styled("f", Style::default().fg(Color::Yellow)),
            Span::raw(": filter | "),
            Span::styled("c", Style::default().fg(Color::Yellow)),
            Span::raw(": clear filter | "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(": next tab"),
        ])]
    } else {
        // General help
        vec![Line::from(vec![
            Span::styled("?", Style::default().fg(Color::Yellow)),
            Span::raw(": help | "),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(": new | "),
            Span::styled("i", Style::default().fg(Color::Yellow)),
            Span::raw(": init | "),
            Span::styled("I", Style::default().fg(Color::Yellow)),
            Span::raw(": reinit | "),
            Span::styled("R", Style::default().fg(Color::Yellow)),
            Span::raw(": reset | "),
            Span::styled("x", Style::default().fg(Color::Yellow)),
            Span::raw(": run | "),
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(": submit | "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(": delete | "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(": refresh | "),
            Span::styled("a", Style::default().fg(Color::Yellow)),
            Span::raw(": all users"),
        ])]
    };

    // Build title with Torc logo and status message
    // ASCII representation of the Torc workflow icon: ○─○─▶
    let logo = "○─○─▶ ";
    let logo_style = Style::default().fg(Color::Cyan);

    let (title_text, title_style) = if let Some(ref status) = app.status_message {
        if status.is_visible() {
            (
                format!("Torc ─ {}", status.message),
                Style::default().fg(status.color()),
            )
        } else {
            (
                "Torc".to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        }
    } else {
        (
            "Torc".to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    };

    let title_line = Line::from(vec![
        Span::styled(logo, logo_style),
        Span::styled(title_text, title_style),
    ]);

    let block = Block::default().borders(Borders::ALL).title(title_line);

    let paragraph = ratatui::widgets::Paragraph::new(help_text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_server_url(f: &mut Frame, area: Rect, app: &App) {
    use crate::client::version_check;

    // Server status indicator
    let (status_icon, status_color) = if app.is_server_running() {
        ("● ", Color::Green) // Running
    } else if app.server_process.is_some() {
        ("○ ", Color::Yellow) // Stopped but was started
    } else {
        ("", Color::White) // Not managed
    };

    let mut spans = vec![
        Span::styled("Server: ", Style::default().fg(Color::White)),
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::styled(&app.server_url, Style::default().fg(Color::Cyan)),
    ];

    // Add version info if available
    if let Some(ref version_result) = app.version_mismatch
        && let Some(ref server_version) = version_result.server_version
    {
        let version_color = match version_result.severity {
            version_check::VersionMismatchSeverity::Major => Color::Red,
            version_check::VersionMismatchSeverity::Minor => Color::Yellow,
            version_check::VersionMismatchSeverity::Patch => Color::Yellow,
            version_check::VersionMismatchSeverity::None => Color::Green,
        };
        let display = if let Some(ref api_ver) = version_result.server_api_version {
            format!(" (server {} API {})", server_version, api_ver)
        } else {
            format!(" (server {})", server_version)
        };
        spans.push(Span::styled(display, Style::default().fg(version_color)));
    }

    spans.extend(vec![
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("u", Style::default().fg(Color::Yellow)),
        Span::styled(": URL ", Style::default().fg(Color::DarkGray)),
    ]);

    // Add server management hints
    if app.is_server_running() {
        spans.extend(vec![
            Span::styled("K", Style::default().fg(Color::Yellow)),
            Span::styled(": stop ", Style::default().fg(Color::DarkGray)),
            Span::styled("O", Style::default().fg(Color::Yellow)),
            Span::styled(": output", Style::default().fg(Color::DarkGray)),
        ]);
    } else {
        spans.extend(vec![
            Span::styled("S", Style::default().fg(Color::Yellow)),
            Span::styled(": start", Style::default().fg(Color::DarkGray)),
        ]);
    }

    // Add user display
    let user_display = app.get_current_user_display();
    let user_color = if app.show_all_users {
        Color::Yellow
    } else {
        Color::Cyan
    };
    spans.extend(vec![
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("◎ ", Style::default().fg(user_color)),
        Span::styled("User: ", Style::default().fg(Color::White)),
        Span::styled(user_display, Style::default().fg(user_color)),
        Span::styled(" ", Style::default()),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::styled(": toggle", Style::default().fg(Color::DarkGray)),
    ]);

    let text = vec![Line::from(spans)];

    // Build title with TUI version
    let title = Line::from(vec![
        Span::styled("◉ ", Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("Connection │ v{}", version_check::full_version()),
            Style::default().fg(Color::White),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_workflows_table(f: &mut Frame, area: Rect, app: &mut App) {
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Cyan);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["ID", "Name", "User", "Description"])
        .style(header_style)
        .bottom_margin(1);

    let rows = app.workflows.iter().map(|workflow| {
        let id = workflow.id.map(|i| i.to_string()).unwrap_or_default();
        let name = workflow.name.clone();
        let user = workflow.user.clone();
        let description = workflow
            .description
            .clone()
            .unwrap_or_else(|| String::from(""));

        Row::new(vec![
            Cell::from(id),
            Cell::from(name),
            Cell::from(user),
            Cell::from(description),
        ])
    });

    let (title, border_style) = if app.focus == Focus::Workflows {
        (
            Line::from(vec![
                Span::styled("◆ ", Style::default().fg(Color::Green)),
                Span::styled("Workflows", Style::default().fg(Color::White)),
                Span::styled(
                    " │ Enter: load details",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("◇ ", Style::default().fg(Color::Cyan)),
                Span::styled("Workflows", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Length(15),
            Constraint::Percentage(100),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.workflows_state);
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let all_types = DetailViewType::all();
    let titles: Vec<&str> = all_types.iter().map(|t| t.as_str()).collect();

    let selected = match app.detail_view {
        DetailViewType::Jobs => 0,
        DetailViewType::Files => 1,
        DetailViewType::Events => 2,
        DetailViewType::Results => 3,
        DetailViewType::ScheduledNodes => 4,
        DetailViewType::SlurmStats => 5,
        DetailViewType::Dag => 6,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("◈ Detail View")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(selected)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    f.render_widget(tabs, area);
}

fn draw_detail_table(f: &mut Frame, area: Rect, app: &mut App) {
    match app.detail_view {
        DetailViewType::Jobs => draw_jobs_table(f, area, app),
        DetailViewType::Files => draw_files_table(f, area, app),
        DetailViewType::Events => draw_events_table(f, area, app),
        DetailViewType::Results => draw_results_table(f, area, app),
        DetailViewType::ScheduledNodes => draw_scheduled_nodes_table(f, area, app),
        DetailViewType::SlurmStats => draw_slurm_stats_table(f, area, app),
        DetailViewType::Dag => draw_dag(f, area, app),
    }
}

fn draw_jobs_table(f: &mut Frame, area: Rect, app: &mut App) {
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Cyan);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["ID", "Name", "Status", "Command"])
        .style(header_style)
        .bottom_margin(1);

    let rows = app.jobs.iter().map(|job| {
        let id = job.id.map(|i| i.to_string()).unwrap_or_default();
        let name = job.name.clone();
        let status_str = job
            .status
            .as_ref()
            .map(|s| format!("{:?}", s))
            .unwrap_or_default();

        // Color the status based on its value
        let status_color = match status_str.as_str() {
            "Completed" => Color::Green,
            "Running" => Color::Yellow,
            "Failed" => Color::Red,
            "Canceled" | "Terminated" => Color::Magenta,
            "Ready" => Color::Cyan,
            "Blocked" => Color::DarkGray,
            "Pending" | "Scheduled" => Color::Blue,
            _ => Color::White,
        };

        let command = job.command.clone();

        Row::new(vec![
            Cell::from(id),
            Cell::from(name),
            Cell::from(Span::styled(status_str, Style::default().fg(status_color))),
            Cell::from(command),
        ])
    });

    let (title, border_style) = if app.focus == Focus::Details {
        (
            Line::from(vec![
                Span::styled("▶ ", Style::default().fg(Color::Green)),
                Span::styled("Jobs", Style::default().fg(Color::White)),
                Span::styled(
                    " │ Enter: details  l: logs  c: cancel  t: terminate  y: retry",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("▶ ", Style::default().fg(Color::Cyan)),
                Span::styled("Jobs", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Length(15),
            Constraint::Percentage(100),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.jobs_state);
}

/// Format epoch seconds as ISO 8601 timestamp
fn format_timestamp(epoch_secs: f64) -> String {
    use chrono::{DateTime, Utc};
    let secs = epoch_secs as i64;
    let nsecs = ((epoch_secs - secs as f64) * 1_000_000_000.0) as u32;
    DateTime::<Utc>::from_timestamp(secs, nsecs)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

fn draw_files_table(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Details;
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Cyan);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["ID", "Name", "Path", "Modified"])
        .style(header_style)
        .bottom_margin(1);

    let rows = app.files.iter().map(|file| {
        let id = file.id.map(|i| i.to_string()).unwrap_or_default();
        let name = file.name.clone();
        let path = file.path.clone();
        let st_mtime = file.st_mtime.map(format_timestamp).unwrap_or_default();

        Row::new(vec![
            Cell::from(id),
            Cell::from(name),
            Cell::from(path),
            Cell::from(st_mtime),
        ])
    });

    let (title, border_style) = if is_focused {
        (
            Line::from(vec![
                Span::styled("◫ ", Style::default().fg(Color::Green)),
                Span::styled("Files", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("◫ ", Style::default().fg(Color::Cyan)),
                Span::styled("Files", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Percentage(50),
            Constraint::Length(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.files_state);
}

fn draw_events_table(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Details;
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Cyan);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["Timestamp", "Level", "Event Type", "Data"])
        .style(header_style)
        .bottom_margin(1);

    let rows = app.events.iter().map(|event| {
        let timestamp = format_timestamp_ms(event.timestamp);
        let severity_str = event.severity.to_string();
        let event_type = &event.event_type;
        let data = event.data.to_string();

        let severity_color = match severity_str.to_lowercase().as_str() {
            "error" => Color::Red,
            "warning" => Color::Yellow,
            "info" => Color::Green,
            "debug" => Color::Blue,
            _ => Color::White,
        };

        Row::new(vec![
            Cell::from(timestamp),
            Cell::from(Span::styled(
                severity_str,
                Style::default().fg(severity_color),
            )),
            Cell::from(event_type.clone()),
            Cell::from(data),
        ])
    });

    let event_count = app.events.len();
    let (title, border_style) = if is_focused {
        (
            Line::from(vec![
                Span::styled("⚡ ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("Events ({})", event_count),
                    Style::default().fg(Color::White),
                ),
                Span::styled(" [SSE Live]", Style::default().fg(Color::Green)),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("⚡ ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("Events ({})", event_count),
                    Style::default().fg(Color::White),
                ),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(20),     // Timestamp
            Constraint::Length(10),     // Level
            Constraint::Length(25),     // Event Type
            Constraint::Percentage(55), // Data
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.events_state);
}

fn draw_results_table(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Details;
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Cyan);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec![
        "ID", "Job ID", "Run", "Attempt", "Return", "Status", "Peak Mem", "Peak CPU",
    ])
    .style(header_style)
    .bottom_margin(1);

    let rows = app.results.iter().map(|result| {
        let id = result.id.map(|i| i.to_string()).unwrap_or_default();
        let job_id = result.job_id.to_string();
        let run_id = result.run_id.to_string();
        let attempt_id = result.attempt_id.unwrap_or(1).to_string();
        let return_code = result.return_code;
        let status = format!("{:?}", result.status);

        // Format peak memory (bytes to human readable)
        let peak_mem = result
            .peak_memory_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "-".to_string());

        // Format peak CPU percentage
        let peak_cpu = result
            .peak_cpu_percent
            .map(|pct| format!("{:.1}%", pct))
            .unwrap_or_else(|| "-".to_string());

        // Color based on return code
        let row_color = if return_code == 0 {
            Color::Green
        } else {
            Color::Red
        };

        Row::new(vec![
            Cell::from(id),
            Cell::from(job_id),
            Cell::from(run_id),
            Cell::from(attempt_id),
            Cell::from(Span::styled(
                return_code.to_string(),
                Style::default().fg(row_color),
            )),
            Cell::from(Span::styled(status, Style::default().fg(row_color))),
            Cell::from(peak_mem),
            Cell::from(peak_cpu),
        ])
    });

    let (title, border_style) = if is_focused {
        (
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled("Results", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Cyan)),
                Span::styled("Results", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),  // ID
            Constraint::Length(8),  // Job ID
            Constraint::Length(5),  // Run
            Constraint::Length(7),  // Attempt
            Constraint::Length(7),  // Return
            Constraint::Length(12), // Status
            Constraint::Length(10), // Peak Mem
            Constraint::Length(10), // Peak CPU
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.results_state);
}

fn draw_scheduled_nodes_table(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Details;
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Cyan);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["ID", "Scheduler ID", "Config ID", "Type", "Status"])
        .style(header_style)
        .bottom_margin(1);

    let rows = app.scheduled_nodes.iter().map(|node| {
        let id = node.id.map(|i| i.to_string()).unwrap_or_default();
        let scheduler_id = node.scheduler_id.to_string();
        let config_id = node.scheduler_config_id.to_string();
        let scheduler_type = node.scheduler_type.clone();
        let status = node.status.clone();

        // Color based on status
        let status_color = match status.as_str() {
            "running" => Color::Green,
            "pending" | "scheduled" => Color::Yellow,
            "failed" | "error" => Color::Red,
            "completed" | "done" => Color::Blue,
            _ => Color::White,
        };

        Row::new(vec![
            Cell::from(id),
            Cell::from(scheduler_id),
            Cell::from(config_id),
            Cell::from(scheduler_type),
            Cell::from(Span::styled(status, Style::default().fg(status_color))),
        ])
    });

    let (title, border_style) = if is_focused {
        (
            Line::from(vec![
                Span::styled("⊞ ", Style::default().fg(Color::Green)),
                Span::styled("Scheduled Nodes", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("⊞ ", Style::default().fg(Color::Cyan)),
                Span::styled("Scheduled Nodes", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),  // ID
            Constraint::Length(14), // Scheduler ID
            Constraint::Length(10), // Config ID
            Constraint::Length(10), // Type
            Constraint::Length(12), // Status
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.scheduled_nodes_state);
}

fn draw_slurm_stats_table(f: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focus == Focus::Details;
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Cyan);
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec![
        "Job ID",
        "Run",
        "Attempt",
        "Slurm Job",
        "Max RSS",
        "Max VM",
        "Ave CPU (s)",
        "CPU %",
        "Nodes",
    ])
    .style(header_style)
    .bottom_margin(1);

    let rows = app.slurm_stats.iter().map(|stat| {
        let job_id = stat.job_id.to_string();
        let run_id = stat.run_id.to_string();
        let attempt_id = stat.attempt_id.to_string();
        let slurm_job_id = stat.slurm_job_id.clone().unwrap_or_else(|| "-".to_string());
        let max_rss = stat
            .max_rss_bytes
            .filter(|&b| b > 0)
            .map(format_bytes)
            .unwrap_or_else(|| "-".to_string());
        let max_vm = stat
            .max_vm_size_bytes
            .filter(|&b| b > 0)
            .map(format_bytes)
            .unwrap_or_else(|| "-".to_string());
        let ave_cpu = stat
            .ave_cpu_seconds
            .filter(|&s| s > 0.0)
            .map(|s| format!("{:.1}", s))
            .unwrap_or_else(|| "-".to_string());
        let cpu_pct = stat
            .ave_cpu_seconds
            .filter(|&s| s > 0.0)
            .and_then(|ave_s| {
                app.exec_time_map
                    .get(&(stat.job_id, stat.run_id, stat.attempt_id))
                    .filter(|&&m| m > 0.0)
                    .map(|&m| ave_s / (m * 60.0) * 100.0)
            })
            .map(|p| format!("{:.1}%", p))
            .unwrap_or_else(|| "-".to_string());
        let nodes = stat.node_list.clone().unwrap_or_else(|| "-".to_string());

        Row::new(vec![
            Cell::from(job_id),
            Cell::from(run_id),
            Cell::from(attempt_id),
            Cell::from(slurm_job_id),
            Cell::from(max_rss),
            Cell::from(max_vm),
            Cell::from(ave_cpu),
            Cell::from(cpu_pct),
            Cell::from(nodes),
        ])
    });

    let stat_count = app.slurm_stats.len();
    let (title, border_style) = if is_focused {
        (
            Line::from(vec![
                Span::styled("⚑ ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("Slurm Stats ({})", stat_count),
                    Style::default().fg(Color::White),
                ),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("⚑ ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("Slurm Stats ({})", stat_count),
                    Style::default().fg(Color::White),
                ),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),  // Job ID
            Constraint::Length(5),  // Run
            Constraint::Length(8),  // Attempt
            Constraint::Length(12), // Slurm Job
            Constraint::Length(10), // Max RSS
            Constraint::Length(10), // Max VM
            Constraint::Length(12), // Ave CPU (s)
            Constraint::Length(10), // CPU %
            Constraint::Min(10),    // Nodes
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    )
    .row_highlight_style(selected_style)
    .highlight_symbol("▸ ");

    f.render_stateful_widget(table, area, &mut app.slurm_stats_state);
}

fn draw_filter_input(f: &mut Frame, area: Rect, app: &App) {
    let columns = app.get_filter_columns();
    let selected_column = columns[app.filter_column_index];

    let filter_status = if let Some(ref filter) = app.filter {
        format!(
            " | Active filter: {} contains '{}'",
            filter.column, filter.value
        )
    } else {
        String::new()
    };

    let text = vec![Line::from(vec![
        Span::styled("Filter by ", Style::default().fg(Color::White)),
        Span::styled(
            selected_column,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": ", Style::default().fg(Color::White)),
        Span::styled(&app.filter_input, Style::default().fg(Color::Cyan)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::styled(": change column | ", Style::default().fg(Color::White)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(": apply | ", Style::default().fg(Color::White)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(": cancel", Style::default().fg(Color::White)),
        Span::styled(&filter_status, Style::default().fg(Color::DarkGray)),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Filter Input")
        .border_style(Style::default().fg(Color::Green));

    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_server_url_input(f: &mut Frame, area: Rect, app: &App) {
    let text = vec![Line::from(vec![
        Span::styled("Server URL: ", Style::default().fg(Color::White)),
        Span::styled(&app.server_url_input, Style::default().fg(Color::Cyan)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(": connect | ", Style::default().fg(Color::White)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(": cancel", Style::default().fg(Color::White)),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Change Server URL")
        .border_style(Style::default().fg(Color::Green));

    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_workflow_path_input(f: &mut Frame, area: Rect, app: &App) {
    let text = vec![Line::from(vec![
        Span::styled("Workflow spec file: ", Style::default().fg(Color::White)),
        Span::styled(&app.workflow_path_input, Style::default().fg(Color::Cyan)),
        Span::styled("_", Style::default().fg(Color::White)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(": create | ", Style::default().fg(Color::White)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(": cancel", Style::default().fg(Color::White)),
        Span::styled(
            " (supports ~, YAML/JSON/JSON5)",
            Style::default().fg(Color::DarkGray),
        ),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Create Workflow")
        .border_style(Style::default().fg(Color::Green));

    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn draw_dag(f: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focus == Focus::Details;
    let (title, border_style) = if is_focused {
        (
            Line::from(vec![
                Span::styled("◇ ", Style::default().fg(Color::Green)),
                Span::styled("Job DAG", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            Line::from(vec![
                Span::styled("◇ ", Style::default().fg(Color::Cyan)),
                Span::styled("Job DAG", Style::default().fg(Color::White)),
            ]),
            Style::default().fg(Color::DarkGray),
        )
    };

    if let Some(ref dag) = app.dag {
        // Build a text-based representation of the DAG
        let mut lines = Vec::new();

        // Compute layers for topological display
        let layers = dag_compute_layers(&dag.graph);

        // Display jobs layer by layer (top to bottom)
        for (layer_idx, layer) in layers.iter().enumerate() {
            if layer_idx > 0 {
                // Add a visual separator between layers showing flow direction
                lines.push(Line::from(vec![Span::styled(
                    "   ↓↓↓",
                    Style::default().fg(Color::DarkGray),
                )]));
            }

            // Group jobs in this layer by their predecessors to show subgraphs
            let mut current_pred_group: Option<usize> = None;

            // Display all jobs in this layer
            for &node_idx in layer {
                // Check if this job belongs to a different subgraph
                let first_pred: Option<usize> = dag
                    .graph
                    .edges_directed(node_idx, petgraph::Direction::Incoming)
                    .next()
                    .map(|e| e.source().index());

                // Add a separator between different subgraph groups within the same layer
                if layer.len() > 1
                    && first_pred != current_pred_group
                    && current_pred_group.is_some()
                {
                    lines.push(Line::from(vec![Span::styled(
                        "  ─ ─ ─",
                        Style::default().fg(Color::DarkGray),
                    )]));
                }
                current_pred_group = first_pred;

                let node_data = &dag.graph[node_idx];

                // Determine color based on status
                let color = match node_data.status.as_deref() {
                    Some("Completed") => Color::Green,
                    Some("Running") => Color::Yellow,
                    Some("Failed") => Color::Red,
                    Some("Canceled") => Color::Magenta,
                    _ => Color::Cyan,
                };

                // Create a status indicator
                let status_char = match node_data.status.as_deref() {
                    Some("Completed") => "✓",
                    Some("Running") => "▶",
                    Some("Failed") => "✗",
                    Some("Canceled") => "○",
                    _ => "◦",
                };

                // Format: [status] job_name (id: job_id)
                let job_line = format!(
                    "  [{}] {} (id: {})",
                    status_char, node_data.name, node_data.id
                );

                lines.push(Line::from(vec![Span::styled(
                    job_line,
                    Style::default().fg(color),
                )]));
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "No jobs in DAG",
                Style::default().fg(Color::DarkGray),
            )]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style);

        let paragraph = ratatui::widgets::Paragraph::new(lines)
            .block(block)
            .style(Style::default().fg(Color::White))
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(paragraph, area);
    } else {
        // No DAG loaded yet
        let text = vec![Line::from(vec![Span::styled(
            "No DAG data available. Press Enter to load.",
            Style::default().fg(Color::DarkGray),
        )])];

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style);

        let paragraph = ratatui::widgets::Paragraph::new(text)
            .block(block)
            .style(Style::default().fg(Color::White))
            .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(paragraph, area);
    }
}

// Helper function to compute layers for DAG visualization
fn dag_compute_layers(
    graph: &petgraph::Graph<super::dag::JobNode, ()>,
) -> Vec<Vec<petgraph::graph::NodeIndex>> {
    let mut layers: Vec<Vec<petgraph::graph::NodeIndex>> = Vec::new();
    let mut node_layer: HashMap<petgraph::graph::NodeIndex, usize> = HashMap::new();

    // Topological traversal
    let mut topo = Topo::new(graph);
    while let Some(node) = topo.next(graph) {
        // Find the maximum layer of all predecessors
        let mut max_predecessor_layer = 0;
        for edge in graph.edges_directed(node, petgraph::Direction::Incoming) {
            if let Some(&layer) = node_layer.get(&edge.source()) {
                max_predecessor_layer = max_predecessor_layer.max(layer + 1);
            }
        }

        node_layer.insert(node, max_predecessor_layer);

        // Add to appropriate layer
        while layers.len() <= max_predecessor_layer {
            layers.push(Vec::new());
        }
        layers[max_predecessor_layer].push(node);
    }

    // Sort nodes within each layer to group related subgraphs together
    // Group by their parent nodes to keep subgraphs visually connected
    for layer in &mut layers {
        layer.sort_by(|a, b| {
            // Get predecessor indices as sort keys
            let a_preds: Vec<usize> = graph
                .edges_directed(*a, petgraph::Direction::Incoming)
                .map(|e| e.source().index())
                .collect();
            let b_preds: Vec<usize> = graph
                .edges_directed(*b, petgraph::Direction::Incoming)
                .map(|e| e.source().index())
                .collect();

            // Sort by first predecessor, then by job name for consistency
            match (a_preds.first(), b_preds.first()) {
                (Some(a_pred), Some(b_pred)) => a_pred.cmp(b_pred).then_with(|| {
                    let a_name = &graph[*a].name;
                    let b_name = &graph[*b].name;
                    a_name.cmp(b_name)
                }),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => {
                    let a_name = &graph[*a].name;
                    let b_name = &graph[*b].name;
                    a_name.cmp(b_name)
                }
            }
        });
    }

    layers
}
