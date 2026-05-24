use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::client::apis;
use crate::client::apis::configuration::BasicAuth;

mod api;
mod app;
pub mod components;
mod dag;
mod ui;

use app::App;
use components::StatusMessage;

/// Check if the Torc server is reachable by calling the ping endpoint
fn check_server_connection(
    base_url: &str,
    tls: &crate::client::apis::configuration::TlsConfig,
    basic_auth: &Option<BasicAuth>,
) -> bool {
    let mut config = crate::client::apis::configuration::Configuration::with_tls(tls.clone());
    config.base_path = base_url.to_string();
    config.basic_auth = basic_auth.clone();
    if let Err(e) = config.apply_cookie_header_from_env() {
        log::error!("Failed to apply TORC_COOKIE_HEADER: {e}");
        return false;
    }

    apis::system_api::ping(&config).is_ok()
}

pub fn run(
    standalone: bool,
    port: u16,
    database: Option<String>,
    tls_ca_cert: Option<String>,
    tls_insecure: bool,
    basic_auth: Option<BasicAuth>,
) -> Result<()> {
    env_logger::init();

    // Setup terminal first
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app - this will work even if server is not running
    let mut app = App::new_with_options(
        standalone,
        port,
        database,
        tls_ca_cert,
        tls_insecure,
        basic_auth,
    )?;

    // In standalone mode, auto-start the server
    if standalone {
        app.start_server_standalone();
        // Give server a moment to start, then try to connect
        std::thread::sleep(std::time::Duration::from_millis(500));
        if check_server_connection(&app.server_url, &app.tls, &app.basic_auth) {
            app.set_status(StatusMessage::success("Server started in standalone mode"));
            let _ = app.refresh_workflows();
            // Check server version
            app.check_server_version();
        } else {
            app.set_status(StatusMessage::warning(
                "Server starting... press 'r' to refresh when ready",
            ));
        }
    } else {
        // Check if server is running and show appropriate message
        if check_server_connection(&app.server_url, &app.tls, &app.basic_auth) {
            // Connected - check version
            app.check_server_version();
        } else {
            app.set_status(StatusMessage::warning(
                "No server connection. Press 'S' to start server or 'u' to change URL",
            ));
        }
    }

    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    use app::{DetailViewType, Focus, JobAction, PopupType, WorkflowAction};

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        // Check for auto-refresh
        let _ = app.check_auto_refresh();

        // Poll process viewer for new output
        app.poll_process_output();

        // Poll server process for new output
        app.poll_server_output();

        // Poll for SSE events
        app.poll_sse_events();

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            // Handle popup-specific keys first
            if app.has_popup() {
                match &app.popup {
                    Some(PopupType::Help) => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.close_popup(),
                        _ => {}
                    },
                    Some(PopupType::Confirmation { .. }) => match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            let _ = app.confirm_action();
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.cancel_action();
                        }
                        _ => {}
                    },
                    Some(PopupType::RecoverPrompt { .. }) => match key.code {
                        KeyCode::Esc => app.recover_prompt_cancel(),
                        KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                            app.recover_prompt_toggle_field();
                        }
                        KeyCode::Backspace => app.recover_prompt_backspace(),
                        KeyCode::Enter => {
                            let _ = app.recover_prompt_submit();
                        }
                        KeyCode::Char(c) => app.recover_prompt_add_char(c),
                        _ => {}
                    },
                    Some(PopupType::JobDetails(_)) => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.close_popup(),
                        KeyCode::Char('l') => {
                            // Close details and open logs
                            app.close_popup();
                            app.show_job_logs();
                        }
                        KeyCode::Down => {
                            if let Some(PopupType::JobDetails(popup)) = app.popup.as_mut() {
                                popup.scroll_down();
                            }
                        }
                        KeyCode::Up => {
                            if let Some(PopupType::JobDetails(popup)) = app.popup.as_mut() {
                                popup.scroll_up();
                            }
                        }
                        _ => {}
                    },
                    Some(PopupType::LogViewer(viewer)) => {
                        if viewer.is_searching {
                            match key.code {
                                KeyCode::Esc => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.cancel_search();
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.apply_search();
                                    }
                                }
                                KeyCode::Backspace => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.remove_search_char();
                                    }
                                }
                                KeyCode::Char(c) => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.add_search_char(c);
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => app.close_popup(),
                                KeyCode::Tab => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.toggle_tab();
                                    }
                                }
                                KeyCode::Down => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.scroll_down(1);
                                    }
                                }
                                KeyCode::Up => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.scroll_up(1);
                                    }
                                }
                                KeyCode::PageDown => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.scroll_down(20);
                                    }
                                }
                                KeyCode::PageUp => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.scroll_up(20);
                                    }
                                }
                                KeyCode::Char('g') => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.scroll_to_top();
                                    }
                                }
                                KeyCode::Char('G') => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.scroll_to_bottom(30); // approximate visible height
                                    }
                                }
                                KeyCode::Char('/') => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.start_search();
                                    }
                                }
                                KeyCode::Char('n') => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.next_match();
                                    }
                                }
                                KeyCode::Char('N') => {
                                    if let Some(PopupType::LogViewer(v)) = app.popup.as_mut() {
                                        v.prev_match();
                                    }
                                }
                                KeyCode::Char('y') => {
                                    // Show path in status bar for manual copy
                                    if let Some(PopupType::LogViewer(ref v)) = app.popup
                                        && let Some(path) = v.current_path()
                                    {
                                        app.set_status(components::StatusMessage::info(&format!(
                                            "Path: {}",
                                            path
                                        )));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(PopupType::FileViewer(viewer)) => {
                        if viewer.is_searching {
                            match key.code {
                                KeyCode::Esc => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.cancel_search();
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.apply_search();
                                    }
                                }
                                KeyCode::Backspace => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.remove_search_char();
                                    }
                                }
                                KeyCode::Char(c) => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.add_search_char(c);
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => app.close_popup(),
                                KeyCode::Down => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.scroll_down(1);
                                    }
                                }
                                KeyCode::Up => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.scroll_up(1);
                                    }
                                }
                                KeyCode::PageDown => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.scroll_down(20);
                                    }
                                }
                                KeyCode::PageUp => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.scroll_up(20);
                                    }
                                }
                                KeyCode::Char('g') => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.scroll_to_top();
                                    }
                                }
                                KeyCode::Char('G') => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.scroll_to_bottom(30);
                                    }
                                }
                                KeyCode::Char('/') => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.start_search();
                                    }
                                }
                                KeyCode::Char('n') => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.next_match();
                                    }
                                }
                                KeyCode::Char('N') => {
                                    if let Some(PopupType::FileViewer(v)) = app.popup.as_mut() {
                                        v.prev_match();
                                    }
                                }
                                KeyCode::Char('y') => {
                                    // Show path in status bar for manual copy
                                    if let Some(PopupType::FileViewer(ref v)) = app.popup {
                                        app.set_status(components::StatusMessage::info(&format!(
                                            "Path: {}",
                                            v.file_path
                                        )));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(PopupType::ProcessViewer(viewer)) => {
                        let is_server = viewer.title.contains("Server");
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                // If in kill confirmation mode, cancel it instead of closing
                                if viewer.kill_confirm {
                                    if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                        v.cancel_kill();
                                    }
                                } else if is_server {
                                    // For server, keep it running in background
                                    app.close_server_popup();
                                } else {
                                    // For other processes, closing kills them
                                    app.close_popup();
                                }
                            }
                            KeyCode::Char('k') => {
                                // Request kill confirmation
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.request_kill();
                                }
                            }
                            KeyCode::Char('y') => {
                                // Confirm kill if in confirmation mode
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut()
                                    && v.kill_confirm
                                {
                                    v.kill();
                                }
                            }
                            KeyCode::Char('n') => {
                                // Cancel kill confirmation
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.cancel_kill();
                                }
                            }
                            KeyCode::Char('a') => {
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.toggle_auto_scroll();
                                }
                            }
                            KeyCode::Down => {
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.scroll_down(1);
                                }
                            }
                            KeyCode::Up => {
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.scroll_up(1);
                                }
                            }
                            KeyCode::PageDown => {
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.scroll_down(20);
                                }
                            }
                            KeyCode::PageUp => {
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.scroll_up(20);
                                }
                            }
                            KeyCode::Char('g') => {
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.scroll_to_top();
                                }
                            }
                            KeyCode::Char('G') => {
                                if let Some(PopupType::ProcessViewer(v)) = app.popup.as_mut() {
                                    v.scroll_to_bottom();
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(PopupType::Error(_)) => match key.code {
                        KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => app.close_popup(),
                        _ => {}
                    },
                    None => {}
                }
                continue;
            }

            match app.focus {
                Focus::FilterInput => match key.code {
                    KeyCode::Esc => app.cancel_filter(),
                    KeyCode::Enter => app.apply_filter(),
                    KeyCode::Backspace => app.remove_filter_char(),
                    KeyCode::Tab => app.next_filter_column(),
                    KeyCode::BackTab => app.prev_filter_column(),
                    KeyCode::Char(c) => app.add_filter_char(c),
                    _ => {}
                },
                Focus::ServerUrlInput => match key.code {
                    KeyCode::Esc => app.cancel_server_url_input(),
                    KeyCode::Enter => {
                        if let Err(e) = app.apply_server_url() {
                            app.set_status(components::StatusMessage::error(&format!(
                                "Failed to connect: {}",
                                e
                            )));
                            app.cancel_server_url_input();
                        }
                    }
                    KeyCode::Backspace => app.remove_server_url_char(),
                    KeyCode::Char(c) => app.add_server_url_char(c),
                    _ => {}
                },
                Focus::WorkflowPathInput => match key.code {
                    KeyCode::Esc => app.cancel_workflow_path_input(),
                    KeyCode::Enter => {
                        if let Err(e) = app.apply_workflow_path() {
                            app.set_status(components::StatusMessage::error(&format!(
                                "Failed to create workflow: {}",
                                e
                            )));
                        }
                    }
                    KeyCode::Backspace => app.remove_workflow_path_char(),
                    KeyCode::Char(c) => app.add_workflow_path_char(c),
                    _ => {}
                },
                Focus::OutputDirInput => match key.code {
                    KeyCode::Esc => app.cancel_output_dir_input(),
                    KeyCode::Enter => {
                        app.apply_output_dir();
                    }
                    KeyCode::Backspace => app.remove_output_dir_char(),
                    KeyCode::Char(c) => app.add_output_dir_char(c),
                    _ => {}
                },
                Focus::Popup | Focus::RecoverPrompt => {
                    // Handled above (popup-specific match)
                }
                Focus::Workflows | Focus::Details => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('?') => app.show_help(),
                    KeyCode::Down => app.next_in_active_table(),
                    KeyCode::Up => app.previous_in_active_table(),
                    KeyCode::PageDown => app.page_down_in_active_table(),
                    KeyCode::PageUp => app.page_up_in_active_table(),
                    KeyCode::Char('g') => app.jump_to_top_in_active_table(),
                    KeyCode::Char('G') => app.jump_to_bottom_in_active_table(),
                    KeyCode::Enter => {
                        if app.focus == Focus::Workflows {
                            app.load_detail_data()?;
                        } else if app.detail_view == DetailViewType::Jobs {
                            app.show_job_details();
                        } else if app.detail_view == DetailViewType::Files {
                            app.show_file_contents();
                        }
                    }
                    KeyCode::Tab => app.next_detail_view(),
                    KeyCode::BackTab => app.previous_detail_view(),
                    // Page through the active list (Workflows / Jobs / Results /
                    // Compute Nodes) one TUI_PAGE_SIZE page at a time, on demand.
                    KeyCode::Char(']') => {
                        if let Err(err) = app.next_page() {
                            app.set_status(components::StatusMessage::error(&format!(
                                "Failed to load next page: {}",
                                err
                            )));
                        }
                    }
                    KeyCode::Char('[') => {
                        if let Err(err) = app.prev_page() {
                            app.set_status(components::StatusMessage::error(&format!(
                                "Failed to load previous page: {}",
                                err
                            )));
                        }
                    }
                    KeyCode::Char('r') => {
                        app.refresh_workflows()?;
                        if app.selected_workflow_id.is_some() {
                            app.reload_detail_data()?;
                        }
                        app.set_status(components::StatusMessage::info("Refreshed"));
                    }
                    KeyCode::Left | KeyCode::Right => {
                        app.toggle_focus();
                    }
                    KeyCode::Char('f') => {
                        app.start_filter();
                    }
                    KeyCode::Char('=') => {
                        app.filter_by_current_row();
                    }
                    KeyCode::Char('e') => {
                        if let Err(err) = app.jump_to_events() {
                            app.set_status(components::StatusMessage::error(&format!(
                                "Failed to load events: {}",
                                err
                            )));
                        }
                    }
                    KeyCode::Char('c') => {
                        app.clear_filter();
                    }
                    KeyCode::Char('u') => {
                        app.start_server_url_input();
                    }
                    KeyCode::Char('o') => {
                        app.start_output_dir_input();
                    }
                    KeyCode::Char('a') => {
                        app.toggle_show_all_users()?;
                    }
                    KeyCode::Char('A') => {
                        app.toggle_auto_refresh();
                    }
                    // Workflow actions
                    KeyCode::Char('n') => {
                        app.start_workflow_path_input();
                    }
                    KeyCode::Char('i') => {
                        app.request_workflow_action(WorkflowAction::Initialize);
                    }
                    KeyCode::Char('I') => {
                        app.request_workflow_action(WorkflowAction::Reinitialize);
                    }
                    KeyCode::Char('R') => {
                        app.request_workflow_action(WorkflowAction::Reset);
                    }
                    KeyCode::Char('x') => {
                        app.request_workflow_action(WorkflowAction::Run);
                    }
                    KeyCode::Char('s') => {
                        app.request_workflow_action(WorkflowAction::Submit);
                    }
                    KeyCode::Char('d') => {
                        app.request_workflow_action(WorkflowAction::Delete);
                    }
                    KeyCode::Char('C') => {
                        // On the Jobs detail tab, cancel the selected job;
                        // anywhere else, cancel the selected workflow.
                        if app.focus == Focus::Details && app.detail_view == DetailViewType::Jobs {
                            app.request_job_action(JobAction::Cancel);
                        } else {
                            app.request_workflow_action(WorkflowAction::Cancel);
                        }
                    }
                    KeyCode::Char('W') => {
                        app.request_workflow_action(WorkflowAction::Watch);
                    }
                    KeyCode::Char('V') => {
                        app.request_workflow_action(WorkflowAction::Recover);
                    }
                    KeyCode::Char('v') => {
                        app.request_workflow_action(WorkflowAction::RecoverDryRun);
                    }
                    // Server management
                    KeyCode::Char('S') => {
                        app.start_server();
                    }
                    KeyCode::Char('K') => {
                        app.stop_server();
                    }
                    KeyCode::Char('O') => {
                        app.show_server_output();
                    }
                    // Log viewing actions
                    KeyCode::Char('l') if app.focus == Focus::Details => match app.detail_view {
                        DetailViewType::Jobs => {
                            app.show_job_logs();
                        }
                        DetailViewType::ScheduledNodes => {
                            app.show_slurm_logs();
                        }
                        _ => {}
                    },
                    KeyCode::Char('t')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Jobs =>
                    {
                        app.request_job_action(JobAction::Terminate);
                    }
                    KeyCode::Char('y')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Jobs =>
                    {
                        app.request_job_action(JobAction::Retry);
                    }
                    // Sort the Results table by Peak Memory / Peak CPU. Each
                    // press cycles None → Desc → Asc → None.
                    KeyCode::Char('m')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Results =>
                    {
                        app.cycle_results_sort_peak_memory();
                    }
                    KeyCode::Char('p')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Results =>
                    {
                        app.cycle_results_sort_peak_cpu();
                    }
                    // Sort the Compute Nodes table: 1=ID, 2=Hostname, m=Peak
                    // Memory, p=Peak CPU. Each press cycles None → Desc → Asc.
                    KeyCode::Char('1')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::ComputeNodes =>
                    {
                        app.cycle_compute_nodes_sort_id();
                    }
                    KeyCode::Char('2')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::ComputeNodes =>
                    {
                        app.cycle_compute_nodes_sort_hostname();
                    }
                    KeyCode::Char('m')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::ComputeNodes =>
                    {
                        app.cycle_compute_nodes_sort_peak_memory();
                    }
                    KeyCode::Char('p')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::ComputeNodes =>
                    {
                        app.cycle_compute_nodes_sort_peak_cpu();
                    }
                    // Sort the Workflows list: 1=ID, 2=Name, 3=User.
                    KeyCode::Char('1') if app.focus == Focus::Workflows => {
                        app.cycle_workflows_sort_id();
                    }
                    KeyCode::Char('2') if app.focus == Focus::Workflows => {
                        app.cycle_workflows_sort_name();
                    }
                    KeyCode::Char('3') if app.focus == Focus::Workflows => {
                        app.cycle_workflows_sort_user();
                    }
                    KeyCode::Char('E')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Results =>
                    {
                        app.cycle_results_sort_runtime();
                    }
                    // Sort the Jobs table by column index 1=ID, 2=Name, 3=Status.
                    KeyCode::Char('1')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Jobs =>
                    {
                        app.cycle_jobs_sort_id();
                    }
                    KeyCode::Char('2')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Jobs =>
                    {
                        app.cycle_jobs_sort_name();
                    }
                    KeyCode::Char('3')
                        if app.focus == Focus::Details
                            && app.detail_view == DetailViewType::Jobs =>
                    {
                        app.cycle_jobs_sort_status();
                    }
                    _ => {}
                },
            }
        }
    }
}
