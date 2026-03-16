use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;

use anyhow::Result;
use petgraph::graph::NodeIndex;
use ratatui::widgets::TableState;

use crate::client::log_paths::{
    get_job_combined_path, get_job_stderr_path, get_job_stdout_path, get_slurm_stderr_path,
    get_slurm_stdout_path,
};
use crate::client::sse_client::SseEvent;
use crate::models::{
    FileModel, JobModel, ResultModel, ScheduledComputeNodesModel, SlurmStatsModel, WorkflowModel,
};

use crate::client::apis::configuration::{BasicAuth, TlsConfig};
use crate::client::config::TorcConfig;

use super::api::TorcClient;
use super::components::{
    ConfirmationDialog, ErrorDialog, FileViewer, JobDetailsPopup, LogViewer, ProcessViewer,
    StatusMessage,
};
use super::dag::{DagLayout, JobNode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetailViewType {
    Jobs,
    Files,
    Events,
    Results,
    ScheduledNodes,
    SlurmStats,
    Dag,
}

/// Actions that can be performed on workflows
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorkflowAction {
    Initialize,
    InitializeForce, // Initialize with --force (ignore missing input files)
    Reinitialize,
    ReinitializeForce, // Reinitialize with --force
    Reset,
    Run,
    Submit,
    Watch,       // Watch workflow with recovery
    WatchNoAuto, // Watch workflow without recovery
    Delete,
    Cancel,
}

impl WorkflowAction {
    pub fn confirmation_message(&self, workflow_name: &str) -> String {
        match self {
            Self::Initialize => format!("Initialize workflow '{}'?", workflow_name),
            Self::InitializeForce => {
                format!("Force initialize workflow '{}'?", workflow_name)
            }
            Self::Reinitialize => format!(
                "Re-initialize workflow '{}'?\nThis will reset all job statuses.",
                workflow_name
            ),
            Self::ReinitializeForce => {
                format!("Force re-initialize workflow '{}'?", workflow_name)
            }
            Self::Reset => format!(
                "Reset workflow '{}' status?\nThis will clear all job statuses and results.",
                workflow_name
            ),
            Self::Run => format!("Run workflow '{}' locally?", workflow_name),
            Self::Submit => format!("Submit workflow '{}' to scheduler?", workflow_name),
            Self::Watch => format!(
                "Watch workflow '{}' with recovery?\nThis will monitor and automatically retry failed jobs.",
                workflow_name
            ),
            Self::WatchNoAuto => format!(
                "Watch workflow '{}'?\nThis will monitor without automatic recovery.",
                workflow_name
            ),
            Self::Delete => format!(
                "DELETE workflow '{}'?\nThis action cannot be undone!",
                workflow_name
            ),
            Self::Cancel => format!("Cancel workflow '{}'?", workflow_name),
        }
    }

    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::Delete
                | Self::Reset
                | Self::Reinitialize
                | Self::ReinitializeForce
                | Self::InitializeForce
        )
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Initialize => "Initialize Workflow",
            Self::InitializeForce => "Initialize Workflow (Force)",
            Self::Reinitialize => "Re-initialize Workflow",
            Self::ReinitializeForce => "Re-initialize Workflow (Force)",
            Self::Reset => "Reset Workflow",
            Self::Run => "Run Workflow",
            Self::Submit => "Submit Workflow",
            Self::Watch => "Watch Workflow (Auto-Recovery)",
            Self::WatchNoAuto => "Watch Workflow",
            Self::Delete => "Delete Workflow",
            Self::Cancel => "Cancel Workflow",
        }
    }
}

/// Actions that can be performed on jobs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobAction {
    Cancel,
    Terminate,
    Retry,
}

impl JobAction {
    pub fn confirmation_message(&self, job_name: &str) -> String {
        match self {
            Self::Cancel => format!("Cancel job '{}'?", job_name),
            Self::Terminate => format!("Terminate job '{}'?", job_name),
            Self::Retry => format!("Retry job '{}'?", job_name),
        }
    }
}

/// Popup types that can be displayed
pub enum PopupType {
    Help,
    JobDetails(JobDetailsPopup),
    LogViewer(LogViewer),
    FileViewer(FileViewer),
    ProcessViewer(ProcessViewer),
    Confirmation {
        dialog: ConfirmationDialog,
        action: PendingAction,
    },
    Error(ErrorDialog),
}

/// Pending action waiting for confirmation
#[derive(Debug, Clone)]
pub enum PendingAction {
    Workflow(WorkflowAction, i64, String), // action, workflow_id, workflow_name
    Job(JobAction, i64, String),           // action, job_id, job_name
}

impl DetailViewType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Jobs => "▶ Jobs",
            Self::Files => "◫ Files",
            Self::Events => "⚡ Events",
            Self::Results => "✓ Results",
            Self::ScheduledNodes => "⊞ Nodes",
            Self::SlurmStats => "⚑ Slurm Stats",
            Self::Dag => "◇ DAG",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Jobs,
            Self::Files,
            Self::Events,
            Self::Results,
            Self::ScheduledNodes,
            Self::SlurmStats,
            Self::Dag,
        ]
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Jobs => Self::Files,
            Self::Files => Self::Events,
            Self::Events => Self::Results,
            Self::Results => Self::ScheduledNodes,
            Self::ScheduledNodes => Self::SlurmStats,
            Self::SlurmStats => Self::Dag,
            Self::Dag => Self::Jobs,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Jobs => Self::Dag,
            Self::Files => Self::Jobs,
            Self::Events => Self::Files,
            Self::Results => Self::Events,
            Self::ScheduledNodes => Self::Results,
            Self::SlurmStats => Self::ScheduledNodes,
            Self::Dag => Self::SlurmStats,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Workflows,
    Details,
    FilterInput,
    ServerUrlInput,
    WorkflowPathInput,
    OutputDirInput,
    Popup,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub column: String,
    pub value: String,
}

pub struct App {
    pub client: TorcClient,
    pub server_url: String,
    pub server_url_input: String,
    pub user_filter: Option<String>,
    pub workflows: Vec<WorkflowModel>,
    pub workflows_state: TableState,
    pub jobs: Vec<JobModel>,
    pub jobs_all: Vec<JobModel>,
    pub jobs_state: TableState,
    pub files: Vec<FileModel>,
    pub files_all: Vec<FileModel>,
    pub files_state: TableState,
    pub events: Vec<SseEvent>,
    pub events_all: Vec<SseEvent>,
    pub events_state: TableState,
    pub results: Vec<ResultModel>,
    pub results_all: Vec<ResultModel>,
    pub results_state: TableState,
    pub results_workflow_id: Option<i64>,
    pub exec_time_map: std::collections::HashMap<(i64, i64, i64), f64>,
    pub scheduled_nodes: Vec<ScheduledComputeNodesModel>,
    pub scheduled_nodes_all: Vec<ScheduledComputeNodesModel>,
    pub scheduled_nodes_state: TableState,
    pub slurm_stats: Vec<SlurmStatsModel>,
    pub slurm_stats_all: Vec<SlurmStatsModel>,
    pub slurm_stats_state: TableState,
    pub dag: Option<DagLayout>,
    pub detail_view: DetailViewType,
    pub selected_workflow_id: Option<i64>,
    pub focus: Focus,
    pub previous_focus: Focus,
    pub filter: Option<Filter>,
    pub filter_input: String,
    pub filter_column_index: usize,

    // New fields for enhanced functionality
    pub popup: Option<PopupType>,
    pub status_message: Option<StatusMessage>,
    pub workflow_path_input: String,
    pub auto_refresh: bool,
    pub last_refresh: std::time::Instant,

    // Server management
    pub server_process: Option<ProcessViewer>,
    pub standalone_database: Option<String>,

    // Version info
    pub version_mismatch: Option<crate::client::version_check::VersionCheckResult>,

    // User filtering
    pub current_user: String,
    pub show_all_users: bool,

    // SSE event streaming
    pub sse_receiver: Option<mpsc::Receiver<SseEvent>>,
    pub sse_thread: Option<JoinHandle<()>>,
    pub sse_workflow_id: Option<i64>,

