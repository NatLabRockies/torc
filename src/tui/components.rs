//! Reusable UI components for the TUI

use std::fs;
use std::path::Path;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// A confirmation dialog that asks the user to confirm an action
#[derive(Debug, Clone)]
pub struct ConfirmationDialog {
    pub title: String,
    pub message: String,
    pub confirm_text: String,
    pub cancel_text: String,
    pub is_destructive: bool,
}

impl Default for ConfirmationDialog {
    fn default() -> Self {
        Self {
            title: "Confirm".to_string(),
            message: "Are you sure?".to_string(),
            confirm_text: "Yes".to_string(),
            cancel_text: "No".to_string(),
            is_destructive: false,
        }
    }
}

impl ConfirmationDialog {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            ..Default::default()
        }
    }

    pub fn destructive(mut self) -> Self {
        self.is_destructive = true;
        self
    }

    pub fn with_confirm_text(mut self, text: &str) -> Self {
        self.confirm_text = text.to_string();
        self
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        // Calculate dialog size - center in screen
        let dialog_width = 50.min(area.width.saturating_sub(4));
        let dialog_height = 7.min(area.height.saturating_sub(2));

        let dialog_x = (area.width.saturating_sub(dialog_width)) / 2;
        let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        // Clear the area behind the dialog
        f.render_widget(Clear, dialog_area);

        let border_color = if self.is_destructive {
            Color::Red
        } else {
            Color::Yellow
        };

        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(dialog_area);
        f.render_widget(block, dialog_area);

        // Split inner area for message and buttons
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        // Message
        let message = Paragraph::new(self.message.as_str())
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(message, chunks[0]);

        // Buttons hint
        let confirm_style = if self.is_destructive {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };

        let buttons = Line::from(vec![
            Span::styled("y", confirm_style),
            Span::raw(format!(": {} | ", self.confirm_text)),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(format!(": {} | ", self.cancel_text)),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw(": cancel"),
        ]);

        let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
        f.render_widget(buttons_para, chunks[1]);
    }
}

/// Status message types for the status bar
#[derive(Debug, Clone, PartialEq)]
pub enum StatusLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// A status message to display in the status bar
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub message: String,
    pub level: StatusLevel,
    pub timestamp: std::time::Instant,
}