    // TLS configuration
    pub tls: TlsConfig,

    // Authentication
    pub basic_auth: Option<BasicAuth>,

    // Output directory for log files
    pub output_dir: PathBuf,
    pub output_dir_input: String,
}

impl App {
    #[allow(dead_code)]
    pub fn new() -> Result<Self> {
        Self::new_with_options(false, 8080, None, None, false, None)
    }

    pub fn new_with_options(
        standalone: bool,
        port: u16,
        database: Option<String>,
        tls_ca_cert: Option<String>,
        tls_insecure: bool,
        basic_auth: Option<BasicAuth>,
    ) -> Result<Self> {
        let tls = TlsConfig {
            ca_cert_path: tls_ca_cert.as_ref().map(std::path::PathBuf::from),
            insecure: tls_insecure,
        };
        let client = TorcClient::new_with_tls(tls.clone(), basic_auth.clone())?;

        // In standalone mode, override the server URL to use the specified port
        let server_url = if standalone {
            format!("http://localhost:{}/torc-service/v1", port)
        } else {
            client.get_base_url().to_string()
        };

        // Load output directory from config
        let output_dir = TorcConfig::load().unwrap_or_default().client.run.output_dir;

        // Get current user from environment
        let current_user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let mut app = Self {
            client,
            server_url: server_url.clone(),
            server_url_input: String::new(),
            user_filter: Some(current_user.clone()),
            workflows: Vec::new(),
            workflows_state: TableState::default(),
            jobs: Vec::new(),
            jobs_all: Vec::new(),
            jobs_state: TableState::default(),
            files: Vec::new(),
            files_all: Vec::new(),
            files_state: TableState::default(),
            events: Vec::new(),
            events_all: Vec::new(),
            events_state: TableState::default(),
            results: Vec::new(),
            results_all: Vec::new(),
            results_state: TableState::default(),
            results_workflow_id: None,
            exec_time_map: std::collections::HashMap::new(),
            scheduled_nodes: Vec::new(),
            scheduled_nodes_all: Vec::new(),
            scheduled_nodes_state: TableState::default(),
            slurm_stats: Vec::new(),
            slurm_stats_all: Vec::new(),
            slurm_stats_state: TableState::default(),
            dag: None,
            detail_view: DetailViewType::Jobs,
            selected_workflow_id: None,
            focus: Focus::Workflows,
            previous_focus: Focus::Workflows,
            filter: None,
            filter_input: String::new(),
            filter_column_index: 0,
            popup: None,
            status_message: None,
            workflow_path_input: String::new(),
            auto_refresh: false,
            last_refresh: std::time::Instant::now(),
            server_process: None,
            standalone_database: database,
            version_mismatch: None,
            current_user,
            show_all_users: false,
            sse_receiver: None,
            sse_thread: None,
            sse_workflow_id: None,
            tls,
            basic_auth,
            output_dir,
            output_dir_input: String::new(),
        };

        // Update client to use the correct URL
        if standalone {
            app.client.set_base_url(&server_url);
        }

        // Try to load workflows, but don't fail if server is not available
        let _ = app.refresh_workflows();

        Ok(app)
    }

    pub fn refresh_workflows(&mut self) -> Result<()> {
        self.workflows = if let Some(ref user) = self.user_filter {
            self.client.list_workflows_for_user(user)?
        } else {
            self.client.list_workflows()?
        };

        if !self.workflows.is_empty() && self.workflows_state.selected().is_none() {
            self.workflows_state.select(Some(0));
        }
        Ok(())
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Workflows => Focus::Details,
            Focus::Details => Focus::Workflows,
            // Stay in current mode for input/popup states
            Focus::FilterInput => Focus::FilterInput,
            Focus::ServerUrlInput => Focus::ServerUrlInput,
            Focus::WorkflowPathInput => Focus::WorkflowPathInput,
            Focus::OutputDirInput => Focus::OutputDirInput,
            Focus::Popup => Focus::Popup,
        };
    }

    pub fn next_in_active_table(&mut self) {
        match self.focus {
            Focus::Workflows => {
                self.workflows_state.select(Some(
                    self.workflows_state
                        .selected()
                        .map(|i| (i + 1).min(self.workflows.len().saturating_sub(1)))
                        .unwrap_or(0),
                ));
            }
            Focus::Details => {
                let (state, len) = match self.detail_view {
                    DetailViewType::Jobs => (&mut self.jobs_state, self.jobs.len()),
                    DetailViewType::Files => (&mut self.files_state, self.files.len()),
                    DetailViewType::Events => (&mut self.events_state, self.events.len()),
                    DetailViewType::Results => (&mut self.results_state, self.results.len()),
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::Dag => return, // DAG view doesn't support table navigation
                };
                if len > 0 {
                    state.select(Some(
                        state
                            .selected()
                            .map(|i| (i + 1).min(len.saturating_sub(1)))
                            .unwrap_or(0),
                    ));
                }
            }
            // No navigation in input/popup modes
            Focus::FilterInput
            | Focus::ServerUrlInput
            | Focus::WorkflowPathInput
            | Focus::OutputDirInput
            | Focus::Popup => {}
        }
    }

    pub fn previous_in_active_table(&mut self) {
        match self.focus {
            Focus::Workflows => {
                self.workflows_state.select(Some(
                    self.workflows_state
                        .selected()
                        .map(|i| i.saturating_sub(1))
                        .unwrap_or(0),
                ));
            }
            Focus::Details => {
                let (state, len) = match self.detail_view {
                    DetailViewType::Jobs => (&mut self.jobs_state, self.jobs.len()),
                    DetailViewType::Files => (&mut self.files_state, self.files.len()),
                    DetailViewType::Events => (&mut self.events_state, self.events.len()),
                    DetailViewType::Results => (&mut self.results_state, self.results.len()),
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::Dag => return, // DAG view doesn't support table navigation
                };
                if len > 0 {
                    state.select(Some(
                        state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0),
                    ));
                }
            }
            // No navigation in input/popup modes
            Focus::FilterInput
            | Focus::ServerUrlInput
            | Focus::WorkflowPathInput
            | Focus::OutputDirInput
            | Focus::Popup => {}
        }
    }

    pub fn load_detail_data(&mut self) -> Result<()> {
        if let Some(idx) = self.workflows_state.selected()
            && let Some(workflow) = self.workflows.get(idx)
        {
            self.selected_workflow_id = workflow.id;
            if let Some(workflow_id) = workflow.id {
                // Clear any existing filter when loading new data
                self.filter = None;

                match self.detail_view {
                    DetailViewType::Jobs => {
                        self.jobs_all = self.client.list_jobs(workflow_id)?;
                        self.jobs = self.jobs_all.clone();
                        if !self.jobs.is_empty() {
                            self.jobs_state.select(Some(0));
                        }
                    }
                    DetailViewType::Files => {
                        self.files_all = self.client.list_files(workflow_id)?;
                        self.files = self.files_all.clone();
                        if !self.files.is_empty() {
                            self.files_state.select(Some(0));
                        }
                    }
                    DetailViewType::Events => {
                        // Start SSE connection for real-time events
                        self.start_sse_connection(workflow_id);
                    }
                    DetailViewType::Results => {
                        if self.results_workflow_id != Some(workflow_id) {
                            self.results_all = self.client.list_results(workflow_id)?;
                            self.results_workflow_id = Some(workflow_id);
                        }
                        self.results = self.results_all.clone();
                        if !self.results.is_empty() {
                            self.results_state.select(Some(0));
                        }
                    }
                    DetailViewType::ScheduledNodes => {
                        self.scheduled_nodes_all =
                            self.client.list_scheduled_compute_nodes(workflow_id)?;
                        self.scheduled_nodes = self.scheduled_nodes_all.clone();
                        if !self.scheduled_nodes.is_empty() {
                            self.scheduled_nodes_state.select(Some(0));
                        }
                    }
                    DetailViewType::SlurmStats => {
                        self.slurm_stats_all = self.client.list_slurm_stats(workflow_id)?;
                        self.slurm_stats = self.slurm_stats_all.clone();
                        if !self.slurm_stats.is_empty() {
                            self.slurm_stats_state.select(Some(0));
                        }
                        // Load results for CPU% computation if not already loaded
                        // for this workflow
                        if self.results_workflow_id != Some(workflow_id)
                            && let Ok(r) = self.client.list_results(workflow_id)
                        {
                            self.results_all = r;
                            self.results = self.results_all.clone();
                            self.results_workflow_id = Some(workflow_id);
                        }
                        self.rebuild_exec_time_map();
                    }
                    DetailViewType::Dag => {
                        // Load jobs if not already loaded
                        if self.jobs_all.is_empty() {
                            self.jobs_all = self.client.list_jobs(workflow_id)?;
                            self.jobs = self.jobs_all.clone();
                        }
                        // Build the DAG
                        self.build_dag_from_jobs();
                    }
                }
            }
        }
        Ok(())
    }

    /// Rebuild the cached exec_time_map from results_all.
    /// Called when results are loaded or refreshed so draw_slurm_stats_table
    /// can look up execution times without rebuilding the map every frame.
    fn rebuild_exec_time_map(&mut self) {
        self.exec_time_map = self
            .results_all
            .iter()
            .map(|r| {
                let attempt_id = r.attempt_id.unwrap_or(1);
                ((r.job_id, r.run_id, attempt_id), r.exec_time_minutes)
            })
            .collect();
    }

    pub fn next_detail_view(&mut self) {
        self.detail_view = self.detail_view.next();
        // Load data for the new tab if a workflow is selected
        if self.selected_workflow_id.is_some() {
            let _ = self.load_detail_data();
        }
    }

    pub fn previous_detail_view(&mut self) {
        self.detail_view = self.detail_view.previous();
        // Load data for the new tab if a workflow is selected
        if self.selected_workflow_id.is_some() {
            let _ = self.load_detail_data();
        }
    }

    pub fn start_filter(&mut self) {
        self.focus = Focus::FilterInput;
        self.filter_input.clear();
        self.filter_column_index = 0;
    }

    pub fn cancel_filter(&mut self) {
        self.focus = Focus::Details;
        self.filter_input.clear();
    }

    pub fn get_filter_columns(&self) -> Vec<&str> {
        match self.detail_view {
            DetailViewType::Jobs => vec!["Status", "Name", "Command"],
            DetailViewType::Files => vec!["Name", "Path"],
            DetailViewType::Events => vec!["Event Type", "Data"],
            DetailViewType::Results => vec!["Status", "Return Code"],
            DetailViewType::ScheduledNodes => vec!["Status", "Scheduler Type"],
            DetailViewType::SlurmStats => vec!["Job ID", "Slurm Job", "Nodes"],
            DetailViewType::Dag => vec![], // DAG view doesn't support filtering
        }
    }

    pub fn next_filter_column(&mut self) {
        let columns = self.get_filter_columns();
        self.filter_column_index = (self.filter_column_index + 1) % columns.len();
    }

    pub fn prev_filter_column(&mut self) {
        let columns = self.get_filter_columns();
        if self.filter_column_index == 0 {
            self.filter_column_index = columns.len() - 1;
        } else {
            self.filter_column_index -= 1;
        }
    }

    pub fn add_filter_char(&mut self, c: char) {
        self.filter_input.push(c);
    }

    pub fn remove_filter_char(&mut self) {
        self.filter_input.pop();
    }

    pub fn apply_filter(&mut self) {
        if self.filter_input.is_empty() {
            self.clear_filter();
            self.focus = Focus::Details;
            return;
        }

        let columns = self.get_filter_columns();
        let column = columns[self.filter_column_index].to_string();
        let value = self.filter_input.clone().to_lowercase();

        self.filter = Some(Filter {
            column: column.clone(),
            value: value.clone(),
        });

        match self.detail_view {
            DetailViewType::Jobs => {
                self.jobs = self
                    .jobs_all
                    .iter()
                    .filter(|job| match column.as_str() {
                        "Status" => job
                            .status
                            .as_ref()
                            .map(|s| format!("{:?}", s).to_lowercase().contains(&value))
                            .unwrap_or(false),
                        "Name" => job.name.to_lowercase().contains(&value),
                        "Command" => job.command.to_lowercase().contains(&value),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                if !self.jobs.is_empty() {
                    self.jobs_state.select(Some(0));
                } else {
                    self.jobs_state.select(None);
                }
            }
            DetailViewType::Files => {
                self.files = self
                    .files_all
                    .iter()
                    .filter(|file| match column.as_str() {
                        "Name" => file.name.to_lowercase().contains(&value),
                        "Path" => file.path.to_lowercase().contains(&value),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                if !self.files.is_empty() {
                    self.files_state.select(Some(0));
                } else {
                    self.files_state.select(None);
                }
            }
            DetailViewType::Events => {
                self.events = self
                    .events_all
                    .iter()
                    .filter(|event| match column.as_str() {
                        "Event Type" => event.event_type.to_lowercase().contains(&value),
                        "Data" => event.data.to_string().to_lowercase().contains(&value),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                if !self.events.is_empty() {
                    self.events_state.select(Some(0));
                } else {
                    self.events_state.select(None);
                }
            }
            DetailViewType::Results => {
                self.results = self
                    .results_all
                    .iter()
                    .filter(|result| match column.as_str() {
                        "Status" => format!("{:?}", result.status)
                            .to_lowercase()
                            .contains(&value),
                        "Return Code" => result.return_code.to_string().contains(&value),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                if !self.results.is_empty() {
                    self.results_state.select(Some(0));
                } else {
                    self.results_state.select(None);
                }
            }
            DetailViewType::ScheduledNodes => {
                self.scheduled_nodes = self
                    .scheduled_nodes_all
                    .iter()
                    .filter(|node| match column.as_str() {
                        "Status" => node.status.to_lowercase().contains(&value),
                        "Scheduler Type" => node.scheduler_type.to_lowercase().contains(&value),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                if !self.scheduled_nodes.is_empty() {
                    self.scheduled_nodes_state.select(Some(0));
                } else {
                    self.scheduled_nodes_state.select(None);
                }
            }
            DetailViewType::SlurmStats => {
                self.slurm_stats = self
                    .slurm_stats_all
                    .iter()
                    .filter(|stat| match column.as_str() {
                        "Job ID" => stat.job_id.to_string().contains(&value),
                        "Slurm Job" => stat
                            .slurm_job_id
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&value),
                        "Nodes" => stat
                            .node_list
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&value),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                if !self.slurm_stats.is_empty() {
                    self.slurm_stats_state.select(Some(0));
                } else {
                    self.slurm_stats_state.select(None);
                }
            }
            DetailViewType::Dag => {
                // DAG view doesn't support filtering
            }
        }

        self.focus = Focus::Details;
    }

    pub fn clear_filter(&mut self) {
        self.filter = None;
        match self.detail_view {
            DetailViewType::Jobs => {
                self.jobs = self.jobs_all.clone();
                if !self.jobs.is_empty() {
                    self.jobs_state.select(Some(0));
                }
            }
            DetailViewType::Files => {
                self.files = self.files_all.clone();
                if !self.files.is_empty() {
                    self.files_state.select(Some(0));
                }
            }
            DetailViewType::Events => {
                self.events = self.events_all.clone();
                if !self.events.is_empty() {
                    self.events_state.select(Some(0));
                }
            }
            DetailViewType::Results => {
                self.results = self.results_all.clone();
                if !self.results.is_empty() {
                    self.results_state.select(Some(0));
                }
            }
            DetailViewType::ScheduledNodes => {
                self.scheduled_nodes = self.scheduled_nodes_all.clone();
                if !self.scheduled_nodes.is_empty() {
                    self.scheduled_nodes_state.select(Some(0));
                }
            }
            DetailViewType::SlurmStats => {
                self.slurm_stats = self.slurm_stats_all.clone();
                if !self.slurm_stats.is_empty() {
                    self.slurm_stats_state.select(Some(0));
                }
            }
            DetailViewType::Dag => {
                // DAG view doesn't support filtering
            }
        }
    }

    pub fn start_server_url_input(&mut self) {
        self.focus = Focus::ServerUrlInput;
        self.server_url_input = self.server_url.clone();
    }

    pub fn cancel_server_url_input(&mut self) {
        self.focus = Focus::Workflows;
        self.server_url_input.clear();
    }

    pub fn add_server_url_char(&mut self, c: char) {
        self.server_url_input.push(c);
    }

    pub fn remove_server_url_char(&mut self) {
        self.server_url_input.pop();
    }

    pub fn apply_server_url(&mut self) -> Result<()> {
        if self.server_url_input.is_empty() {
            self.cancel_server_url_input();
            return Ok(());
        }

        // Create new client with updated URL, preserving authentication
        self.client = TorcClient::from_url_with_tls(
            self.server_url_input.clone(),
            self.tls.clone(),
            self.basic_auth.clone(),
        )?;
        self.server_url = self.server_url_input.clone();
        self.focus = Focus::Workflows;

        // Refresh workflows with new connection
        self.refresh_workflows()?;

        Ok(())
    }

    // === Output Directory Input ===

    pub fn start_output_dir_input(&mut self) {
        self.focus = Focus::OutputDirInput;
        self.output_dir_input = self.output_dir.display().to_string();
    }

    pub fn cancel_output_dir_input(&mut self) {
        self.focus = Focus::Workflows;
        self.output_dir_input.clear();
    }

    pub fn add_output_dir_char(&mut self, c: char) {
        self.output_dir_input.push(c);
    }

    pub fn remove_output_dir_char(&mut self) {
        self.output_dir_input.pop();
    }

    pub fn apply_output_dir(&mut self) {
        if self.output_dir_input.is_empty() {
            self.cancel_output_dir_input();
            return;
        }

        // Expand ~ to home directory
        let path = if self.output_dir_input.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(format!("{}{}", home, &self.output_dir_input[1..]))
        } else {
            PathBuf::from(&self.output_dir_input)
        };

        self.output_dir = path;
        self.focus = Focus::Workflows;
        self.set_status(StatusMessage::success(&format!(
            "Output directory set to: {}",
            self.output_dir.display()
        )));
    }

    pub fn get_current_user_display(&self) -> String {
        if self.show_all_users {
            "All Users".to_string()
        } else {
            self.user_filter
                .clone()
                .unwrap_or_else(|| "Unknown".to_string())
        }
    }

    pub fn toggle_show_all_users(&mut self) -> Result<()> {
        self.show_all_users = !self.show_all_users;
        if self.show_all_users {
            self.user_filter = None;
            self.set_status(StatusMessage::info("Showing all users"));
        } else {
            self.user_filter = Some(self.current_user.clone());
            self.set_status(StatusMessage::info(&format!(
                "Showing workflows for {}",
                self.current_user
            )));
        }
        self.refresh_workflows()?;
        Ok(())
    }

    pub fn build_dag_from_jobs(&mut self) {
        let mut dag = DagLayout::new();
        let mut job_id_to_node: HashMap<i64, NodeIndex> = HashMap::new();

        // Create nodes for all jobs
        for job in &self.jobs_all {
            if let Some(job_id) = job.id {
                let node = dag.add_node(JobNode {
                    id: job_id,
                    name: job.name.clone(),
                    status: job.status.as_ref().map(|s| format!("{:?}", s)),
                });
                job_id_to_node.insert(job_id, node);
            }
        }

        // Fetch blocking relationships from server
        if let Some(workflow_id) = self.selected_workflow_id {
            match self.client.list_job_dependencies(workflow_id) {
                Ok(dependencies) => {
                    // Add edges to graph
                    for dep in dependencies {
                        if let (Some(&from_node), Some(&to_node)) = (
                            job_id_to_node.get(&dep.depends_on_job_id),
                            job_id_to_node.get(&dep.job_id),
                        ) {
                            dag.add_edge(from_node, to_node);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load job dependencies: {}", e);
                    // Continue without edges - at least show nodes
                }
            }
        }

        dag.compute_layout();
        self.dag = Some(dag);
    }

    // === Popup Management ===

    pub fn show_help(&mut self) {
        self.previous_focus = self.focus;
        self.focus = Focus::Popup;
        self.popup = Some(PopupType::Help);
    }

    pub fn close_popup(&mut self) {
        // Check if we're closing a workflow run process viewer - if so, refresh data
        let should_refresh = if let Some(PopupType::ProcessViewer(ref viewer)) = self.popup {
            // Refresh if this was a workflow run (not server output)
            !viewer.title.contains("Server")
        } else {
            false
        };

        self.popup = None;
        self.focus = self.previous_focus;

        // Refresh workflow and job data after closing a workflow run viewer
        if should_refresh {
            if let Some(workflow_id) = self.selected_workflow_id {
                // Refresh jobs for the current workflow
                if let Ok(jobs) = self.client.list_jobs(workflow_id) {
                    self.jobs_all = jobs.clone();
                    self.jobs = jobs;
                    if !self.jobs.is_empty() {
                        self.jobs_state.select(Some(0));
                    }
                    // Clear any filter since we've refreshed all data
                    self.filter = None;
                }
                // Also refresh results
                if let Ok(results) = self.client.list_results(workflow_id) {
                    self.results_all = results.clone();
                    self.results = results;
                    self.results_workflow_id = Some(workflow_id);
                    if !self.results.is_empty() {
                        self.results_state.select(Some(0));
                    }
                    self.rebuild_exec_time_map();
                }
            }
            // Refresh workflow list to update status
            let _ = self.refresh_workflows();
        }
    }

    pub fn has_popup(&self) -> bool {
        self.popup.is_some()
    }

    /// Poll the process viewer for new output (called from event loop)
    pub fn poll_process_output(&mut self) {
        if let Some(PopupType::ProcessViewer(ref mut viewer)) = self.popup {
            viewer.poll_output();
        }
    }

    // === Status Messages ===

    pub fn set_status(&mut self, message: StatusMessage) {
        self.status_message = Some(message);
    }

    /// Check server version and set version_mismatch if there's a problem
    pub fn check_server_version(&mut self) {
        use crate::client::version_check;

        let mut config =
            crate::client::apis::configuration::Configuration::with_tls(self.tls.clone());
        config.base_path = self.server_url.clone();
        config.basic_auth = self.basic_auth.clone();

        let result = version_check::check_version(&config);

        // Only store if we got a server version and there's a mismatch
        if result.server_version.is_some() && result.severity.has_warning() {
            // Show status message based on severity
            match result.severity {
                version_check::VersionMismatchSeverity::Major => {
                    self.set_status(StatusMessage::error(&result.message));
                }
                version_check::VersionMismatchSeverity::Minor => {
                    self.set_status(StatusMessage::warning(&result.message));
                }
                version_check::VersionMismatchSeverity::Patch => {
                    // Subtle info for patch differences
                    self.set_status(StatusMessage::info(&result.message));
                }
                version_check::VersionMismatchSeverity::None => {}
            }
            self.version_mismatch = Some(result);
        } else {
            self.version_mismatch = None;
        }
    }

    /// Show an error dialog for long error messages
    pub fn show_error_dialog(&mut self, title: &str, message: &str) {
        self.popup = Some(PopupType::Error(ErrorDialog::new(title, message)));
    }

    // === Workflow Actions ===

    pub fn get_selected_workflow(&self) -> Option<&WorkflowModel> {
        self.workflows_state
            .selected()
            .and_then(|idx| self.workflows.get(idx))
    }

    pub fn request_workflow_action(&mut self, action: WorkflowAction) {
        if let Some(workflow) = self.get_selected_workflow() {
            if let Some(workflow_id) = workflow.id {
                let workflow_name = workflow.name.clone();
                let dialog = ConfirmationDialog::new(
                    action.title(),
                    &action.confirmation_message(&workflow_name),
                );
                let dialog = if action.is_destructive() {
                    dialog.destructive()
                } else {
                    dialog
                };

                self.previous_focus = self.focus;
                self.focus = Focus::Popup;
                self.popup = Some(PopupType::Confirmation {
                    dialog,
                    action: PendingAction::Workflow(action, workflow_id, workflow_name),
                });
            }
        } else {
            self.set_status(StatusMessage::warning("No workflow selected"));
        }
    }

    pub fn confirm_action(&mut self) -> Result<()> {
        if let Some(PopupType::Confirmation { action, .. }) = self.popup.take() {
            self.focus = self.previous_focus;
            match action {
                PendingAction::Workflow(workflow_action, workflow_id, workflow_name) => {
                    if let Err(e) =
                        self.execute_workflow_action(workflow_action, workflow_id, &workflow_name)
                    {
                        self.set_status(StatusMessage::error(&format!("Action error: {}", e)));
                    }
                }
                PendingAction::Job(job_action, job_id, job_name) => {
                    if let Err(e) = self.execute_job_action(job_action, job_id, &job_name) {
                        self.set_status(StatusMessage::error(&format!("Action error: {}", e)));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn cancel_action(&mut self) {
        self.popup = None;
        self.focus = self.previous_focus;
    }

    fn execute_workflow_action(
        &mut self,
        action: WorkflowAction,
        workflow_id: i64,
        workflow_name: &str,
    ) -> Result<()> {
        // Handle Run specially - spawn subprocess with output viewer
        if action == WorkflowAction::Run {
            return self.run_workflow_with_viewer(workflow_id, workflow_name);
        }

        // Handle Watch - spawn torc watch with output viewer
        if action == WorkflowAction::Watch {
            return self.watch_workflow_with_viewer(workflow_id, workflow_name, true);
        }
        if action == WorkflowAction::WatchNoAuto {
            return self.watch_workflow_with_viewer(workflow_id, workflow_name, false);
        }

        // Handle Initialize, Reinitialize and Reset via CLI commands (like torc-dash does)
        if action == WorkflowAction::Initialize {
            return self.initialize_workflow_cli(workflow_id, workflow_name);
        }
        if action == WorkflowAction::InitializeForce {
            return self.run_initialize_command(workflow_id, workflow_name, true);
        }
        if action == WorkflowAction::Reinitialize {
            return self.reinitialize_workflow_cli(workflow_id, workflow_name);
        }
        if action == WorkflowAction::ReinitializeForce {
            return self.run_reinitialize_command(workflow_id, workflow_name, true);
        }
        if action == WorkflowAction::Reset {
            return self.reset_workflow_cli(workflow_id, workflow_name);
        }

        let result = match action {
            WorkflowAction::Initialize => unreachable!(), // Handled above
            WorkflowAction::InitializeForce => unreachable!(), // Handled above
            WorkflowAction::Reinitialize => unreachable!(), // Handled above
            WorkflowAction::ReinitializeForce => unreachable!(), // Handled above
            WorkflowAction::Reset => unreachable!(),      // Handled above
            WorkflowAction::Run => unreachable!(),        // Handled above
            WorkflowAction::Watch => unreachable!(),      // Handled above
            WorkflowAction::WatchNoAuto => unreachable!(), // Handled above
            WorkflowAction::Submit => self.client.submit_workflow(workflow_id),
            WorkflowAction::Delete => self.client.delete_workflow(workflow_id),
            WorkflowAction::Cancel => self.client.cancel_workflow(workflow_id),
        };

        match result {
            Ok(_) => {
                let msg = match action {
                    WorkflowAction::Initialize => unreachable!(),
                    WorkflowAction::InitializeForce => unreachable!(),
                    WorkflowAction::Reinitialize => unreachable!(),
                    WorkflowAction::ReinitializeForce => unreachable!(),
                    WorkflowAction::Reset => unreachable!(),
                    WorkflowAction::Run => unreachable!(),
                    WorkflowAction::Watch => unreachable!(),
                    WorkflowAction::WatchNoAuto => unreachable!(),
                    WorkflowAction::Submit => {
                        format!("Workflow '{}' submitted to scheduler", workflow_name)
                    }
                    WorkflowAction::Delete => format!("Workflow '{}' deleted", workflow_name),
                    WorkflowAction::Cancel => format!("Workflow '{}' canceled", workflow_name),
                };
                self.set_status(StatusMessage::success(&msg));

                // Refresh workflows list after action
                if action == WorkflowAction::Delete {
                    self.refresh_workflows()?;
                } else {
                    // Reload the detail data to show updated status
                    let _ = self.load_detail_data();
                }
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to {} workflow: {}",
                    action.title().to_lowercase(),
                    e
                )));
            }
        }

        Ok(())
    }

    /// Initialize workflow using CLI command (following torc-dash pattern)
    /// First does a dry-run check, then prompts user if there are existing files
    fn initialize_workflow_cli(&mut self, workflow_id: i64, workflow_name: &str) -> Result<()> {
        self.set_status(StatusMessage::info(&format!(
            "Checking workflow '{}'...",
            workflow_name
        )));

        let exe_path = self.get_torc_exe_path();
        let url = self.client.get_base_url();
        let workflow_id_str = workflow_id.to_string();

        // First, do a dry-run check to see if there are existing output files
        let check_output = std::process::Command::new(&exe_path)
            .args([
                "--url",
                url,
                "-f",
                "json",
                "workflows",
                "initialize",
                &workflow_id_str,
                "--dry-run",
            ])
            .output();

        match check_output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);

                // Try to parse JSON response
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    let existing_count = json
                        .get("existing_output_file_count")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let missing_count = json
                        .get("missing_input_file_count")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let safe = json.get("safe").and_then(|v| v.as_bool()).unwrap_or(true);

                    // Check for missing input files (fatal error)
                    if !safe || missing_count > 0 {
                        let missing_files = json
                            .get("missing_input_files")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        self.set_status(StatusMessage::error(&format!(
                            "Cannot initialize: {} missing input file(s): {}",
                            missing_count, missing_files
                        )));
                        return Ok(());
                    }

                    // Check for existing output files (needs confirmation)
                    if existing_count > 0 {
                        let existing_files = json
                            .get("existing_output_files")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .take(5) // Show max 5 files
                                    .collect::<Vec<_>>()
                                    .join("\n  - ")
                            })
                            .unwrap_or_default();

                        let msg = if existing_count > 5 {
                            format!(
                                "Found {} existing output file(s):\n  - {}\n  ... and {} more.\n\nDelete these files and initialize?",
                                existing_count,
                                existing_files,
                                existing_count - 5
                            )
                        } else {
                            format!(
                                "Found {} existing output file(s):\n  - {}\n\nDelete these files and initialize?",
                                existing_count, existing_files
                            )
                        };

                        // Show confirmation dialog for force initialization
                        let dialog =
                            ConfirmationDialog::new("Initialize with Existing Files", &msg)
                                .destructive();
                        self.previous_focus = self.focus;
                        self.focus = Focus::Popup;
                        self.popup = Some(PopupType::Confirmation {
                            dialog,
                            action: PendingAction::Workflow(
                                WorkflowAction::InitializeForce,
                                workflow_id,
                                workflow_name.to_string(),
                            ),
                        });
                        return Ok(());
                    }
                }

                // No existing files or couldn't parse JSON - proceed with normal initialize
                self.run_initialize_command(workflow_id, workflow_name, false)
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to check initialization: {}",
                    e
                )));
                Ok(())
            }
        }
    }

    /// Run the actual initialize command (with or without --force)
    fn run_initialize_command(
        &mut self,
        workflow_id: i64,
        workflow_name: &str,
        force: bool,
    ) -> Result<()> {
        let exe_path = self.get_torc_exe_path();
        let url = self.client.get_base_url();
        let workflow_id_str = workflow_id.to_string();

        let mut args = vec![
            "--url",
            &url,
            "workflows",
            "initialize",
            "--no-prompts",
            &workflow_id_str,
        ];
        if force {
            args.push("--force");
        }

        let output = std::process::Command::new(&exe_path).args(&args).output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    self.set_status(StatusMessage::success(&format!(
                        "Workflow '{}' initialized",
                        workflow_name
                    )));
                    let _ = self.load_detail_data();
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let error_msg = if !stderr.trim().is_empty() {
                        stderr.trim().to_string()
                    } else if !stdout.trim().is_empty() {
                        stdout.trim().to_string()
                    } else {
                        "Unknown error".to_string()
                    };
                    self.set_status(StatusMessage::error(&format!(
                        "Initialize failed: {}",
                        error_msg
                    )));
                }
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to run initialize command: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    /// Reinitialize workflow using CLI command.
    /// Existing output files generate warnings but do not block the operation.
    fn reinitialize_workflow_cli(&mut self, workflow_id: i64, workflow_name: &str) -> Result<()> {
        self.run_reinitialize_command(workflow_id, workflow_name, false)
    }

    /// Run the actual reinitialize command (with or without --force)
    fn run_reinitialize_command(
        &mut self,
        workflow_id: i64,
        workflow_name: &str,
        force: bool,
    ) -> Result<()> {
        let exe_path = self.get_torc_exe_path();
        let url = self.client.get_base_url();
        let workflow_id_str = workflow_id.to_string();

        let mut args = vec!["--url", &url, "workflows", "reinitialize", &workflow_id_str];
        if force {
            args.push("--force");
        }

        let output = std::process::Command::new(&exe_path).args(&args).output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    self.set_status(StatusMessage::success(&format!(
                        "Workflow '{}' re-initialized",
                        workflow_name
                    )));
                    let _ = self.load_detail_data();
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let error_msg = if !stderr.trim().is_empty() {
                        stderr.trim().to_string()
                    } else if !stdout.trim().is_empty() {
                        stdout.trim().to_string()
                    } else {
                        "Unknown error".to_string()
                    };
                    self.set_status(StatusMessage::error(&format!(
                        "Re-initialize failed: {}",
                        error_msg
                    )));
                }
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to run reinitialize command: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    /// Reset workflow status using CLI command (following torc-dash pattern)
    fn reset_workflow_cli(&mut self, workflow_id: i64, workflow_name: &str) -> Result<()> {
        let exe_path = self.get_torc_exe_path();
        let url = self.client.get_base_url();
        let workflow_id_str = workflow_id.to_string();

        // Run CLI command: torc --url <url> workflows reset-status --no-prompts <workflow_id>
        let output = std::process::Command::new(&exe_path)
            .args([
                "--url",
                url,
                "workflows",
                "reset-status",
                "--no-prompts",
                &workflow_id_str,
            ])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    self.set_status(StatusMessage::success(&format!(
                        "Workflow '{}' status reset",
                        workflow_name
                    )));
                    let _ = self.load_detail_data();
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let error_msg = if !stderr.trim().is_empty() {
                        stderr.trim().to_string()
                    } else if !stdout.trim().is_empty() {
                        stdout.trim().to_string()
                    } else {
                        "Unknown error".to_string()
                    };
                    self.set_status(StatusMessage::error(&format!(
                        "Reset failed: {}",
                        error_msg
                    )));
                }
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to run reset-status command: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    /// Get the path to the torc executable
    fn get_torc_exe_path(&self) -> String {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "torc".to_string())
    }

    fn run_workflow_with_viewer(&mut self, workflow_id: i64, workflow_name: &str) -> Result<()> {
        let mut viewer = ProcessViewer::new(format!("Running: {}", workflow_name));

        let exe_path = self.get_torc_exe_path();

        // Build arguments - note: --url is a global option, must come before subcommand
        let workflow_id_str = workflow_id.to_string();
        let url = self.client.get_base_url();
        let args = vec!["--url", &url, "run", &workflow_id_str];

        match viewer.start(&exe_path, &args) {
            Ok(()) => {
                self.previous_focus = self.focus;
                self.focus = Focus::Popup;
                self.popup = Some(PopupType::ProcessViewer(viewer));
                self.set_status(StatusMessage::info(&format!(
                    "Running workflow '{}' locally...",
                    workflow_name
                )));
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to start workflow runner: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    fn watch_workflow_with_viewer(
        &mut self,
        workflow_id: i64,
        workflow_name: &str,
        recover: bool,
    ) -> Result<()> {
        let title = if recover {
            format!("Watching (recovery): {}", workflow_name)
        } else {
            format!("Watching: {}", workflow_name)
        };
        let mut viewer = ProcessViewer::new(title);

        let exe_path = self.get_torc_exe_path();

        // Build arguments - note: --url is a global option, must come before subcommand
        let workflow_id_str = workflow_id.to_string();
        let url = self.client.get_base_url();

        let args: Vec<&str> = if recover {
            vec![
                "--url",
                &url,
                "watch",
                &workflow_id_str,
                "--recover",
                "--show-job-counts",
            ]
        } else {
            vec![
                "--url",
                &url,
                "watch",
                &workflow_id_str,
                "--show-job-counts",
            ]
        };

        match viewer.start(&exe_path, &args) {
            Ok(()) => {
                self.previous_focus = self.focus;
                self.focus = Focus::Popup;
                self.popup = Some(PopupType::ProcessViewer(viewer));
                let msg = if recover {
                    format!("Watching workflow '{}' with recovery...", workflow_name)
                } else {
                    format!("Watching workflow '{}'...", workflow_name)
                };
                self.set_status(StatusMessage::info(&msg));
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to start watcher: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    // === Job Actions ===

    pub fn get_selected_job(&self) -> Option<&JobModel> {
        self.jobs_state
            .selected()
            .and_then(|idx| self.jobs.get(idx))
    }

    pub fn request_job_action(&mut self, action: JobAction) {
        if let Some(job) = self.get_selected_job() {
            if let Some(job_id) = job.id {
                let job_name = job.name.clone();
                let dialog = ConfirmationDialog::new(
                    match action {
                        JobAction::Cancel => "Cancel Job",
                        JobAction::Terminate => "Terminate Job",
                        JobAction::Retry => "Retry Job",
                    },
                    &action.confirmation_message(&job_name),
                );

                self.previous_focus = self.focus;
                self.focus = Focus::Popup;
                self.popup = Some(PopupType::Confirmation {
                    dialog,
                    action: PendingAction::Job(action, job_id, job_name),
                });
            }
        } else {
            self.set_status(StatusMessage::warning("No job selected"));
        }
    }

    fn execute_job_action(&mut self, action: JobAction, job_id: i64, job_name: &str) -> Result<()> {
        let result = match action {
            JobAction::Cancel => self.client.cancel_job(job_id),
            JobAction::Terminate => self.client.terminate_job(job_id),
            JobAction::Retry => self.client.retry_job(job_id),
        };

        match result {
            Ok(_) => {
                let msg = match action {
                    JobAction::Cancel => format!("Job '{}' canceled", job_name),
                    JobAction::Terminate => format!("Job '{}' terminated", job_name),
                    JobAction::Retry => format!("Job '{}' queued for retry", job_name),
                };
                self.set_status(StatusMessage::success(&msg));

                // Reload jobs to show updated status
                let _ = self.load_detail_data();
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to {:?} job: {}",
                    action, e
                )));
            }
        }

        Ok(())
    }

    pub fn show_job_details(&mut self) {
        if let Some(job) = self.get_selected_job() {
            let popup = JobDetailsPopup::new(
                job.id.unwrap_or(0),
                job.name.clone(),
                job.command.clone(),
                job.status
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_default(),
            );
            self.previous_focus = self.focus;
            self.focus = Focus::Popup;
            self.popup = Some(PopupType::JobDetails(popup));
        } else {
            self.set_status(StatusMessage::warning("No job selected"));
        }
    }

    // === Log Viewer ===

    pub fn show_job_logs(&mut self) {
        if let Some(job) = self.get_selected_job() {
            let job_id = job.id.unwrap_or(0);
            let job_name = job.name.clone();

            // Try to get log paths from results
            let mut viewer = LogViewer::new(job_id, job_name);

            // Try to load logs
            if let Err(e) = self.load_job_logs(&mut viewer) {
                self.set_status(StatusMessage::warning(&format!(
                    "Could not load logs: {}",
                    e
                )));
            }

            self.previous_focus = self.focus;
            self.focus = Focus::Popup;
            self.popup = Some(PopupType::LogViewer(viewer));
        } else {
            self.set_status(StatusMessage::warning("No job selected"));
        }
    }

    fn load_job_logs(&self, viewer: &mut LogViewer) -> Result<()> {
        // Try to find log files based on job results
        if let Some(workflow_id) = self.selected_workflow_id {
            let results = self.client.list_results(workflow_id)?;

            // Find the most recent result for this job
            // Sort by (run_id, attempt_id) to get the latest attempt of the latest run
            if let Some(result) = results
                .iter()
                .filter(|r| r.job_id == viewer.job_id)
                .max_by_key(|r| (r.run_id, r.attempt_id.unwrap_or(1)))
            {
                // Construct log paths using the standard path pattern
                let output_dir = &self.output_dir;

                let attempt_id = result.attempt_id.unwrap_or(1);
                let stdout_path = get_job_stdout_path(
                    output_dir,
                    workflow_id,
                    viewer.job_id,
                    result.run_id,
                    attempt_id,
                );
                let stderr_path = get_job_stderr_path(
                    output_dir,
                    workflow_id,
                    viewer.job_id,
                    result.run_id,
                    attempt_id,
                );
                let combined_path = get_job_combined_path(
                    output_dir,
                    workflow_id,
                    viewer.job_id,
                    result.run_id,
                    attempt_id,
                );

                // Try separate .o file first, then fall back to combined .log
                if let Ok(content) = std::fs::read_to_string(&stdout_path) {
                    viewer.stdout_path = Some(stdout_path);
                    viewer.stdout_content = content;
                } else if let Ok(content) = std::fs::read_to_string(&combined_path) {
                    viewer.stdout_path = Some(combined_path.clone());
                    viewer.stdout_content = content;
                } else {
                    viewer.stdout_path = Some(stdout_path.clone());
                    viewer.stdout_content = format!(
                        "Could not read file: {}\n\nThe file may not exist if:\n- The job has not run yet\n- The output directory is different\n- You are on a different system\n- The job used a stdio mode that doesn't capture stdout",
                        stdout_path
                    );
                }

                // Try separate .e file first, then fall back to combined .log
                if let Ok(content) = std::fs::read_to_string(&stderr_path) {
                    viewer.stderr_path = Some(stderr_path);
                    viewer.stderr_content = content;
                } else if let Ok(content) = std::fs::read_to_string(&combined_path) {
                    viewer.stderr_path = Some(combined_path);
                    viewer.stderr_content = content;
                } else {
                    viewer.stderr_path = Some(stderr_path.clone());
                    viewer.stderr_content = format!(
                        "Could not read file: {}\n\nThe file may not exist if:\n- The job has not run yet\n- The output directory is different\n- You are on a different system\n- The job used a stdio mode that doesn't capture stderr",
                        stderr_path
                    );
                }
            } else {
                viewer.stdout_content =
                    "No results found for this job.\n\nThe job may not have run yet.".to_string();
                viewer.stderr_content = "No results found for this job.".to_string();
            }
        }

        Ok(())
    }

    // === Slurm Log Viewer ===

    pub fn get_selected_scheduled_node(&self) -> Option<&ScheduledComputeNodesModel> {
        self.scheduled_nodes_state
            .selected()
            .and_then(|idx| self.scheduled_nodes.get(idx))
    }

    pub fn show_slurm_logs(&mut self) {
        if let Some(node) = self.get_selected_scheduled_node() {
            // Only show logs for Slurm nodes
            if node.scheduler_type.to_lowercase() != "slurm" {
                self.set_status(StatusMessage::warning(
                    "Log viewing is only available for Slurm scheduled nodes",
                ));
                return;
            }

            let scheduler_id = node.scheduler_id.to_string();
            let node_name = format!("Slurm Job {}", scheduler_id);

            // Use job_id of 0 and custom name since this is for a Slurm job, not a Torc job
            let mut viewer = LogViewer::new(0, node_name);

            // Load Slurm logs
            if let Err(e) = self.load_slurm_logs(&mut viewer, &scheduler_id) {
                self.set_status(StatusMessage::warning(&format!(
                    "Could not load Slurm logs: {}",
                    e
                )));
            }

            self.previous_focus = self.focus;
            self.focus = Focus::Popup;
            self.popup = Some(PopupType::LogViewer(viewer));
        } else {
            self.set_status(StatusMessage::warning("No scheduled node selected"));
        }
    }

    fn load_slurm_logs(&self, viewer: &mut LogViewer, scheduler_id: &str) -> Result<()> {
        let output_dir = &self.output_dir;

        let workflow_id = self.selected_workflow_id.unwrap_or(0);
        let stdout_path = get_slurm_stdout_path(output_dir, workflow_id, scheduler_id);
        let stderr_path = get_slurm_stderr_path(output_dir, workflow_id, scheduler_id);

        viewer.stdout_path = Some(stdout_path.clone());
        viewer.stderr_path = Some(stderr_path.clone());

        // Try to read stdout
        if let Ok(content) = std::fs::read_to_string(&stdout_path) {
            viewer.stdout_content = content;
        } else {
            viewer.stdout_content = format!(
                "Could not read file: {}\n\nThe file may not exist if:\n- The Slurm job has not run yet\n- The output directory is different\n- You are on a different system",
                stdout_path
            );
        }

        // Try to read stderr
        if let Ok(content) = std::fs::read_to_string(&stderr_path) {
            viewer.stderr_content = content;
        } else {
            viewer.stderr_content = format!(
                "Could not read file: {}\n\nThe file may not exist if:\n- The Slurm job has not run yet\n- The output directory is different\n- You are on a different system",
                stderr_path
            );
        }

        Ok(())
    }

    // === File Viewer ===

    pub fn get_selected_file(&self) -> Option<&FileModel> {
        self.files_state
            .selected()
            .and_then(|idx| self.files.get(idx))
    }

    pub fn show_file_contents(&mut self) {
        if let Some(file) = self.get_selected_file() {
            let file_name = file.name.clone();
            let file_path = file.path.clone();

            let mut viewer = FileViewer::new(file_name, file_path);

            // Try to load the file contents
            if let Err(e) = viewer.load_content() {
                self.set_status(StatusMessage::warning(&format!(
                    "Could not load file: {}",
                    e
                )));
            }

            self.previous_focus = self.focus;
            self.focus = Focus::Popup;
            self.popup = Some(PopupType::FileViewer(viewer));
        } else {
            self.set_status(StatusMessage::warning("No file selected"));
        }
    }

    // === Workflow Path Input (Create Workflow) ===

    pub fn start_workflow_path_input(&mut self) {
        self.previous_focus = self.focus;
        self.focus = Focus::WorkflowPathInput;
        self.workflow_path_input.clear();
    }

    pub fn cancel_workflow_path_input(&mut self) {
        self.focus = self.previous_focus;
        self.workflow_path_input.clear();
    }

    pub fn add_workflow_path_char(&mut self, c: char) {
        self.workflow_path_input.push(c);
    }

    pub fn remove_workflow_path_char(&mut self) {
        self.workflow_path_input.pop();
    }

    pub fn apply_workflow_path(&mut self) -> Result<()> {
        if self.workflow_path_input.is_empty() {
            self.cancel_workflow_path_input();
            return Ok(());
        }

        // Expand the path (handle ~ for home directory)
        let path = if self.workflow_path_input.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}{}", home, &self.workflow_path_input[1..])
        } else {
            self.workflow_path_input.clone()
        };

        self.focus = self.previous_focus;

        // Check if file exists
        if !std::path::Path::new(&path).exists() {
            self.set_status(StatusMessage::error(&format!("File not found: {}", path)));
            return Ok(());
        }

        // Try to create workflow from the file
        match self.client.create_workflow_from_file(&path) {
            Ok(workflow_id) => {
                self.set_status(StatusMessage::success(&format!(
                    "Workflow created with ID: {}",
                    workflow_id
                )));
                self.refresh_workflows()?;
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                // Use error dialog for long messages (> 80 chars) to avoid truncation
                if error_msg.len() > 80 {
                    self.show_error_dialog("Failed to Create Workflow", &error_msg);
                } else {
                    self.set_status(StatusMessage::error(&format!(
                        "Failed to create workflow: {}",
                        e
                    )));
                }
            }
        }

        self.workflow_path_input.clear();
        Ok(())
    }

    // === Auto-refresh ===

    pub fn toggle_auto_refresh(&mut self) {
        self.auto_refresh = !self.auto_refresh;
        if self.auto_refresh {
            self.set_status(StatusMessage::info("Auto-refresh enabled (30s interval)"));
        } else {
            self.set_status(StatusMessage::info("Auto-refresh disabled"));
        }
    }

    pub fn check_auto_refresh(&mut self) -> Result<()> {
        if self.auto_refresh && self.last_refresh.elapsed() > std::time::Duration::from_secs(30) {
            self.refresh_workflows()?;
            if self.selected_workflow_id.is_some() {
                let _ = self.load_detail_data();
            }
            self.last_refresh = std::time::Instant::now();
        }
        Ok(())
    }

    // === Server Management ===

    pub fn is_server_running(&self) -> bool {
        self.server_process
            .as_ref()
            .map(|p| p.is_running)
            .unwrap_or(false)
    }

    pub fn start_server(&mut self) {
        if self.is_server_running() {
            self.set_status(StatusMessage::warning("Server is already running"));
            return;
        }

        let mut viewer = ProcessViewer::new("Torc Server".to_string());

        // Find the torc-server binary - try several locations
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let server_paths = [
            // Same directory as current executable
            exe_dir
                .as_ref()
                .map(|d| d.join("torc-server").to_string_lossy().to_string()),
            // Current directory
            Some("./torc-server".to_string()),
            // In PATH
            Some("torc-server".to_string()),
        ];

        let mut server_path = None;
        for path_opt in server_paths.iter().flatten() {
            if std::path::Path::new(path_opt).exists() || !path_opt.contains('/') {
                server_path = Some(path_opt.clone());
                break;
            }
        }

        let server_path = match server_path {
            Some(p) => p,
            None => {
                self.set_status(StatusMessage::error(
                    "Could not find torc-server binary. Make sure it's in PATH or same directory.",
                ));
                return;
            }
        };

        // Extract port from current server URL to use for the new server
        // Default to 8080 if we can't parse it
        let port = self
            .server_url
            .split(':')
            .next_back()
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8080);

        let port_str = port.to_string();
        let args = vec!["run", "--port", &port_str];

        match viewer.start(&server_path, &args) {
            Ok(()) => {
                self.server_process = Some(viewer);
                self.set_status(StatusMessage::success(&format!(
                    "Server started on port {}",
                    port
                )));
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to start server: {}",
                    e
                )));
            }
        }
    }

    /// Start server in standalone mode with optional database path
    pub fn start_server_standalone(&mut self) {
        if self.is_server_running() {
            return;
        }

        let mut viewer = ProcessViewer::new("Torc Server (standalone)".to_string());

        // Find the torc-server binary
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        let server_paths = [
            exe_dir
                .as_ref()
                .map(|d| d.join("torc-server").to_string_lossy().to_string()),
            Some("./torc-server".to_string()),
            Some("torc-server".to_string()),
        ];

        let mut server_path = None;
        for path_opt in server_paths.iter().flatten() {
            if std::path::Path::new(path_opt).exists() || !path_opt.contains('/') {
                server_path = Some(path_opt.clone());
                break;
            }
        }

        let server_path = match server_path {
            Some(p) => p,
            None => {
                self.set_status(StatusMessage::error("Could not find torc-server binary"));
                return;
            }
        };

        // Extract port from server URL
        let port = self
            .server_url
            .split(':')
            .next_back()
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8080);

        let port_str = port.to_string();

        // Build args with optional database path
        let mut args = vec!["run", "--port", &port_str];
        let db_path;
        if let Some(ref db) = self.standalone_database {
            db_path = db.clone();
            args.push("--database");
            args.push(&db_path);
        }

        match viewer.start(&server_path, &args) {
            Ok(()) => {
                self.server_process = Some(viewer);
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to start server: {}",
                    e
                )));
            }
        }
    }

    pub fn stop_server(&mut self) {
        if let Some(ref mut viewer) = self.server_process {
            if viewer.is_running {
                viewer.kill();
                self.set_status(StatusMessage::info("Server stopped"));
            } else {
                self.set_status(StatusMessage::warning("Server is not running"));
            }
        } else {
            self.set_status(StatusMessage::warning("No server process to stop"));
        }
    }

    pub fn show_server_output(&mut self) {
        if let Some(viewer) = self.server_process.take() {
            self.previous_focus = self.focus;
            self.focus = Focus::Popup;
            self.popup = Some(PopupType::ProcessViewer(viewer));
        } else {
            self.set_status(StatusMessage::warning(
                "No server process. Press S to start one.",
            ));
        }
    }

    pub fn close_server_popup(&mut self) {
        // When closing the server popup, move the viewer back to server_process
        if let Some(PopupType::ProcessViewer(viewer)) = self.popup.take() {
            self.server_process = Some(viewer);
        }
        self.focus = self.previous_focus;
    }

    /// Poll the server process for new output (called from event loop)
    pub fn poll_server_output(&mut self) {
        if let Some(ref mut viewer) = self.server_process {
            viewer.poll_output();
        }
    }

    // === SSE Event Streaming ===

    /// Start SSE connection for real-time events from a workflow
    pub fn start_sse_connection(&mut self, workflow_id: i64) {
        // Stop existing connection if any
        self.stop_sse_connection();

        // Clear existing events when switching workflows
        self.events.clear();
        self.events_all.clear();
        self.events_state.select(None);

        // Create channel for receiving events
        let (tx, rx) = mpsc::channel();
        self.sse_receiver = Some(rx);
        self.sse_workflow_id = Some(workflow_id);

        // Get the base URL for SSE connection
        let base_url = self.server_url.clone();
        let tls = self.tls.clone();
        let basic_auth = self.basic_auth.clone();

        // Start background thread for SSE connection
        let handle = std::thread::spawn(move || {
            let mut config = crate::client::apis::configuration::Configuration::with_tls(tls);
            config.base_path = base_url;
            config.basic_auth = basic_auth;

            match crate::client::sse_client::SseConnection::connect(&config, workflow_id, None) {
                Ok(mut connection) => {
                    loop {
                        match connection.next_event() {
                            Ok(Some(event)) => {
                                if tx.send(event).is_err() {
                                    // Receiver dropped, exit thread
                                    break;
                                }
                            }
                            Ok(None) => {
                                // Connection closed
                                break;
                            }
                            Err(_) => {
                                // Error reading, exit thread
                                break;
                            }
                        }
                    }
                }
                Err(_) => {
                    // Failed to connect, thread exits
                }
            }
        });

        self.sse_thread = Some(handle);
        self.set_status(StatusMessage::info(
            "SSE connection started - waiting for events...",
        ));
    }

    /// Stop the SSE connection
    pub fn stop_sse_connection(&mut self) {
        // Drop the receiver to signal the thread to stop
        self.sse_receiver = None;
        self.sse_workflow_id = None;

        // Wait for thread to finish (with timeout)
        if let Some(handle) = self.sse_thread.take() {
            // Don't block, just let it finish in background
            std::thread::spawn(move || {
                let _ = handle.join();
            });
        }
    }

    /// Poll for new SSE events (called from event loop)
    pub fn poll_sse_events(&mut self) {
        if let Some(ref receiver) = self.sse_receiver {
            // Try to receive events without blocking
            while let Ok(event) = receiver.try_recv() {
                // Add event to the beginning (newest first)
                self.events.insert(0, event.clone());
                self.events_all.insert(0, event);

                // Select first event if nothing selected
                if self.events_state.selected().is_none() && !self.events.is_empty() {
                    self.events_state.select(Some(0));
                }
            }
        }
    }
}