impl StatusMessage {
    pub fn info(message: &str) -> Self {
        Self {
            message: message.to_string(),
            level: StatusLevel::Info,
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn success(message: &str) -> Self {
        Self {
            message: message.to_string(),
            level: StatusLevel::Success,
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn warning(message: &str) -> Self {
        Self {
            message: message.to_string(),
            level: StatusLevel::Warning,
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            message: message.to_string(),
            level: StatusLevel::Error,
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn color(&self) -> Color {
        match self.level {
            StatusLevel::Info => Color::Cyan,
            StatusLevel::Success => Color::Green,
            StatusLevel::Warning => Color::Yellow,
            StatusLevel::Error => Color::Red,
        }
    }

    /// Check if message should still be displayed (auto-dismiss after 5 seconds for success/info)
    pub fn is_visible(&self) -> bool {
        match self.level {
            StatusLevel::Success | StatusLevel::Info => {
                self.timestamp.elapsed() < std::time::Duration::from_secs(5)
            }
            StatusLevel::Warning | StatusLevel::Error => true, // Keep visible until cleared
        }
    }
}

/// An error dialog that displays a full error message in a popup
/// Used when error messages are too long for the status bar
#[derive(Debug, Clone)]
pub struct ErrorDialog {
    pub title: String,
    pub message: String,
}

impl ErrorDialog {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        // Calculate dialog size based on message length
        // Allow more width and height for long messages
        let dialog_width = 80.min(area.width.saturating_sub(4));
        // Calculate height based on message lines (rough estimate)
        // Cap line_count to prevent overflow when casting to u16
        let line_count = self.message.lines().count() + self.message.len() / 70 + 5;
        let capped_line_count = line_count.min(u16::MAX as usize - 4);
        let dialog_height = (capped_line_count as u16 + 4).min(area.height.saturating_sub(4));

        let dialog_x = (area.width.saturating_sub(dialog_width)) / 2;
        let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        // Clear the area behind the dialog
        f.render_widget(Clear, dialog_area);

        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let inner = block.inner(dialog_area);
        f.render_widget(block, dialog_area);

        // Split inner area for message and buttons
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(inner);

        // Message with word wrapping
        let message = Paragraph::new(self.message.as_str())
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });
        f.render_widget(message, chunks[0]);

        // Close hint
        let hint = Line::from(vec![
            Span::raw("Press "),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" or "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" to close"),
        ]);
        let hint_para = Paragraph::new(hint).alignment(Alignment::Center);
        f.render_widget(hint_para, chunks[1]);
    }
}

/// A help popup showing all available keybindings
pub struct HelpPopup;

impl HelpPopup {
    pub fn render(f: &mut Frame, area: Rect, context: &str) {
        // Calculate popup size
        let popup_width = 70.min(area.width.saturating_sub(4));
        let popup_height = 44.min(area.height.saturating_sub(2));

        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Clear the area behind the popup
        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Help (press q or Esc to close) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        let mut lines = vec![
            Line::from(vec![Span::styled(
                "Global Keys",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Self::key_line("q", "Quit / Close popup"),
            Self::key_line("?", "Show this help"),
            Self::key_line("r", "Refresh current view"),
            Self::key_line("A", "Toggle auto-refresh"),
            Self::key_line("Tab/Shift+Tab", "Switch between detail tabs"),
            Self::key_line(left_right_arrows(), "Switch focus between panes"),
            Self::key_line(up_down_arrows(), "Navigate rows in tables"),
            Self::key_line("Enter", "Load details / Confirm action"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Workflow Actions",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Self::key_line("n", "Create new workflow from spec file"),
            Self::key_line("i", "Initialize workflow"),
            Self::key_line("I", "Re-initialize workflow"),
            Self::key_line("R", "Reset workflow status"),
            Self::key_line("x", "Run workflow locally"),
            Self::key_line("s", "Submit workflow to scheduler"),
            Self::key_line("W", "Watch workflow (recovery)"),
            Self::key_line("d", "Delete workflow"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Job Actions (Jobs tab)",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Self::key_line("l", "View logs (Jobs/Scheduled Nodes)"),
            Self::key_line("Enter", "View job details"),
            Self::key_line("c", "Cancel job"),
            Self::key_line("t", "Terminate job"),
            Self::key_line("y", "Retry failed job"),
        ];

        // Add context-specific help
        if context == "filter" {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Filter Mode",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(""));
            lines.push(Self::key_line("Tab", "Change filter column"));
            lines.push(Self::key_line("Enter", "Apply filter"));
            lines.push(Self::key_line("Esc", "Cancel filter"));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Server Management",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));
        lines.push(Self::key_line("S", "Start torc-server"));
        lines.push(Self::key_line("K", "Stop/Kill server"));
        lines.push(Self::key_line("O", "Show server output"));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Connection",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));
        lines.push(Self::key_line("u", "Change server URL"));
        lines.push(Self::key_line("o", "Change output directory"));
        lines.push(Self::key_line("w", "Change user filter"));
        lines.push(Self::key_line("a", "Toggle show all users"));

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }

    fn key_line(key: &str, description: &str) -> Line<'static> {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:16}", key), Style::default().fg(Color::Green)),
            Span::raw(description.to_string()),
        ])
    }
}

/// Job details popup showing full job information
#[derive(Debug, Clone)]
pub struct JobDetailsPopup {
    pub job_id: i64,
    pub job_name: String,
    pub command: String,
    pub status: String,
    pub scroll_offset: u16,
}

impl JobDetailsPopup {
    pub fn new(job_id: i64, job_name: String, command: String, status: String) -> Self {
        Self {
            job_id,
            job_name,
            command,
            status,
            scroll_offset: 0,
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let popup_width = 80.min(area.width.saturating_sub(4));
        let popup_height = 20.min(area.height.saturating_sub(2));

        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(format!(" Job Details: {} ", self.job_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        let status_color = match self.status.as_str() {
            "Completed" => Color::Green,
            "Running" => Color::Yellow,
            "Failed" => Color::Red,
            "Canceled" => Color::Magenta,
            _ => Color::White,
        };

        let lines = vec![
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::raw(self.job_id.to_string()),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&self.status, Style::default().fg(status_color)),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Command:",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(self.command.clone()),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Press q or Esc to close, l to view logs",
                Style::default().fg(Color::DarkGray),
            )]),
        ];

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));

        f.render_widget(paragraph, inner);
    }
}

/// Log viewer for displaying job stdout/stderr
#[derive(Debug, Clone)]
pub struct LogViewer {
    pub job_id: i64,
    pub job_name: String,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub stdout_content: String,
    pub stderr_content: String,
    pub active_tab: LogTab,
    pub scroll_offset: u16,
    pub search_query: String,
    pub search_matches: Vec<usize>, // Line numbers with matches
    pub current_match: usize,
    pub is_searching: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogTab {
    Stdout,
    Stderr,
}

impl LogViewer {
    pub fn new(job_id: i64, job_name: String) -> Self {
        Self {
            job_id,
            job_name,
            stdout_path: None,
            stderr_path: None,
            stdout_content: String::new(),
            stderr_content: String::new(),
            active_tab: LogTab::Stdout,
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: 0,
            is_searching: false,
        }
    }

    pub fn toggle_tab(&mut self) {
        self.active_tab = match self.active_tab {
            LogTab::Stdout => LogTab::Stderr,
            LogTab::Stderr => LogTab::Stdout,
        };
        self.scroll_offset = 0;
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self, visible_height: u16) {
        let content = self.current_content();
        let line_count = content.lines().count() as u16;
        self.scroll_offset = line_count.saturating_sub(visible_height);
    }

    pub fn current_content(&self) -> &str {
        match self.active_tab {
            LogTab::Stdout => &self.stdout_content,
            LogTab::Stderr => &self.stderr_content,
        }
    }

    pub fn current_path(&self) -> Option<&str> {
        match self.active_tab {
            LogTab::Stdout => self.stdout_path.as_deref(),
            LogTab::Stderr => self.stderr_path.as_deref(),
        }
    }

    pub fn start_search(&mut self) {
        self.is_searching = true;
        self.search_query.clear();
    }

    pub fn cancel_search(&mut self) {
        self.is_searching = false;
    }

    pub fn add_search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.update_search_matches();
    }

    pub fn remove_search_char(&mut self) {
        self.search_query.pop();
        self.update_search_matches();
    }

    pub fn apply_search(&mut self) {
        self.is_searching = false;
        if !self.search_matches.is_empty() {
            self.jump_to_match(0);
        }
    }

    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }

        let query = self.search_query.to_lowercase();
        // Clone content to avoid borrow issues
        let content = self.current_content().to_string();

        let matches: Vec<usize> = content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();

        self.search_matches = matches;
        self.current_match = 0;
    }

    pub fn next_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.search_matches.len();
            self.jump_to_current_match();
        }
    }

    pub fn prev_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.current_match = if self.current_match == 0 {
                self.search_matches.len() - 1
            } else {
                self.current_match - 1
            };
            self.jump_to_current_match();
        }
    }

    fn jump_to_match(&mut self, index: usize) {
        if let Some(&line_num) = self.search_matches.get(index) {
            self.scroll_offset = line_num as u16;
        }
    }

    fn jump_to_current_match(&mut self) {
        self.jump_to_match(self.current_match);
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(format!(" Logs: {} ", self.job_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Split into tabs, content, and search/status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Tab bar
                Constraint::Min(5),    // Content
                Constraint::Length(2), // Status/search bar
            ])
            .split(inner);

        // Tab bar
        let stdout_style = if self.active_tab == LogTab::Stdout {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let stderr_style = if self.active_tab == LogTab::Stderr {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let tab_line = Line::from(vec![
            Span::styled(" stdout ", stdout_style),
            Span::raw(" | "),
            Span::styled(" stderr ", stderr_style),
            Span::raw("  "),
            Span::styled("(Tab to switch)", Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(tab_line), chunks[0]);

        // Content
        let content = self.current_content();
        let lines: Vec<Line> = content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let is_match = self.search_matches.contains(&i);
                let is_current_match =
                    !self.search_matches.is_empty() && self.search_matches[self.current_match] == i;

                let style = if is_current_match {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else if is_match {
                    Style::default().bg(Color::DarkGray)
                } else if self.active_tab == LogTab::Stderr && !line.is_empty() {
                    Style::default().fg(Color::LightRed)
                } else {
                    Style::default().fg(Color::White)
                };

                // Add line numbers
                let line_num = format!("{:4} ", i + 1);
                Line::from(vec![
                    Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                    Span::styled(line.to_string(), style),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .scroll((self.scroll_offset, 0))
            .wrap(Wrap { trim: false });
        f.render_widget(paragraph, chunks[1]);

        // Status/search bar
        let status_line: Line = if self.is_searching {
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Yellow)),
                Span::raw(self.search_query.clone()),
                Span::styled("_", Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled("Enter", Style::default().fg(Color::DarkGray)),
                Span::raw(": apply | "),
                Span::styled("Esc", Style::default().fg(Color::DarkGray)),
                Span::raw(": cancel"),
            ])
        } else {
            let match_info = if !self.search_matches.is_empty() {
                format!(
                    " | Match {}/{}",
                    self.current_match + 1,
                    self.search_matches.len()
                )
            } else {
                String::new()
            };

            let path_info = self
                .current_path()
                .map(|p| format!("Path: {}", p))
                .unwrap_or_else(|| "No log file".to_string());

            Line::from(vec![
                Span::styled(path_info, Style::default().fg(Color::DarkGray)),
                Span::raw(match_info),
            ])
        };

        let help_line = Line::from(vec![
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(": close | "),
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(": search | "),
            Span::styled("n/N", Style::default().fg(Color::Yellow)),
            Span::raw(": next/prev | "),
            Span::styled("g/G", Style::default().fg(Color::Yellow)),
            Span::raw(": top/bottom | "),
            Span::styled("y", Style::default().fg(Color::Yellow)),
            Span::raw(": copy path"),
        ]);

        let status_para = Paragraph::new(vec![status_line, help_line]);
        f.render_widget(status_para, chunks[2]);
    }
}

/// File viewer for displaying file contents
#[derive(Debug, Clone)]
pub struct FileViewer {
    pub file_name: String,
    pub file_path: String,
    pub content: String,
    pub scroll_offset: u16,
    pub search_query: String,
    pub search_matches: Vec<usize>, // Line numbers with matches
    pub current_match: usize,
    pub is_searching: bool,
    pub is_binary: bool,
}

impl FileViewer {
    pub fn new(file_name: String, file_path: String) -> Self {
        Self {
            file_name,
            file_path,
            content: String::new(),
            scroll_offset: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: 0,
            is_searching: false,
            is_binary: false,
        }
    }

    pub fn load_content(&mut self) -> Result<(), String> {
        let path = Path::new(&self.file_path);

        if !path.exists() {
            self.content = format!(
                "File not found: {}\n\nThe file may not exist if:\n- It hasn't been created yet\n- You are on a different system\n- The path is incorrect",
                self.file_path
            );
            return Ok(());
        }

        // Check file size - limit to 1MB for display
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > 1_000_000 {
            self.content = format!(
                "File is too large to display ({:.2} MB)\n\nPath: {}",
                metadata.len() as f64 / 1_000_000.0,
                self.file_path
            );
            return Ok(());
        }

        // Try to read as text
        match fs::read_to_string(path) {
            Ok(content) => {
                self.content = content;
                self.is_binary = false;
            }
            Err(_) => {
                // Might be binary - try to read as bytes and show hex dump snippet
                match fs::read(path) {
                    Ok(bytes) => {
                        self.is_binary = true;
                        let preview_len = bytes.len().min(512);
                        let hex_lines: Vec<String> = bytes[..preview_len]
                            .chunks(16)
                            .enumerate()
                            .map(|(i, chunk)| {
                                let hex: String =
                                    chunk.iter().map(|b| format!("{:02x} ", b)).collect();
                                let ascii: String = chunk
                                    .iter()
                                    .map(|&b| {
                                        if (32..127).contains(&b) {
                                            b as char
                                        } else {
                                            '.'
                                        }
                                    })
                                    .collect();
                                format!("{:08x}  {:48}  {}", i * 16, hex, ascii)
                            })
                            .collect();

                        self.content = format!(
                            "Binary file ({} bytes)\n\nHex dump (first {} bytes):\n\n{}{}",
                            bytes.len(),
                            preview_len,
                            hex_lines.join("\n"),
                            if bytes.len() > preview_len {
                                "\n\n... (truncated)"
                            } else {
                                ""
                            }
                        );
                    }
                    Err(e) => {
                        self.content = format!("Could not read file: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self, visible_height: u16) {
        let line_count = self.content.lines().count() as u16;
        self.scroll_offset = line_count.saturating_sub(visible_height);
    }

    pub fn start_search(&mut self) {
        self.is_searching = true;
        self.search_query.clear();
    }

    pub fn cancel_search(&mut self) {
        self.is_searching = false;
    }

    pub fn add_search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.update_search_matches();
    }

    pub fn remove_search_char(&mut self) {
        self.search_query.pop();
        self.update_search_matches();
    }

    pub fn apply_search(&mut self) {
        self.is_searching = false;
        if !self.search_matches.is_empty() {
            self.jump_to_match(0);
        }
    }

    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }

        let query = self.search_query.to_lowercase();
        let content = self.content.clone();

        let matches: Vec<usize> = content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();

        self.search_matches = matches;
        self.current_match = 0;
    }

    pub fn next_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.search_matches.len();
            self.jump_to_current_match();
        }
    }

    pub fn prev_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.current_match = if self.current_match == 0 {
                self.search_matches.len() - 1
            } else {
                self.current_match - 1
            };
            self.jump_to_current_match();
        }
    }

    fn jump_to_match(&mut self, index: usize) {
        if let Some(&line_num) = self.search_matches.get(index) {
            self.scroll_offset = line_num as u16;
        }
    }

    fn jump_to_current_match(&mut self) {
        self.jump_to_match(self.current_match);
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(format!(" File: {} ", self.file_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Split into content and status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),    // Content
                Constraint::Length(2), // Status/search bar
            ])
            .split(inner);

        // Content
        let lines: Vec<Line> = self
            .content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let is_match = self.search_matches.contains(&i);
                let is_current_match =
                    !self.search_matches.is_empty() && self.search_matches[self.current_match] == i;

                let style = if is_current_match {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else if is_match {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };

                // Add line numbers
                let line_num = format!("{:4} ", i + 1);
                Line::from(vec![
                    Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                    Span::styled(line.to_string(), style),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .scroll((self.scroll_offset, 0))
            .wrap(Wrap { trim: false });
        f.render_widget(paragraph, chunks[0]);

        // Status/search bar
        let status_line: Line = if self.is_searching {
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Yellow)),
                Span::raw(self.search_query.clone()),
                Span::styled("_", Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled("Enter", Style::default().fg(Color::DarkGray)),
                Span::raw(": apply | "),
                Span::styled("Esc", Style::default().fg(Color::DarkGray)),
                Span::raw(": cancel"),
            ])
        } else {
            let match_info = if !self.search_matches.is_empty() {
                format!(
                    " | Match {}/{}",
                    self.current_match + 1,
                    self.search_matches.len()
                )
            } else {
                String::new()
            };

            let file_info = if self.is_binary {
                format!("Binary file | Path: {}", self.file_path)
            } else {
                format!("Path: {}", self.file_path)
            };

            Line::from(vec![
                Span::styled(file_info, Style::default().fg(Color::DarkGray)),
                Span::raw(match_info),
            ])
        };

        let help_line = Line::from(vec![
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(": close | "),
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(": search | "),
            Span::styled("n/N", Style::default().fg(Color::Yellow)),
            Span::raw(": next/prev | "),
            Span::styled("g/G", Style::default().fg(Color::Yellow)),
            Span::raw(": top/bottom | "),
            Span::styled("y", Style::default().fg(Color::Yellow)),
            Span::raw(": show path"),
        ]);

        let status_para = Paragraph::new(vec![status_line, help_line]);
        f.render_widget(status_para, chunks[1]);
    }
}

/// Process viewer for displaying subprocess output
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

pub struct ProcessViewer {
    pub title: String,
    pub output_lines: Vec<String>,
    pub scroll_offset: u16,
    pub auto_scroll: bool,
    pub is_running: bool,
    pub kill_confirm: bool,
    child: Option<Child>,
    output_receiver: Option<Receiver<String>>,
}

impl ProcessViewer {
    pub fn new(title: String) -> Self {
        Self {
            title,
            output_lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            is_running: false,
            kill_confirm: false,
            child: None,
            output_receiver: None,
        }
    }

    /// Start a process and capture its output
    pub fn start(&mut self, program: &str, args: &[&str]) -> Result<(), String> {
        let mut cmd = Command::new(program);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn process: {}", e))?;

        // Create channel for output
        let (tx, rx) = mpsc::channel();

        // Spawn thread to read stdout
        if let Some(stdout) = child.stdout.take() {
            let tx_stdout = tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    let _ = tx_stdout.send(line);
                }
            });
        }

        // Spawn thread to read stderr
        if let Some(stderr) = child.stderr.take() {
            let tx_stderr = tx;
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let _ = tx_stderr.send(format!("[stderr] {}", line));
                }
            });
        }

        self.child = Some(child);
        self.output_receiver = Some(rx);
        self.is_running = true;
        self.output_lines
            .push(format!("Started: {} {}", program, args.join(" ")));
        self.output_lines.push(String::new());

        Ok(())
    }

    /// Poll for new output from the process
    pub fn poll_output(&mut self) {
        // Don't poll if process is not running and we've already cleaned up
        if !self.is_running && self.output_receiver.is_none() {
            return;
        }

        // Read any available output
        let mut channel_disconnected = false;
        if let Some(ref rx) = self.output_receiver {
            loop {
                match rx.try_recv() {
                    Ok(line) => {
                        self.output_lines.push(line);
                        if self.auto_scroll {
                            // Will be adjusted in render based on visible height
                            self.scroll_offset = u16::MAX;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // Process output streams closed
                        channel_disconnected = true;
                        break;
                    }
                }
            }
        }

        // Check if process has exited
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.is_running = false;
                    self.output_lines.push(String::new());
                    if status.success() {
                        self.output_lines
                            .push("Process exited successfully (code: 0)".to_string());
                    } else {
                        self.output_lines.push(format!(
                            "Process exited with code: {}",
                            status.code().unwrap_or(-1)
                        ));
                    }
                    // Clean up - stop polling
                    self.output_receiver = None;
                    self.child = None;
                }
                Ok(None) => {
                    // Still running
                }
                Err(e) => {
                    self.is_running = false;
                    self.output_lines
                        .push(format!("Error checking process status: {}", e));
                    // Clean up - stop polling
                    self.output_receiver = None;
                    self.child = None;
                }
            }
        } else if channel_disconnected {
            // Channel disconnected but child already cleaned up
            self.output_receiver = None;
        }
    }

    /// Request kill confirmation
    pub fn request_kill(&mut self) {
        if self.is_running {
            self.kill_confirm = true;
        }
    }

    /// Cancel kill confirmation
    pub fn cancel_kill(&mut self) {
        self.kill_confirm = false;
    }

    /// Kill the running process
    pub fn kill(&mut self) {
        self.kill_confirm = false;
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            self.is_running = false;
            self.output_lines.push(String::new());
            self.output_lines.push("Process killed by user".to_string());
        }
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.auto_scroll = false;
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.auto_scroll = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_to_top(&mut self) {
        self.auto_scroll = false;
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
        self.scroll_offset = u16::MAX;
    }

    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
        if self.auto_scroll {
            self.scroll_offset = u16::MAX;
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        f.render_widget(Clear, area);

        let status_indicator = if self.is_running {
            " [RUNNING]"
        } else {
            " [FINISHED]"
        };
        let auto_scroll_indicator = if self.auto_scroll {
            " [AUTO-SCROLL]"
        } else {
            ""
        };

        let block = Block::default()
            .title(format!(
                " {}{}{} ",
                self.title, status_indicator, auto_scroll_indicator
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if self.is_running {
                Color::Yellow
            } else {
                Color::Green
            }));

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Split into content and help bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Content
                Constraint::Length(1), // Help bar
            ])
            .split(inner);

        let visible_height = chunks[0].height as usize;
        let total_lines = self.output_lines.len();

        // Calculate scroll offset for auto-scroll
        let scroll_offset = if self.auto_scroll || self.scroll_offset == u16::MAX {
            total_lines.saturating_sub(visible_height) as u16
        } else {
            self.scroll_offset
                .min(total_lines.saturating_sub(visible_height) as u16)
        };

        // Content
        let lines: Vec<Line> = self
            .output_lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let style = if line.starts_with("[stderr]") {
                    Style::default().fg(Color::LightRed)
                } else if line.starts_with("Started:") || line.starts_with("Process ") {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                let line_num = format!("{:4} ", i + 1);
                Line::from(vec![
                    Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                    Span::styled(line.clone(), style),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .scroll((scroll_offset, 0))
            .wrap(Wrap { trim: false });
        f.render_widget(paragraph, chunks[0]);

        // Help bar
        let help_items = if self.kill_confirm {
            vec![
                Span::styled(
                    "Kill process? ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "y",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(": yes | "),
                Span::styled(
                    "n",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("/"),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(": cancel"),
            ]
        } else if self.is_running {
            vec![
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(": close | "),
                Span::styled("k", Style::default().fg(Color::Red)),
                Span::raw(": kill | "),
                Span::styled("a", Style::default().fg(Color::Yellow)),
                Span::raw(": toggle auto-scroll | "),
                Span::styled("g/G", Style::default().fg(Color::Yellow)),
                Span::raw(": top/bottom | "),
                Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
                Span::raw(": scroll"),
            ]
        } else {
            vec![
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(": close | "),
                Span::styled("g/G", Style::default().fg(Color::Yellow)),
                Span::raw(": top/bottom | "),
                Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
                Span::raw(": scroll"),
            ]
        };

        let help_line = Line::from(help_items);
        let help_para = Paragraph::new(help_line);
        f.render_widget(help_para, chunks[1]);
    }
}

impl Drop for ProcessViewer {
    fn drop(&mut self) {
        // Make sure to kill the process when the viewer is dropped
        if self.is_running {
            self.kill();
        }
    }
}

/// Helper function for arrow display
fn left_right_arrows() -> &'static str {
    "\u{2190}/\u{2192}"
}

fn up_down_arrows() -> &'static str {
    "\u{2191}/\u{2193}"
}
