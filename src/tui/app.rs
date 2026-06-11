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
    ComputeNodeModel, FileModel, JobModel, JobStatus, ResultModel, ScheduledComputeNodesModel,
    SlurmStatsModel, UserDataModel, WorkflowModel,
};

use crate::client::apis::configuration::{BasicAuth, TlsConfig};
use crate::client::config::TorcConfig;

use super::api::TorcClient;
use super::components::{
    ConfirmationDialog, ErrorDialog, FileViewer, JobDetailsPopup, LogViewer, ProcessViewer,
    RecoverPromptDialog, StatusMessage, UserDataDetailsPopup, WorkflowDetailsPopup,
};
use super::dag::{DagLayout, JobNode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetailViewType {
    Summary,
    Jobs,
    Files,
    UserData,
    Events,
    Results,
    ComputeNodes,
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
    Watch,         // Watch workflow with recovery
    WatchNoAuto,   // Watch workflow without recovery
    Recover,       // One-shot recovery: adjust resources and resubmit failed jobs
    RecoverDryRun, // Preview what recovery would do without applying changes
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
            Self::Recover => format!(
                "Recover workflow '{}'?\n\
                 Diagnoses failures, increases memory/runtime for OOM/timeout jobs, \
                 resets failed jobs, and resubmits Slurm allocations.\n\
                 Workflow must be complete with no active workers.",
                workflow_name
            ),
            Self::RecoverDryRun => format!(
                "Preview recovery for workflow '{}'?\n\
                 Shows the proposed resource adjustments and Slurm scheduler plan \
                 without making any changes.",
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
                | Self::Recover
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
            Self::Recover => "Recover Workflow",
            Self::RecoverDryRun => "Recover Workflow (Dry Run)",
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
    ResetStatus,
}

impl JobAction {
    pub fn confirmation_message(&self, job_name: &str) -> String {
        match self {
            Self::Cancel => format!("Cancel job '{}'?", job_name),
            Self::Terminate => format!("Terminate job '{}'?", job_name),
            Self::Retry => format!("Retry job '{}'?", job_name),
            Self::ResetStatus => format!(
                "Reset job '{}' to uninitialized for rerun?\n\
                 Downstream dependents are reset when the workflow is re-initialized ('I').",
                job_name
            ),
        }
    }
}

/// Popup types that can be displayed
pub enum PopupType {
    Help,
    JobDetails(JobDetailsPopup),
    UserDataDetails(UserDataDetailsPopup),
    WorkflowDetails(WorkflowDetailsPopup),
    LogViewer(LogViewer),
    FileViewer(FileViewer),
    ProcessViewer(ProcessViewer),
    Confirmation {
        dialog: ConfirmationDialog,
        action: PendingAction,
    },
    RecoverPrompt {
        dialog: RecoverPromptDialog,
        workflow_id: i64,
        workflow_name: String,
        dry_run: bool,
    },
    Error(ErrorDialog),
}

/// Pending action waiting for confirmation
#[derive(Debug, Clone)]
pub enum PendingAction {
    Workflow(WorkflowAction, i64, String), // action, workflow_id, workflow_name
    Job(JobAction, i64, String),           // action, job_id, job_name
    JobsResetStatus(Vec<i64>),             // multi-selection reset (job_ids)
}

impl DetailViewType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Summary => "◆ Summary",
            Self::Jobs => "▶ Jobs",
            Self::Files => "◫ Files",
            Self::UserData => "◈ User Data",
            Self::Events => "⚡ Events",
            Self::Results => "✓ Results",
            Self::ComputeNodes => "▣ Compute",
            Self::ScheduledNodes => "⊞ Nodes",
            Self::SlurmStats => "⚑ Slurm Stats",
            Self::Dag => "◇ DAG",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Summary,
            Self::Jobs,
            Self::Files,
            Self::UserData,
            Self::Events,
            Self::Results,
            Self::ComputeNodes,
            Self::ScheduledNodes,
            Self::SlurmStats,
            Self::Dag,
        ]
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Summary => Self::Jobs,
            Self::Jobs => Self::Files,
            Self::Files => Self::UserData,
            Self::UserData => Self::Events,
            Self::Events => Self::Results,
            Self::Results => Self::ComputeNodes,
            Self::ComputeNodes => Self::ScheduledNodes,
            Self::ScheduledNodes => Self::SlurmStats,
            Self::SlurmStats => Self::Dag,
            Self::Dag => Self::Summary,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::Summary => Self::Dag,
            Self::Jobs => Self::Summary,
            Self::Files => Self::Jobs,
            Self::UserData => Self::Files,
            Self::Events => Self::UserData,
            Self::Results => Self::Events,
            Self::ComputeNodes => Self::Results,
            Self::ScheduledNodes => Self::ComputeNodes,
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
    RecoverPrompt,
    Popup,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub column: String,
    pub value: String,
}

/// Which table the active filter applies to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterTarget {
    Workflows,
    Details,
}

/// Number of rows to advance/retreat for PageDown/PageUp.
pub const PAGE_STEP: usize = 10;

/// Number of records the TUI fetches per page for the paginated list views
/// (Workflows, Jobs, Results, Compute Nodes). Lists are loaded on demand one
/// page at a time; `]` / `[` move to the next / previous page.
pub const TUI_PAGE_SIZE: i64 = 250;

/// Sort state for the Results detail table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultsSort {
    None,
    IdDesc,
    IdAsc,
    JobIdDesc,
    JobIdAsc,
    ReturnDesc,
    ReturnAsc,
    CompletionDesc,
    CompletionAsc,
    PeakMemoryDesc,
    PeakMemoryAsc,
    PeakCpuDesc,
    PeakCpuAsc,
    RuntimeDesc,
    RuntimeAsc,
}

/// Sort state for the Jobs detail table. Number keys 1..3 cycle a single
/// column at a time (None → Desc → Asc → None).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobsSort {
    None,
    IdDesc,
    IdAsc,
    NameDesc,
    NameAsc,
    StatusDesc,
    StatusAsc,
}

impl JobsSort {
    pub fn cycle_id(self) -> Self {
        match self {
            Self::IdDesc => Self::IdAsc,
            Self::IdAsc => Self::None,
            _ => Self::IdDesc,
        }
    }
    pub fn cycle_name(self) -> Self {
        match self {
            Self::NameDesc => Self::NameAsc,
            Self::NameAsc => Self::None,
            _ => Self::NameDesc,
        }
    }
    pub fn cycle_status(self) -> Self {
        match self {
            Self::StatusDesc => Self::StatusAsc,
            Self::StatusAsc => Self::None,
            _ => Self::StatusDesc,
        }
    }
    pub fn id_indicator(self) -> &'static str {
        match self {
            Self::IdDesc => " ↓",
            Self::IdAsc => " ↑",
            _ => "",
        }
    }
    pub fn name_indicator(self) -> &'static str {
        match self {
            Self::NameDesc => " ↓",
            Self::NameAsc => " ↑",
            _ => "",
        }
    }
    pub fn status_indicator(self) -> &'static str {
        match self {
            Self::StatusDesc => " ↓",
            Self::StatusAsc => " ↑",
            _ => "",
        }
    }
}

impl ResultsSort {
    pub fn cycle_id(self) -> Self {
        match self {
            Self::IdDesc => Self::IdAsc,
            Self::IdAsc => Self::None,
            _ => Self::IdDesc,
        }
    }

    pub fn cycle_job_id(self) -> Self {
        match self {
            Self::JobIdDesc => Self::JobIdAsc,
            Self::JobIdAsc => Self::None,
            _ => Self::JobIdDesc,
        }
    }

    pub fn cycle_return(self) -> Self {
        match self {
            Self::ReturnDesc => Self::ReturnAsc,
            Self::ReturnAsc => Self::None,
            _ => Self::ReturnDesc,
        }
    }

    pub fn cycle_completion(self) -> Self {
        match self {
            Self::CompletionDesc => Self::CompletionAsc,
            Self::CompletionAsc => Self::None,
            _ => Self::CompletionDesc,
        }
    }

    /// Cycle: None → Desc → Asc → None for the Peak Memory column. If currently
    /// sorting by another column, jump to PeakMemoryDesc.
    pub fn cycle_peak_memory(self) -> Self {
        match self {
            Self::PeakMemoryDesc => Self::PeakMemoryAsc,
            Self::PeakMemoryAsc => Self::None,
            _ => Self::PeakMemoryDesc,
        }
    }

    pub fn cycle_peak_cpu(self) -> Self {
        match self {
            Self::PeakCpuDesc => Self::PeakCpuAsc,
            Self::PeakCpuAsc => Self::None,
            _ => Self::PeakCpuDesc,
        }
    }

    pub fn cycle_runtime(self) -> Self {
        match self {
            Self::RuntimeDesc => Self::RuntimeAsc,
            Self::RuntimeAsc => Self::None,
            _ => Self::RuntimeDesc,
        }
    }

    pub fn id_indicator(self) -> &'static str {
        match self {
            Self::IdDesc => " ↓",
            Self::IdAsc => " ↑",
            _ => "",
        }
    }

    pub fn job_id_indicator(self) -> &'static str {
        match self {
            Self::JobIdDesc => " ↓",
            Self::JobIdAsc => " ↑",
            _ => "",
        }
    }

    pub fn return_indicator(self) -> &'static str {
        match self {
            Self::ReturnDesc => " ↓",
            Self::ReturnAsc => " ↑",
            _ => "",
        }
    }

    pub fn completion_indicator(self) -> &'static str {
        match self {
            Self::CompletionDesc => " ↓",
            Self::CompletionAsc => " ↑",
            _ => "",
        }
    }

    /// Returns the arrow indicator for the Peak Memory column header.
    pub fn peak_memory_indicator(self) -> &'static str {
        match self {
            Self::PeakMemoryDesc => " ↓",
            Self::PeakMemoryAsc => " ↑",
            _ => "",
        }
    }

    pub fn peak_cpu_indicator(self) -> &'static str {
        match self {
            Self::PeakCpuDesc => " ↓",
            Self::PeakCpuAsc => " ↑",
            _ => "",
        }
    }

    pub fn runtime_indicator(self) -> &'static str {
        match self {
            Self::RuntimeDesc => " ↓",
            Self::RuntimeAsc => " ↑",
            _ => "",
        }
    }
}

/// Sort state for the Workflows list. Number keys 1..3 cycle a single column
/// at a time (None → Desc → Asc → None), mirroring [`JobsSort`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowsSort {
    None,
    IdDesc,
    IdAsc,
    NameDesc,
    NameAsc,
    UserDesc,
    UserAsc,
}

impl WorkflowsSort {
    pub fn cycle_id(self) -> Self {
        match self {
            Self::IdDesc => Self::IdAsc,
            Self::IdAsc => Self::None,
            _ => Self::IdDesc,
        }
    }
    pub fn cycle_name(self) -> Self {
        match self {
            Self::NameDesc => Self::NameAsc,
            Self::NameAsc => Self::None,
            _ => Self::NameDesc,
        }
    }
    pub fn cycle_user(self) -> Self {
        match self {
            Self::UserDesc => Self::UserAsc,
            Self::UserAsc => Self::None,
            _ => Self::UserDesc,
        }
    }
    pub fn id_indicator(self) -> &'static str {
        match self {
            Self::IdDesc => " ↓",
            Self::IdAsc => " ↑",
            _ => "",
        }
    }
    pub fn name_indicator(self) -> &'static str {
        match self {
            Self::NameDesc => " ↓",
            Self::NameAsc => " ↑",
            _ => "",
        }
    }
    pub fn user_indicator(self) -> &'static str {
        match self {
            Self::UserDesc => " ↓",
            Self::UserAsc => " ↑",
            _ => "",
        }
    }
}

/// Sort state for the Compute Nodes detail table. Keys 1=ID, 2=Hostname cycle
/// like [`JobsSort`]; `m`/`p` cycle Peak Memory / Peak CPU like [`ResultsSort`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeNodesSort {
    None,
    IdDesc,
    IdAsc,
    HostnameDesc,
    HostnameAsc,
    PeakCpuDesc,
    PeakCpuAsc,
    PeakMemoryDesc,
    PeakMemoryAsc,
}

impl ComputeNodesSort {
    pub fn cycle_id(self) -> Self {
        match self {
            Self::IdDesc => Self::IdAsc,
            Self::IdAsc => Self::None,
            _ => Self::IdDesc,
        }
    }
    pub fn cycle_hostname(self) -> Self {
        match self {
            Self::HostnameDesc => Self::HostnameAsc,
            Self::HostnameAsc => Self::None,
            _ => Self::HostnameDesc,
        }
    }
    pub fn cycle_peak_cpu(self) -> Self {
        match self {
            Self::PeakCpuDesc => Self::PeakCpuAsc,
            Self::PeakCpuAsc => Self::None,
            _ => Self::PeakCpuDesc,
        }
    }
    pub fn cycle_peak_memory(self) -> Self {
        match self {
            Self::PeakMemoryDesc => Self::PeakMemoryAsc,
            Self::PeakMemoryAsc => Self::None,
            _ => Self::PeakMemoryDesc,
        }
    }
    pub fn id_indicator(self) -> &'static str {
        match self {
            Self::IdDesc => " ↓",
            Self::IdAsc => " ↑",
            _ => "",
        }
    }
    pub fn hostname_indicator(self) -> &'static str {
        match self {
            Self::HostnameDesc => " ↓",
            Self::HostnameAsc => " ↑",
            _ => "",
        }
    }
    pub fn peak_cpu_indicator(self) -> &'static str {
        match self {
            Self::PeakCpuDesc => " ↓",
            Self::PeakCpuAsc => " ↑",
            _ => "",
        }
    }
    pub fn peak_memory_indicator(self) -> &'static str {
        match self {
            Self::PeakMemoryDesc => " ↓",
            Self::PeakMemoryAsc => " ↑",
            _ => "",
        }
    }
}

/// Aggregated high-level information about a single workflow, computed from
/// list_jobs + get_workflow + is_workflow_complete and rendered by the
/// Summary detail view.
#[derive(Debug, Clone)]
pub struct WorkflowSummary {
    pub workflow_id: i64,
    pub workflow_name: String,
    pub workflow_user: String,
    pub description: Option<String>,
    pub is_complete: bool,
    pub is_canceled: bool,
    pub total_jobs: usize,
    /// Counts indexed by `JobStatus as usize` (0 = Uninitialized .. 10 = PendingFailed).
    pub counts: [usize; 11],
}

pub struct App {
    pub client: TorcClient,
    pub server_url: String,
    pub server_url_input: String,
    pub user_filter: Option<String>,
    pub workflows: Vec<WorkflowModel>,
    pub workflows_all: Vec<WorkflowModel>,
    pub workflows_state: TableState,
    pub workflows_sort: WorkflowsSort,
    /// Offset of the currently-loaded Workflows page (multiple of TUI_PAGE_SIZE).
    pub workflows_offset: i64,
    /// True when the last Workflows fetch filled a full page (a next page may exist).
    pub workflows_has_more: bool,
    pub jobs: Vec<JobModel>,
    pub jobs_all: Vec<JobModel>,
    pub jobs_workflow_id: Option<i64>,
    pub jobs_state: TableState,
    pub jobs_sort: JobsSort,
    /// Job IDs marked on the Jobs tab (Space / '*') for a multi-job
    /// reset-status. Cleared when the selected workflow changes; stale IDs
    /// (e.g. from a row no longer listed after a filter change) are ignored
    /// at action time by intersecting with the currently-listed jobs.
    pub selected_job_ids: std::collections::HashSet<i64>,
    /// Offset of the currently-loaded Jobs page on the Jobs detail tab.
    pub jobs_offset: i64,
    /// True when the last Jobs-tab fetch filled a full page.
    pub jobs_has_more: bool,
    /// Wall-clock time of the most recent `jobs_all` fetch. The Jobs table's
    /// Elapsed column is computed relative to this snapshot instead of
    /// `Utc::now()` so it doesn't keep ticking up against stale rows whose
    /// server-side status has since moved off Running.
    pub jobs_fetched_at: Option<chrono::DateTime<chrono::Utc>>,
    pub files: Vec<FileModel>,
    pub files_all: Vec<FileModel>,
    pub files_state: TableState,
    pub user_data: Vec<UserDataModel>,
    pub user_data_all: Vec<UserDataModel>,
    pub user_data_state: TableState,
    /// Offset of the currently-loaded User Data page.
    pub user_data_offset: i64,
    /// True when the last User Data fetch filled a full page.
    pub user_data_has_more: bool,
    pub events: Vec<SseEvent>,
    pub events_all: Vec<SseEvent>,
    pub events_state: TableState,
    pub results: Vec<ResultModel>,
    pub results_all: Vec<ResultModel>,
    pub results_state: TableState,
    pub results_workflow_id: Option<i64>,
    pub results_sort: ResultsSort,
    /// Offset of the currently-loaded Results page on the Results detail tab.
    pub results_offset: i64,
    /// True when the last Results-tab fetch filled a full page.
    pub results_has_more: bool,
    pub exec_time_map: std::collections::HashMap<(i64, i64, i64), f64>,
    pub compute_nodes: Vec<ComputeNodeModel>,
    pub compute_nodes_all: Vec<ComputeNodeModel>,
    pub compute_nodes_state: TableState,
    pub compute_nodes_sort: ComputeNodesSort,
    /// Offset of the currently-loaded Compute Nodes page.
    pub compute_nodes_offset: i64,
    /// True when the last Compute Nodes fetch filled a full page.
    pub compute_nodes_has_more: bool,
    pub scheduled_nodes: Vec<ScheduledComputeNodesModel>,
    pub scheduled_nodes_all: Vec<ScheduledComputeNodesModel>,
    pub scheduled_nodes_state: TableState,
    pub slurm_stats: Vec<SlurmStatsModel>,
    pub slurm_stats_all: Vec<SlurmStatsModel>,
    pub slurm_stats_state: TableState,
    pub dag: Option<DagLayout>,
    pub summary: Option<WorkflowSummary>,
    pub detail_view: DetailViewType,
    pub selected_workflow_id: Option<i64>,
    pub focus: Focus,
    pub previous_focus: Focus,
    pub filter: Option<Filter>,
    pub filter_input: String,
    pub filter_column_index: usize,
    pub filter_target: FilterTarget,

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
        let current_user = crate::get_username();

        let mut app = Self {
            client,
            server_url: server_url.clone(),
            server_url_input: String::new(),
            user_filter: Some(current_user.clone()),
            workflows: Vec::new(),
            workflows_all: Vec::new(),
            workflows_state: TableState::default(),
            // Default to newest-first to match the dash. Users can cycle
            // through Asc / unsorted with the ID column shortcut.
            workflows_sort: WorkflowsSort::IdDesc,
            workflows_offset: 0,
            workflows_has_more: false,
            jobs: Vec::new(),
            jobs_all: Vec::new(),
            jobs_workflow_id: None,
            jobs_state: TableState::default(),
            selected_job_ids: std::collections::HashSet::new(),
            jobs_sort: JobsSort::None,
            jobs_offset: 0,
            jobs_has_more: false,
            jobs_fetched_at: None,
            files: Vec::new(),
            files_all: Vec::new(),
            files_state: TableState::default(),
            user_data: Vec::new(),
            user_data_all: Vec::new(),
            user_data_state: TableState::default(),
            user_data_offset: 0,
            user_data_has_more: false,
            events: Vec::new(),
            events_all: Vec::new(),
            events_state: TableState::default(),
            results: Vec::new(),
            results_all: Vec::new(),
            results_state: TableState::default(),
            results_workflow_id: None,
            results_sort: ResultsSort::None,
            results_offset: 0,
            results_has_more: false,
            exec_time_map: std::collections::HashMap::new(),
            compute_nodes: Vec::new(),
            compute_nodes_all: Vec::new(),
            compute_nodes_state: TableState::default(),
            compute_nodes_sort: ComputeNodesSort::None,
            compute_nodes_offset: 0,
            compute_nodes_has_more: false,
            scheduled_nodes: Vec::new(),
            scheduled_nodes_all: Vec::new(),
            scheduled_nodes_state: TableState::default(),
            slurm_stats: Vec::new(),
            slurm_stats_all: Vec::new(),
            slurm_stats_state: TableState::default(),
            dag: None,
            summary: None,
            detail_view: DetailViewType::Summary,
            selected_workflow_id: None,
            focus: Focus::Workflows,
            previous_focus: Focus::Workflows,
            filter: None,
            filter_input: String::new(),
            filter_column_index: 0,
            filter_target: FilterTarget::Details,
            popup: None,
            status_message: None,
            workflow_path_input: String::new(),
            // Off by default to minimize server load: each refresh tick fans
            // out into several list calls per connected TUI against the
            // single-writer SQLite backend. Users opt in with `A`.
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
        // Capture the currently-selected workflow id so we can re-find it
        // after the refresh, instead of snapping back to row 0 when the list
        // shifts.
        let prev_id = self
            .workflows_state
            .selected()
            .and_then(|i| self.workflows.get(i))
            .and_then(|w| w.id);

        let offset = Some(self.workflows_offset);
        let limit = Some(TUI_PAGE_SIZE);
        let has_more;
        (self.workflows_all, has_more) = if let Some(ref user) = self.user_filter {
            self.client.list_workflows_for_user(user, offset, limit)?
        } else {
            self.client.list_workflows(offset, limit)?
        };
        self.workflows_has_more = has_more;

        // Re-apply any active workflow filter against the freshly loaded data.
        if self.filter_target == FilterTarget::Workflows
            && let Some(ref filter) = self.filter.clone()
        {
            self.workflows =
                filter_workflow_list(&self.workflows_all, &filter.column, &filter.value);
        } else {
            self.workflows = self.workflows_all.clone();
        }
        self.apply_workflows_sort();

        if let Some(id) = prev_id
            && let Some(idx) = self.workflows.iter().position(|w| w.id == Some(id))
        {
            self.workflows_state.select(Some(idx));
        } else if self.workflows.is_empty() {
            self.workflows_state.select(None);
        } else {
            // Repair the selection if the previously-selected row vanished
            // and we don't have a stable id to recover. Leaving a stale,
            // out-of-bounds index would let later actions operate on the
            // wrong row.
            match self.workflows_state.selected() {
                Some(idx) if idx < self.workflows.len() => {}
                _ => self.workflows_state.select(Some(0)),
            }
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
            Focus::RecoverPrompt => Focus::RecoverPrompt,
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
                    DetailViewType::ComputeNodes => {
                        (&mut self.compute_nodes_state, self.compute_nodes.len())
                    }
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::UserData => (&mut self.user_data_state, self.user_data.len()),
                    DetailViewType::Summary | DetailViewType::Dag => return, // No table to navigate
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
            | Focus::RecoverPrompt
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
                    DetailViewType::ComputeNodes => {
                        (&mut self.compute_nodes_state, self.compute_nodes.len())
                    }
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::UserData => (&mut self.user_data_state, self.user_data.len()),
                    DetailViewType::Summary | DetailViewType::Dag => return, // No table to navigate
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
            | Focus::RecoverPrompt
            | Focus::Popup => {}
        }
    }

    pub fn page_down_in_active_table(&mut self) {
        match self.focus {
            Focus::Workflows => {
                let len = self.workflows.len();
                if len == 0 {
                    return;
                }
                let next = self
                    .workflows_state
                    .selected()
                    .map(|i| (i + PAGE_STEP).min(len.saturating_sub(1)))
                    .unwrap_or(0);
                self.workflows_state.select(Some(next));
            }
            Focus::Details => {
                let (state, len) = match self.detail_view {
                    DetailViewType::Jobs => (&mut self.jobs_state, self.jobs.len()),
                    DetailViewType::Files => (&mut self.files_state, self.files.len()),
                    DetailViewType::Events => (&mut self.events_state, self.events.len()),
                    DetailViewType::Results => (&mut self.results_state, self.results.len()),
                    DetailViewType::ComputeNodes => {
                        (&mut self.compute_nodes_state, self.compute_nodes.len())
                    }
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::UserData => (&mut self.user_data_state, self.user_data.len()),
                    DetailViewType::Summary | DetailViewType::Dag => return,
                };
                if len > 0 {
                    let next = state
                        .selected()
                        .map(|i| (i + PAGE_STEP).min(len.saturating_sub(1)))
                        .unwrap_or(0);
                    state.select(Some(next));
                }
            }
            Focus::FilterInput
            | Focus::ServerUrlInput
            | Focus::WorkflowPathInput
            | Focus::OutputDirInput
            | Focus::RecoverPrompt
            | Focus::Popup => {}
        }
    }

    pub fn page_up_in_active_table(&mut self) {
        match self.focus {
            Focus::Workflows => {
                if self.workflows.is_empty() {
                    return;
                }
                let next = self
                    .workflows_state
                    .selected()
                    .map(|i| i.saturating_sub(PAGE_STEP))
                    .unwrap_or(0);
                self.workflows_state.select(Some(next));
            }
            Focus::Details => {
                let (state, len) = match self.detail_view {
                    DetailViewType::Jobs => (&mut self.jobs_state, self.jobs.len()),
                    DetailViewType::Files => (&mut self.files_state, self.files.len()),
                    DetailViewType::Events => (&mut self.events_state, self.events.len()),
                    DetailViewType::Results => (&mut self.results_state, self.results.len()),
                    DetailViewType::ComputeNodes => {
                        (&mut self.compute_nodes_state, self.compute_nodes.len())
                    }
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::UserData => (&mut self.user_data_state, self.user_data.len()),
                    DetailViewType::Summary | DetailViewType::Dag => return,
                };
                if len > 0 {
                    let next = state
                        .selected()
                        .map(|i| i.saturating_sub(PAGE_STEP))
                        .unwrap_or(0);
                    state.select(Some(next));
                }
            }
            Focus::FilterInput
            | Focus::ServerUrlInput
            | Focus::WorkflowPathInput
            | Focus::OutputDirInput
            | Focus::RecoverPrompt
            | Focus::Popup => {}
        }
    }

    pub fn jump_to_top_in_active_table(&mut self) {
        match self.focus {
            Focus::Workflows => {
                if !self.workflows.is_empty() {
                    self.workflows_state.select(Some(0));
                }
            }
            Focus::Details => {
                let (state, len) = match self.detail_view {
                    DetailViewType::Jobs => (&mut self.jobs_state, self.jobs.len()),
                    DetailViewType::Files => (&mut self.files_state, self.files.len()),
                    DetailViewType::Events => (&mut self.events_state, self.events.len()),
                    DetailViewType::Results => (&mut self.results_state, self.results.len()),
                    DetailViewType::ComputeNodes => {
                        (&mut self.compute_nodes_state, self.compute_nodes.len())
                    }
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::UserData => (&mut self.user_data_state, self.user_data.len()),
                    DetailViewType::Summary | DetailViewType::Dag => return,
                };
                if len > 0 {
                    state.select(Some(0));
                }
            }
            Focus::FilterInput
            | Focus::ServerUrlInput
            | Focus::WorkflowPathInput
            | Focus::OutputDirInput
            | Focus::RecoverPrompt
            | Focus::Popup => {}
        }
    }

    pub fn jump_to_bottom_in_active_table(&mut self) {
        match self.focus {
            Focus::Workflows => {
                if !self.workflows.is_empty() {
                    self.workflows_state.select(Some(self.workflows.len() - 1));
                }
            }
            Focus::Details => {
                let (state, len) = match self.detail_view {
                    DetailViewType::Jobs => (&mut self.jobs_state, self.jobs.len()),
                    DetailViewType::Files => (&mut self.files_state, self.files.len()),
                    DetailViewType::Events => (&mut self.events_state, self.events.len()),
                    DetailViewType::Results => (&mut self.results_state, self.results.len()),
                    DetailViewType::ComputeNodes => {
                        (&mut self.compute_nodes_state, self.compute_nodes.len())
                    }
                    DetailViewType::ScheduledNodes => {
                        (&mut self.scheduled_nodes_state, self.scheduled_nodes.len())
                    }
                    DetailViewType::SlurmStats => {
                        (&mut self.slurm_stats_state, self.slurm_stats.len())
                    }
                    DetailViewType::UserData => (&mut self.user_data_state, self.user_data.len()),
                    DetailViewType::Summary | DetailViewType::Dag => return,
                };
                if len > 0 {
                    state.select(Some(len - 1));
                }
            }
            Focus::FilterInput
            | Focus::ServerUrlInput
            | Focus::WorkflowPathInput
            | Focus::OutputDirInput
            | Focus::RecoverPrompt
            | Focus::Popup => {}
        }
    }

    pub fn load_detail_data(&mut self) -> Result<()> {
        if let Some(idx) = self.workflows_state.selected()
            && let Some(workflow_id_opt) = self.workflows.get(idx).map(|w| w.id)
        {
            // When the selected workflow changes, restart every detail list at
            // its first page so we don't carry a stale offset into a workflow
            // that may have far fewer records.
            if self.selected_workflow_id != workflow_id_opt {
                self.reset_detail_pagination();
                self.selected_job_ids.clear();
            }
            self.selected_workflow_id = workflow_id_opt;
            if let Some(workflow_id) = workflow_id_opt {
                // Clear any existing filter when loading new data
                self.filter = None;

                match self.detail_view {
                    DetailViewType::Summary => {
                        if self.jobs_workflow_id != Some(workflow_id) {
                            self.jobs_all = self.client.list_jobs(workflow_id, None, None)?;
                            self.jobs_workflow_id = Some(workflow_id);
                            self.jobs_fetched_at = Some(chrono::Utc::now());
                        }
                        self.jobs = self.jobs_all.clone();
                        self.apply_jobs_sort();
                        let workflow = self.client.get_workflow(workflow_id)?;
                        let completion = self.client.is_workflow_complete(workflow_id)?;

                        let mut counts = [0usize; 11];
                        for job in &self.jobs_all {
                            if let Some(s) = &job.status {
                                counts[*s as usize] += 1;
                            }
                        }
                        self.summary = Some(WorkflowSummary {
                            workflow_id,
                            workflow_name: workflow.name,
                            workflow_user: workflow.user,
                            description: workflow.description,
                            is_complete: completion.is_complete,
                            is_canceled: completion.is_canceled,
                            total_jobs: self.jobs_all.len(),
                            counts,
                        });
                    }
                    DetailViewType::Jobs => {
                        // self.filter was just cleared above, so this loads an
                        // unfiltered first page. Server-side filtering is driven
                        // through reload_jobs_page from apply_filter/paging.
                        self.reload_jobs_page()?;
                    }
                    DetailViewType::Files => {
                        self.files_all = self.client.list_files(workflow_id)?;
                        self.files = self.files_all.clone();
                        if !self.files.is_empty() {
                            self.files_state.select(Some(0));
                        }
                    }
                    DetailViewType::UserData => {
                        (self.user_data_all, self.user_data_has_more) =
                            self.client.list_user_data(
                                workflow_id,
                                Some(self.user_data_offset),
                                Some(TUI_PAGE_SIZE),
                            )?;
                        self.user_data = self.user_data_all.clone();
                        if self.user_data.is_empty() {
                            self.user_data_state.select(None);
                        } else {
                            self.user_data_state.select(Some(0));
                        }
                    }
                    DetailViewType::Events => {
                        // Start SSE connection for real-time events
                        self.start_sse_connection(workflow_id);
                    }
                    DetailViewType::Results => {
                        (self.results_all, self.results_has_more) = self.client.list_results(
                            workflow_id,
                            Some(self.results_offset),
                            Some(TUI_PAGE_SIZE),
                        )?;
                        // results_all now holds only one page; invalidate the
                        // full-list cache so the Slurm Stats tab refetches the
                        // complete set for its CPU%/runtime computations.
                        self.results_workflow_id = None;
                        self.results = self.results_all.clone();
                        self.apply_results_sort();
                        if !self.results.is_empty() {
                            self.results_state.select(Some(0));
                        }
                    }
                    DetailViewType::ComputeNodes => {
                        (self.compute_nodes_all, self.compute_nodes_has_more) =
                            self.client.list_compute_nodes(
                                workflow_id,
                                Some(self.compute_nodes_offset),
                                Some(TUI_PAGE_SIZE),
                            )?;
                        self.compute_nodes = self.compute_nodes_all.clone();
                        self.apply_compute_nodes_sort();
                        if !self.compute_nodes.is_empty() {
                            self.compute_nodes_state.select(Some(0));
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
                            && let Ok((r, _)) = self.client.list_results(workflow_id, None, None)
                        {
                            self.results_all = r;
                            self.results = self.results_all.clone();
                            self.apply_results_sort();
                            self.results_workflow_id = Some(workflow_id);
                        }
                        self.rebuild_exec_time_map();
                    }
                    DetailViewType::Dag => {
                        if self.jobs_workflow_id != Some(workflow_id) {
                            self.jobs_all = self.client.list_jobs(workflow_id, None, None)?;
                            self.jobs_workflow_id = Some(workflow_id);
                            self.jobs_fetched_at = Some(chrono::Utc::now());
                            self.jobs = self.jobs_all.clone();
                            self.apply_jobs_sort();
                        }
                        self.build_dag_from_jobs();
                    }
                }
            }
        }
        Ok(())
    }

    /// Force a re-fetch of the active detail view, bypassing the per-workflow
    /// caches that `load_detail_data` uses to skip redundant calls. Without
    /// this, refreshing while the selected workflow is unchanged leaves cached
    /// views (e.g. Summary, Results) showing stale data. Table positions and
    /// the active filter are preserved so a refresh doesn't snap the user back
    /// to the top or silently drop their filter. Callers are expected to run
    /// `refresh_workflows` immediately before this.
    pub fn reload_detail_data(&mut self) -> Result<()> {
        // The Events view streams over SSE and updates itself live; reloading
        // it would tear down the connection and discard accumulated history,
        // so there is nothing to refresh here.
        if self.detail_view == DetailViewType::Events {
            return Ok(());
        }

        // load_detail_data clears self.filter and reloads the full, unfiltered
        // lists, so capture the active filter and table positions to restore
        // afterward.
        let jobs_sel = self.jobs_state.selected();
        let files_sel = self.files_state.selected();
        let user_data_sel = self.user_data_state.selected();
        let results_sel = self.results_state.selected();
        let compute_nodes_sel = self.compute_nodes_state.selected();
        let scheduled_nodes_sel = self.scheduled_nodes_state.selected();
        let slurm_stats_sel = self.slurm_stats_state.selected();
        let prev_filter = self.filter.clone();
        let filter_target = self.filter_target;

        // Invalidate caches keyed by workflow id so load_detail_data refetches.
        self.jobs_workflow_id = None;
        self.results_workflow_id = None;
        self.load_detail_data()?;

        // Re-narrow freshly-loaded detail data and restore the filter flag that
        // load_detail_data cleared. A Workflows-target filter is already
        // re-applied (with its selection preserved) by refresh_workflows, so
        // only its Details counterpart needs re-narrowing here.
        if let Some(filter) = prev_filter {
            if filter_target == FilterTarget::Details {
                if self.detail_view == DetailViewType::Jobs {
                    // Jobs filters server-side; re-fetch the current page with
                    // the filter rather than narrowing the loaded page.
                    self.filter = Some(filter.clone());
                    self.reload_jobs_page()?;
                } else {
                    self.filter_active_view(FilterTarget::Details, &filter.column, &filter.value);
                }
            }
            self.filter = Some(filter);
        }

        restore_selection(&mut self.jobs_state, jobs_sel, self.jobs.len());
        restore_selection(&mut self.files_state, files_sel, self.files.len());
        restore_selection(
            &mut self.user_data_state,
            user_data_sel,
            self.user_data.len(),
        );
        restore_selection(&mut self.results_state, results_sel, self.results.len());
        restore_selection(
            &mut self.compute_nodes_state,
            compute_nodes_sel,
            self.compute_nodes.len(),
        );
        restore_selection(
            &mut self.scheduled_nodes_state,
            scheduled_nodes_sel,
            self.scheduled_nodes.len(),
        );
        restore_selection(
            &mut self.slurm_stats_state,
            slurm_stats_sel,
            self.slurm_stats.len(),
        );
        Ok(())
    }

    /// Sort `self.results` in-place based on `self.results_sort`. Rows with
    /// missing values sort last in both directions so they don't crowd the
    /// top.
    pub fn apply_results_sort(&mut self) {
        // RFC3339 completion_time → epoch seconds for ordering; unparseable
        // timestamps sort last (treated as None below).
        fn completion_secs(r: &ResultModel) -> Option<i64> {
            chrono::DateTime::parse_from_rfc3339(&r.completion_time)
                .ok()
                .map(|dt| dt.timestamp())
        }
        match self.results_sort {
            ResultsSort::None => {}
            ResultsSort::IdDesc => self
                .results
                .sort_by_key(|r| (r.id.is_none(), std::cmp::Reverse(r.id.unwrap_or(i64::MIN)))),
            ResultsSort::IdAsc => self
                .results
                .sort_by_key(|r| (r.id.is_none(), r.id.unwrap_or(i64::MAX))),
            ResultsSort::JobIdDesc => self.results.sort_by_key(|r| std::cmp::Reverse(r.job_id)),
            ResultsSort::JobIdAsc => self.results.sort_by_key(|r| r.job_id),
            ResultsSort::ReturnDesc => self
                .results
                .sort_by_key(|r| std::cmp::Reverse(r.return_code)),
            ResultsSort::ReturnAsc => self.results.sort_by_key(|r| r.return_code),
            // Cache the parsed timestamp per row; unparseable times sort last.
            ResultsSort::CompletionDesc => self.results.sort_by_cached_key(|r| {
                let secs = completion_secs(r);
                (secs.is_none(), std::cmp::Reverse(secs.unwrap_or(0)))
            }),
            ResultsSort::CompletionAsc => self.results.sort_by_cached_key(|r| {
                let secs = completion_secs(r);
                (secs.is_none(), secs.unwrap_or(0))
            }),
            ResultsSort::PeakMemoryDesc => {
                self.results
                    .sort_by(|a, b| match (a.peak_memory_bytes, b.peak_memory_bytes) {
                        (Some(x), Some(y)) => y.cmp(&x),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    });
            }
            ResultsSort::PeakMemoryAsc => {
                self.results
                    .sort_by(|a, b| match (a.peak_memory_bytes, b.peak_memory_bytes) {
                        (Some(x), Some(y)) => x.cmp(&y),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    });
            }
            // f64::total_cmp gives a total order for floats; partial_cmp +
            // unwrap_or(Equal) violates the strict-weak-ordering required by
            // sort_by when NaN is present, which can scramble unrelated
            // rows. NaN sorts to the end via its natural total_cmp position
            // (greater than +inf), which we don't bother special-casing.
            ResultsSort::PeakCpuDesc => {
                self.results
                    .sort_by(|a, b| match (a.peak_cpu_percent, b.peak_cpu_percent) {
                        (Some(x), Some(y)) => y.total_cmp(&x),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    });
            }
            ResultsSort::PeakCpuAsc => {
                self.results
                    .sort_by(|a, b| match (a.peak_cpu_percent, b.peak_cpu_percent) {
                        (Some(x), Some(y)) => x.total_cmp(&y),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    });
            }
            ResultsSort::RuntimeDesc => {
                self.results
                    .sort_by(|a, b| b.exec_time_minutes.total_cmp(&a.exec_time_minutes));
            }
            ResultsSort::RuntimeAsc => {
                self.results
                    .sort_by(|a, b| a.exec_time_minutes.total_cmp(&b.exec_time_minutes));
            }
        }
    }

    pub fn cycle_results_sort_id(&mut self) {
        let prev_id = self.selected_result_id();
        self.results_sort = self.results_sort.cycle_id();
        self.apply_results_sort();
        self.restore_results_selection(prev_id);
    }

    pub fn cycle_results_sort_job_id(&mut self) {
        let prev_id = self.selected_result_id();
        self.results_sort = self.results_sort.cycle_job_id();
        self.apply_results_sort();
        self.restore_results_selection(prev_id);
    }

    pub fn cycle_results_sort_return(&mut self) {
        let prev_id = self.selected_result_id();
        self.results_sort = self.results_sort.cycle_return();
        self.apply_results_sort();
        self.restore_results_selection(prev_id);
    }

    pub fn cycle_results_sort_completion(&mut self) {
        let prev_id = self.selected_result_id();
        self.results_sort = self.results_sort.cycle_completion();
        self.apply_results_sort();
        self.restore_results_selection(prev_id);
    }

    pub fn cycle_results_sort_peak_memory(&mut self) {
        let prev_id = self.selected_result_id();
        self.results_sort = self.results_sort.cycle_peak_memory();
        self.apply_results_sort();
        self.restore_results_selection(prev_id);
    }

    pub fn cycle_results_sort_peak_cpu(&mut self) {
        let prev_id = self.selected_result_id();
        self.results_sort = self.results_sort.cycle_peak_cpu();
        self.apply_results_sort();
        self.restore_results_selection(prev_id);
    }

    pub fn cycle_results_sort_runtime(&mut self) {
        let prev_id = self.selected_result_id();
        self.results_sort = self.results_sort.cycle_runtime();
        self.apply_results_sort();
        self.restore_results_selection(prev_id);
    }

    fn selected_result_id(&self) -> Option<i64> {
        self.results_state
            .selected()
            .and_then(|i| self.results.get(i))
            .and_then(|r| r.id)
    }

    /// Re-select the row whose stable id matches `prev_id`. Falls back to
    /// row 0 if the previously-selected row is no longer present, or clears
    /// selection entirely when the list is empty.
    fn restore_results_selection(&mut self, prev_id: Option<i64>) {
        if let Some(id) = prev_id
            && let Some(idx) = self.results.iter().position(|r| r.id == Some(id))
        {
            self.results_state.select(Some(idx));
            return;
        }
        if self.results.is_empty() {
            self.results_state.select(None);
        } else {
            self.results_state.select(Some(0));
        }
    }

    /// Sort `self.jobs` in-place based on `self.jobs_sort`. Stable for None.
    pub fn apply_jobs_sort(&mut self) {
        match self.jobs_sort {
            JobsSort::None => {}
            // Sort missing IDs last in either direction (server-assigned IDs
            // should always be present, but match the None-last convention
            // used by the Results / Status sorts for consistency).
            JobsSort::IdDesc => self
                .jobs
                .sort_by_key(|j| (j.id.is_none(), std::cmp::Reverse(j.id.unwrap_or(i64::MIN)))),
            JobsSort::IdAsc => self
                .jobs
                .sort_by_key(|j| (j.id.is_none(), j.id.unwrap_or(i64::MAX))),
            // Cache lowercased keys so we don't allocate on every comparison.
            JobsSort::NameDesc => self
                .jobs
                .sort_by_cached_key(|j| std::cmp::Reverse(j.name.to_lowercase())),
            JobsSort::NameAsc => self.jobs.sort_by_cached_key(|j| j.name.to_lowercase()),
            JobsSort::StatusDesc => self.jobs.sort_by(|a, b| {
                let ka = a.status.map(|s| s as u8).unwrap_or(u8::MAX);
                let kb = b.status.map(|s| s as u8).unwrap_or(u8::MAX);
                kb.cmp(&ka)
            }),
            JobsSort::StatusAsc => self.jobs.sort_by(|a, b| {
                let ka = a.status.map(|s| s as u8).unwrap_or(u8::MAX);
                let kb = b.status.map(|s| s as u8).unwrap_or(u8::MAX);
                ka.cmp(&kb)
            }),
        }
    }

    pub fn cycle_jobs_sort_id(&mut self) {
        let prev_id = self.selected_job_id();
        self.jobs_sort = self.jobs_sort.cycle_id();
        self.apply_jobs_sort();
        self.restore_jobs_selection(prev_id);
    }

    pub fn cycle_jobs_sort_name(&mut self) {
        let prev_id = self.selected_job_id();
        self.jobs_sort = self.jobs_sort.cycle_name();
        self.apply_jobs_sort();
        self.restore_jobs_selection(prev_id);
    }

    pub fn cycle_jobs_sort_status(&mut self) {
        let prev_id = self.selected_job_id();
        self.jobs_sort = self.jobs_sort.cycle_status();
        self.apply_jobs_sort();
        self.restore_jobs_selection(prev_id);
    }

    /// Sort `self.workflows` in-place based on `self.workflows_sort`. Rows with
    /// missing IDs sort last in both directions.
    pub fn apply_workflows_sort(&mut self) {
        match self.workflows_sort {
            WorkflowsSort::None => {}
            WorkflowsSort::IdDesc => self
                .workflows
                .sort_by_key(|w| (w.id.is_none(), std::cmp::Reverse(w.id.unwrap_or(i64::MIN)))),
            WorkflowsSort::IdAsc => self
                .workflows
                .sort_by_key(|w| (w.id.is_none(), w.id.unwrap_or(i64::MAX))),
            WorkflowsSort::NameDesc => self
                .workflows
                .sort_by_cached_key(|w| std::cmp::Reverse(w.name.to_lowercase())),
            WorkflowsSort::NameAsc => self.workflows.sort_by_cached_key(|w| w.name.to_lowercase()),
            WorkflowsSort::UserDesc => self
                .workflows
                .sort_by_cached_key(|w| std::cmp::Reverse(w.user.to_lowercase())),
            WorkflowsSort::UserAsc => self.workflows.sort_by_cached_key(|w| w.user.to_lowercase()),
        }
    }

    fn selected_workflow_row_id(&self) -> Option<i64> {
        self.workflows_state
            .selected()
            .and_then(|i| self.workflows.get(i))
            .and_then(|w| w.id)
    }

    fn restore_workflows_selection(&mut self, prev_id: Option<i64>) {
        if self.workflows.is_empty() {
            self.workflows_state.select(None);
            return;
        }
        let idx = prev_id
            .and_then(|id| self.workflows.iter().position(|w| w.id == Some(id)))
            .unwrap_or(0);
        self.workflows_state.select(Some(idx));
    }

    pub fn cycle_workflows_sort_id(&mut self) {
        let prev_id = self.selected_workflow_row_id();
        self.workflows_sort = self.workflows_sort.cycle_id();
        self.apply_workflows_sort();
        self.restore_workflows_selection(prev_id);
    }

    pub fn cycle_workflows_sort_name(&mut self) {
        let prev_id = self.selected_workflow_row_id();
        self.workflows_sort = self.workflows_sort.cycle_name();
        self.apply_workflows_sort();
        self.restore_workflows_selection(prev_id);
    }

    pub fn cycle_workflows_sort_user(&mut self) {
        let prev_id = self.selected_workflow_row_id();
        self.workflows_sort = self.workflows_sort.cycle_user();
        self.apply_workflows_sort();
        self.restore_workflows_selection(prev_id);
    }

    /// Sort `self.compute_nodes` in-place based on `self.compute_nodes_sort`.
    /// Rows with missing values sort last in both directions.
    pub fn apply_compute_nodes_sort(&mut self) {
        match self.compute_nodes_sort {
            ComputeNodesSort::None => {}
            ComputeNodesSort::IdDesc => self
                .compute_nodes
                .sort_by_key(|n| (n.id.is_none(), std::cmp::Reverse(n.id.unwrap_or(i64::MIN)))),
            ComputeNodesSort::IdAsc => self
                .compute_nodes
                .sort_by_key(|n| (n.id.is_none(), n.id.unwrap_or(i64::MAX))),
            ComputeNodesSort::HostnameDesc => self
                .compute_nodes
                .sort_by_cached_key(|n| std::cmp::Reverse(n.hostname.to_lowercase())),
            ComputeNodesSort::HostnameAsc => self
                .compute_nodes
                .sort_by_cached_key(|n| n.hostname.to_lowercase()),
            ComputeNodesSort::PeakCpuDesc => {
                self.compute_nodes
                    .sort_by(|a, b| match (a.peak_cpu_percent, b.peak_cpu_percent) {
                        (Some(x), Some(y)) => y.total_cmp(&x),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    })
            }
            ComputeNodesSort::PeakCpuAsc => {
                self.compute_nodes
                    .sort_by(|a, b| match (a.peak_cpu_percent, b.peak_cpu_percent) {
                        (Some(x), Some(y)) => x.total_cmp(&y),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    })
            }
            ComputeNodesSort::PeakMemoryDesc => self.compute_nodes.sort_by_key(|n| {
                (
                    n.peak_memory_bytes.is_none(),
                    std::cmp::Reverse(n.peak_memory_bytes.unwrap_or(i64::MIN)),
                )
            }),
            ComputeNodesSort::PeakMemoryAsc => self.compute_nodes.sort_by_key(|n| {
                (
                    n.peak_memory_bytes.is_none(),
                    n.peak_memory_bytes.unwrap_or(i64::MAX),
                )
            }),
        }
    }

    fn selected_compute_node_id(&self) -> Option<i64> {
        self.compute_nodes_state
            .selected()
            .and_then(|i| self.compute_nodes.get(i))
            .and_then(|n| n.id)
    }

    /// Reset every detail-list page offset back to the first page. Called when
    /// the selected workflow changes so a stale offset isn't carried over.
    fn reset_detail_pagination(&mut self) {
        self.jobs_offset = 0;
        self.jobs_has_more = false;
        self.results_offset = 0;
        self.results_has_more = false;
        self.compute_nodes_offset = 0;
        self.compute_nodes_has_more = false;
        self.user_data_offset = 0;
        self.user_data_has_more = false;
    }

    /// True when a next page may exist for the active paginated view.
    pub fn active_page_has_more(&self) -> bool {
        match self.focus {
            Focus::Workflows => self.workflows_has_more,
            Focus::Details => match self.detail_view {
                DetailViewType::Jobs => self.jobs_has_more,
                DetailViewType::Results => self.results_has_more,
                DetailViewType::ComputeNodes => self.compute_nodes_has_more,
                DetailViewType::UserData => self.user_data_has_more,
                _ => false,
            },
            _ => false,
        }
    }

    /// Load the next page of the active paginated view, if one may exist.
    pub fn next_page(&mut self) -> Result<()> {
        if !self.active_page_has_more() {
            return Ok(());
        }
        match self.focus {
            Focus::Workflows => {
                self.workflows_offset += TUI_PAGE_SIZE;
                self.refresh_workflows()?;
                self.select_first_workflow_row();
            }
            Focus::Details => match self.detail_view {
                DetailViewType::Jobs => {
                    // reload_jobs_page (not load_detail_data) so the active
                    // server-side filter is preserved across pages.
                    self.jobs_offset += TUI_PAGE_SIZE;
                    self.reload_jobs_page()?;
                }
                DetailViewType::Results => {
                    self.results_offset += TUI_PAGE_SIZE;
                    self.reload_detail_page_preserving_filter()?;
                }
                DetailViewType::ComputeNodes => {
                    self.compute_nodes_offset += TUI_PAGE_SIZE;
                    self.reload_detail_page_preserving_filter()?;
                }
                DetailViewType::UserData => {
                    self.user_data_offset += TUI_PAGE_SIZE;
                    self.reload_detail_page_preserving_filter()?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Load the previous page of the active paginated view, if not already on
    /// the first page.
    pub fn prev_page(&mut self) -> Result<()> {
        match self.focus {
            Focus::Workflows => {
                if self.workflows_offset == 0 {
                    return Ok(());
                }
                self.workflows_offset = (self.workflows_offset - TUI_PAGE_SIZE).max(0);
                self.refresh_workflows()?;
                self.select_first_workflow_row();
            }
            Focus::Details => match self.detail_view {
                DetailViewType::Jobs if self.jobs_offset > 0 => {
                    self.jobs_offset = (self.jobs_offset - TUI_PAGE_SIZE).max(0);
                    self.reload_jobs_page()?;
                }
                DetailViewType::Results if self.results_offset > 0 => {
                    self.results_offset = (self.results_offset - TUI_PAGE_SIZE).max(0);
                    self.reload_detail_page_preserving_filter()?;
                }
                DetailViewType::ComputeNodes if self.compute_nodes_offset > 0 => {
                    self.compute_nodes_offset = (self.compute_nodes_offset - TUI_PAGE_SIZE).max(0);
                    self.reload_detail_page_preserving_filter()?;
                }
                DetailViewType::UserData if self.user_data_offset > 0 => {
                    self.user_data_offset = (self.user_data_offset - TUI_PAGE_SIZE).max(0);
                    self.reload_detail_page_preserving_filter()?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    fn select_first_workflow_row(&mut self) {
        if self.workflows.is_empty() {
            self.workflows_state.select(None);
        } else {
            self.workflows_state.select(Some(0));
        }
    }

    /// Reload the active detail view's current page, preserving any active
    /// client-side Details filter. `load_detail_data` clears `self.filter`, so
    /// we capture it and re-narrow the freshly loaded page afterward. Used by
    /// paging on the client-side-filtered views (Results, Compute Nodes); the
    /// Jobs pane filters server-side via `reload_jobs_page` instead.
    fn reload_detail_page_preserving_filter(&mut self) -> Result<()> {
        let saved = self.filter.clone();
        let saved_target = self.filter_target;
        self.load_detail_data()?;
        if saved_target == FilterTarget::Details
            && let Some(f) = saved
        {
            self.filter = Some(f.clone());
            self.filter_active_view(FilterTarget::Details, &f.column, &f.value);
        }
        Ok(())
    }

    /// Resolve the Jobs-pane filter (`self.filter`) into server-side query
    /// arguments. Returns `(status, name, command, impossible, message)`.
    /// `impossible` is true when a Status filter value cannot be resolved to a
    /// single status (unknown or ambiguous), in which case the caller should
    /// show zero rows without hitting the server; `message`, when present,
    /// explains why so the caller can surface it. Returns all-`None`/empty when
    /// no Jobs filter is active.
    fn jobs_server_filter(
        &self,
    ) -> (
        Option<JobStatus>,
        Option<String>,
        Option<String>,
        bool,
        Option<String>,
    ) {
        if self.filter_target != FilterTarget::Details {
            return (None, None, None, false, None);
        }
        let Some(filter) = self.filter.as_ref() else {
            return (None, None, None, false, None);
        };
        match filter.column.as_str() {
            "Status" => match resolve_job_status_filter(&filter.value) {
                StatusFilterResolution::Matched(s) => (Some(s), None, None, false, None),
                StatusFilterResolution::Unknown => (
                    None,
                    None,
                    None,
                    true,
                    Some(format!("No job status matches \"{}\"", filter.value)),
                ),
                StatusFilterResolution::Ambiguous(names) => (
                    None,
                    None,
                    None,
                    true,
                    Some(format!(
                        "\"{}\" is ambiguous; matches {}. Type a full status name.",
                        filter.value,
                        names.join(", ")
                    )),
                ),
            },
            "Name" => (None, Some(filter.value.clone()), None, false, None),
            "Command" => (None, None, Some(filter.value.clone()), false, None),
            _ => (None, None, None, false, None),
        }
    }

    /// Fetch the current page of the Jobs detail table from the server,
    /// applying the active Jobs filter server-side so it spans the whole
    /// workflow rather than just the loaded page. Does not modify `self.filter`.
    pub fn reload_jobs_page(&mut self) -> Result<()> {
        let Some(workflow_id) = self.selected_workflow_id else {
            return Ok(());
        };
        let (status, name, command, impossible, message) = self.jobs_server_filter();

        // jobs_all now holds at most one (possibly filtered) page; invalidate
        // the full-list cache so Summary/Dag refetch the complete set.
        self.jobs_workflow_id = None;

        if impossible {
            self.jobs_all = Vec::new();
            self.jobs = Vec::new();
            self.jobs_has_more = false;
            self.jobs_state.select(None);
            self.jobs_fetched_at = None;
            if let Some(msg) = message {
                self.set_status(StatusMessage::error(&msg));
            }
            return Ok(());
        }

        (self.jobs_all, self.jobs_has_more) = self.client.list_jobs_filtered(
            workflow_id,
            Some(self.jobs_offset),
            Some(TUI_PAGE_SIZE),
            status,
            name.as_deref(),
            command.as_deref(),
        )?;
        self.jobs_fetched_at = Some(chrono::Utc::now());
        self.jobs = self.jobs_all.clone();
        self.apply_jobs_sort();
        if self.jobs.is_empty() {
            self.jobs_state.select(None);
        } else {
            self.jobs_state.select(Some(0));
        }
        Ok(())
    }

    fn restore_compute_nodes_selection(&mut self, prev_id: Option<i64>) {
        if self.compute_nodes.is_empty() {
            self.compute_nodes_state.select(None);
            return;
        }
        let idx = prev_id
            .and_then(|id| self.compute_nodes.iter().position(|n| n.id == Some(id)))
            .unwrap_or(0);
        self.compute_nodes_state.select(Some(idx));
    }

    pub fn cycle_compute_nodes_sort_id(&mut self) {
        let prev_id = self.selected_compute_node_id();
        self.compute_nodes_sort = self.compute_nodes_sort.cycle_id();
        self.apply_compute_nodes_sort();
        self.restore_compute_nodes_selection(prev_id);
    }

    pub fn cycle_compute_nodes_sort_hostname(&mut self) {
        let prev_id = self.selected_compute_node_id();
        self.compute_nodes_sort = self.compute_nodes_sort.cycle_hostname();
        self.apply_compute_nodes_sort();
        self.restore_compute_nodes_selection(prev_id);
    }

    pub fn cycle_compute_nodes_sort_peak_cpu(&mut self) {
        let prev_id = self.selected_compute_node_id();
        self.compute_nodes_sort = self.compute_nodes_sort.cycle_peak_cpu();
        self.apply_compute_nodes_sort();
        self.restore_compute_nodes_selection(prev_id);
    }

    pub fn cycle_compute_nodes_sort_peak_memory(&mut self) {
        let prev_id = self.selected_compute_node_id();
        self.compute_nodes_sort = self.compute_nodes_sort.cycle_peak_memory();
        self.apply_compute_nodes_sort();
        self.restore_compute_nodes_selection(prev_id);
    }

    fn selected_job_id(&self) -> Option<i64> {
        self.jobs_state
            .selected()
            .and_then(|i| self.jobs.get(i))
            .and_then(|j| j.id)
    }

    fn restore_jobs_selection(&mut self, prev_id: Option<i64>) {
        if let Some(id) = prev_id
            && let Some(idx) = self.jobs.iter().position(|j| j.id == Some(id))
        {
            self.jobs_state.select(Some(idx));
            return;
        }
        if self.jobs.is_empty() {
            self.jobs_state.select(None);
        } else {
            self.jobs_state.select(Some(0));
        }
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

    /// Jump to the Events tab for the currently-highlighted workflow and
    /// open its live SSE stream. Works whether focus is on the Workflows
    /// pane or already on a Details tab — `load_detail_data` reads the
    /// highlighted workflow row from `workflows_state` and `Events` is the
    /// case that starts the SSE connection.
    pub fn jump_to_events(&mut self) -> Result<()> {
        self.detail_view = DetailViewType::Events;
        self.focus = Focus::Details;
        self.load_detail_data()
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
        // Decide which table is being filtered based on which pane has focus.
        let target = match self.focus {
            Focus::Workflows => FilterTarget::Workflows,
            _ => FilterTarget::Details,
        };
        self.filter_target = target;

        if self.get_filter_columns().is_empty() {
            self.set_status(StatusMessage::info(
                "Filtering is not supported in this view",
            ));
            return;
        }
        self.focus = Focus::FilterInput;
        self.filter_input.clear();
        self.filter_column_index = 0;
    }

    pub fn cancel_filter(&mut self) {
        self.focus = match self.filter_target {
            FilterTarget::Workflows => Focus::Workflows,
            FilterTarget::Details => Focus::Details,
        };
        self.filter_input.clear();
    }

    pub fn get_filter_columns(&self) -> Vec<&str> {
        if self.filter_target == FilterTarget::Workflows {
            return vec!["Name", "User", "Description"];
        }
        match self.detail_view {
            DetailViewType::Summary => vec![], // Summary view doesn't support filtering
            DetailViewType::Jobs => vec!["Status", "Name", "Command"],
            DetailViewType::Files => vec!["Name", "Path"],
            DetailViewType::UserData => vec!["Name", "Data"],
            DetailViewType::Events => vec!["Event Type", "Data"],
            DetailViewType::Results => vec!["Status", "Return Code"],
            DetailViewType::ComputeNodes => vec!["Hostname", "Active"],
            DetailViewType::ScheduledNodes => vec!["Status", "Scheduler Type"],
            DetailViewType::SlurmStats => vec!["Job ID", "Slurm Job", "Nodes"],
            DetailViewType::Dag => vec![], // DAG view doesn't support filtering
        }
    }

    /// Apply a filter using the currently-selected row's value for a sensible
    /// "primary" column on each table. No-op when the focused table doesn't
    /// have a useful primary column or no row is selected.
    pub fn filter_by_current_row(&mut self) {
        let (target, column_name, value) = match self.focus {
            Focus::Workflows => {
                let Some(idx) = self.workflows_state.selected() else {
                    return;
                };
                let Some(wf) = self.workflows.get(idx) else {
                    return;
                };
                (FilterTarget::Workflows, "User", wf.user.clone())
            }
            Focus::Details => match self.detail_view {
                DetailViewType::Jobs => {
                    let Some(idx) = self.jobs_state.selected() else {
                        return;
                    };
                    let Some(job) = self.jobs.get(idx) else {
                        return;
                    };
                    let Some(s) = job.status else { return };
                    (FilterTarget::Details, "Status", format!("{:?}", s))
                }
                DetailViewType::Results => {
                    let Some(idx) = self.results_state.selected() else {
                        return;
                    };
                    let Some(r) = self.results.get(idx) else {
                        return;
                    };
                    (FilterTarget::Details, "Status", format!("{:?}", r.status))
                }
                DetailViewType::Events => {
                    let Some(idx) = self.events_state.selected() else {
                        return;
                    };
                    let Some(e) = self.events.get(idx) else {
                        return;
                    };
                    (FilterTarget::Details, "Event Type", e.event_type.clone())
                }
                DetailViewType::ComputeNodes => {
                    let Some(idx) = self.compute_nodes_state.selected() else {
                        return;
                    };
                    let Some(n) = self.compute_nodes.get(idx) else {
                        return;
                    };
                    (FilterTarget::Details, "Hostname", n.hostname.clone())
                }
                DetailViewType::ScheduledNodes => {
                    let Some(idx) = self.scheduled_nodes_state.selected() else {
                        return;
                    };
                    let Some(n) = self.scheduled_nodes.get(idx) else {
                        return;
                    };
                    (FilterTarget::Details, "Status", n.status.clone())
                }
                DetailViewType::Files => {
                    let Some(idx) = self.files_state.selected() else {
                        return;
                    };
                    let Some(file) = self.files.get(idx) else {
                        return;
                    };
                    (FilterTarget::Details, "Name", file.name.clone())
                }
                DetailViewType::UserData => {
                    let Some(idx) = self.user_data_state.selected() else {
                        return;
                    };
                    let Some(ud) = self.user_data.get(idx) else {
                        return;
                    };
                    (FilterTarget::Details, "Name", ud.name.clone())
                }
                _ => return,
            },
            _ => return,
        };

        // Resolve the column index *before* mutating any state so that an
        // invalid mapping doesn't leave us with a stale `filter_target`.
        let saved_target = self.filter_target;
        self.filter_target = target;
        let columns = self.get_filter_columns();
        let Some(col_idx) = columns.iter().position(|c| *c == column_name) else {
            self.filter_target = saved_target;
            return;
        };
        self.filter_column_index = col_idx;
        self.filter_input = value;
        self.apply_filter();
    }

    pub fn next_filter_column(&mut self) {
        let columns = self.get_filter_columns();
        if columns.is_empty() {
            self.filter_column_index = 0;
            return;
        }
        self.filter_column_index = (self.filter_column_index + 1) % columns.len();
    }

    pub fn prev_filter_column(&mut self) {
        let columns = self.get_filter_columns();
        if columns.is_empty() {
            self.filter_column_index = 0;
            return;
        }
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
        let target = self.filter_target;
        let return_focus = match target {
            FilterTarget::Workflows => Focus::Workflows,
            FilterTarget::Details => Focus::Details,
        };

        if self.filter_input.is_empty() {
            self.clear_filter();
            self.focus = return_focus;
            return;
        }

        let columns = self.get_filter_columns();
        if columns.is_empty() {
            self.focus = return_focus;
            return;
        }
        let column = columns[self.filter_column_index].to_string();
        let value = self.filter_input.clone().to_lowercase();

        self.filter = Some(Filter {
            column: column.clone(),
            value: value.clone(),
        });
        // The Jobs pane filters server-side (across the whole workflow); other
        // views filter the loaded page client-side.
        if target == FilterTarget::Details && self.detail_view == DetailViewType::Jobs {
            self.jobs_offset = 0;
            if let Err(err) = self.reload_jobs_page() {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to filter jobs: {}",
                    err
                )));
            }
        } else if target == FilterTarget::Workflows {
            // Workflows filtering narrows the loaded page client-side, so
            // restart from page 1 first; otherwise filtering while on, say,
            // page 3 would search only that page. refresh_workflows re-applies
            // the active filter (now set above) to the freshly loaded page.
            self.workflows_offset = 0;
            if let Err(err) = self.refresh_workflows() {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to filter workflows: {}",
                    err
                )));
            }
        } else {
            self.filter_active_view(target, &column, &value);
        }
        self.focus = return_focus;
    }

    /// Narrow the visible rows of the active table (`target` plus the current
    /// `detail_view`) to those matching `column`/`value`, resetting the
    /// selection to the first match. Shared by interactive filtering
    /// (`apply_filter`) and refresh (`reload_detail_data`), which must
    /// re-narrow freshly-loaded data after the per-workflow caches are
    /// invalidated. Leaves focus unchanged; callers manage focus.
    fn filter_active_view(&mut self, target: FilterTarget, column: &str, value: &str) {
        // Re-bind as owned so the verbatim match arms below can keep using
        // `column.as_str()` / `&value`.
        let column = column.to_string();
        let value = value.to_string();

        if target == FilterTarget::Workflows {
            self.workflows = filter_workflow_list(&self.workflows_all, &column, &value);
            self.apply_workflows_sort();
            if !self.workflows.is_empty() {
                self.workflows_state.select(Some(0));
            } else {
                self.workflows_state.select(None);
            }
            return;
        }

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
                self.apply_jobs_sort();
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
                self.apply_results_sort();
                if !self.results.is_empty() {
                    self.results_state.select(Some(0));
                } else {
                    self.results_state.select(None);
                }
            }
            DetailViewType::ComputeNodes => {
                self.compute_nodes = self
                    .compute_nodes_all
                    .iter()
                    .filter(|node| match column.as_str() {
                        "Hostname" => node.hostname.to_lowercase().contains(&value),
                        "Active" => node
                            .is_active
                            .map(|active| (if active { "yes" } else { "no" }).contains(&value))
                            .unwrap_or(false),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                self.apply_compute_nodes_sort();
                if !self.compute_nodes.is_empty() {
                    self.compute_nodes_state.select(Some(0));
                } else {
                    self.compute_nodes_state.select(None);
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
            DetailViewType::UserData => {
                self.user_data = self
                    .user_data_all
                    .iter()
                    .filter(|ud| match column.as_str() {
                        "Name" => ud.name.to_lowercase().contains(&value),
                        "Data" => ud
                            .data
                            .as_ref()
                            .map(|v| v.to_string().to_lowercase().contains(&value))
                            .unwrap_or(false),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                if !self.user_data.is_empty() {
                    self.user_data_state.select(Some(0));
                } else {
                    self.user_data_state.select(None);
                }
            }
            DetailViewType::Summary | DetailViewType::Dag => {
                // Summary and DAG views don't support filtering
            }
        }
    }

    pub fn clear_filter(&mut self) {
        // Clear the filter for whichever pane currently has focus. This way
        // pressing `c` on the Workflows pane always restores the workflow
        // list, even if the last filter applied was a Details filter.
        let target = match self.focus {
            Focus::Workflows => FilterTarget::Workflows,
            _ => FilterTarget::Details,
        };
        if self.filter_target == target {
            self.filter = None;
        }
        if target == FilterTarget::Workflows {
            self.workflows = self.workflows_all.clone();
            self.apply_workflows_sort();
            if !self.workflows.is_empty() {
                self.workflows_state.select(Some(0));
            } else {
                self.workflows_state.select(None);
            }
            return;
        }
        match self.detail_view {
            DetailViewType::Jobs => {
                // Re-fetch an unfiltered first page from the server (the Jobs
                // filter is server-side, so the loaded page may be a strict
                // subset that can't be widened client-side).
                self.jobs_offset = 0;
                if let Err(err) = self.reload_jobs_page() {
                    self.set_status(StatusMessage::error(&format!(
                        "Failed to reload jobs: {}",
                        err
                    )));
                }
            }
            DetailViewType::Files => {
                self.files = self.files_all.clone();
                if !self.files.is_empty() {
                    self.files_state.select(Some(0));
                }
            }
            DetailViewType::UserData => {
                self.user_data = self.user_data_all.clone();
                if !self.user_data.is_empty() {
                    self.user_data_state.select(Some(0));
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
                self.apply_results_sort();
                if !self.results.is_empty() {
                    self.results_state.select(Some(0));
                }
            }
            DetailViewType::ComputeNodes => {
                self.compute_nodes = self.compute_nodes_all.clone();
                self.apply_compute_nodes_sort();
                if !self.compute_nodes.is_empty() {
                    self.compute_nodes_state.select(Some(0));
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
            DetailViewType::Summary | DetailViewType::Dag => {
                // Summary and DAG views don't support filtering
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
        // The set of workflows changes, so restart at the first page.
        self.workflows_offset = 0;
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
                if let Ok(jobs) = self.client.list_jobs(workflow_id, None, None) {
                    self.jobs_all = jobs.clone();
                    self.jobs_workflow_id = Some(workflow_id);
                    self.jobs_fetched_at = Some(chrono::Utc::now());
                    self.jobs = jobs;
                    self.apply_jobs_sort();
                    if !self.jobs.is_empty() {
                        self.jobs_state.select(Some(0));
                    }
                    // Clear any filter since we've refreshed all data
                    self.filter = None;
                }
                // Also refresh results
                if let Ok((results, _)) = self.client.list_results(workflow_id, None, None) {
                    self.results_all = results.clone();
                    self.results = results;
                    self.apply_results_sort();
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
        if let Err(e) = config.apply_cookie_header_from_env() {
            log::error!("Failed to apply cookie header: {e}");
        }

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

    /// Open a popup with expanded details for the highlighted workflow,
    /// including the submission directory and configuration fields that don't
    /// fit in the Workflows table. Fetches a fresh copy so the details reflect
    /// the current server state.
    pub fn show_workflow_details(&mut self) {
        let Some(workflow_id) = self
            .get_selected_workflow()
            .and_then(|w| w.id)
            .or(self.selected_workflow_id)
        else {
            self.set_status(StatusMessage::warning("No workflow selected"));
            return;
        };

        let workflow = match self.client.get_workflow(workflow_id) {
            Ok(w) => w,
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Could not load workflow details: {}",
                    e
                )));
                return;
            }
        };

        let rows = build_workflow_detail_rows(&workflow);
        let popup = WorkflowDetailsPopup::new(workflow_id, workflow.name.clone(), rows);
        self.previous_focus = self.focus;
        self.focus = Focus::Popup;
        self.popup = Some(PopupType::WorkflowDetails(popup));
    }

    pub fn request_workflow_action(&mut self, action: WorkflowAction) {
        if let Some(workflow) = self.get_selected_workflow() {
            if let Some(workflow_id) = workflow.id {
                let workflow_name = workflow.name.clone();

                // Recover actions skip the standard yes/no confirmation
                // and use a dedicated prompt that collects multipliers.
                // The prompt itself acts as the confirmation gate.
                if matches!(
                    action,
                    WorkflowAction::Recover | WorkflowAction::RecoverDryRun
                ) {
                    let dry_run = action == WorkflowAction::RecoverDryRun;
                    self.open_recover_prompt(workflow_id, &workflow_name, dry_run);
                    return;
                }

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
                PendingAction::JobsResetStatus(job_ids) => {
                    let description = format!("{} job(s)", job_ids.len());
                    if let Err(e) = self.reset_jobs_status_cli(&job_ids, &description) {
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
            WorkflowAction::Recover => unreachable!(),    // Handled above
            WorkflowAction::RecoverDryRun => unreachable!(), // Handled above
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
                    WorkflowAction::Recover => unreachable!(),
                    WorkflowAction::RecoverDryRun => unreachable!(),
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

        let workflow_id_str = workflow_id.to_string();

        // First, do a dry-run check to see if there are existing output files
        let check_output = self
            .torc_cli_command(&[
                "-f",
                "json",
                "workflows",
                "init",
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
        let workflow_id_str = workflow_id.to_string();

        let mut args = vec!["workflows", "init", "--no-prompts", &workflow_id_str];
        if force {
            args.push("--force");
        }

        let output = self.torc_cli_command(&args).output();

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
        let workflow_id_str = workflow_id.to_string();

        let mut args = vec!["workflows", "reinit", &workflow_id_str];
        if force {
            args.push("--force");
        }

        let output = self.torc_cli_command(&args).output();

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
        let workflow_id_str = workflow_id.to_string();

        // Run CLI command: torc workflows reset-status --no-prompts <workflow_id>
        let output = self
            .torc_cli_command(&[
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

    /// Reset one or more jobs to uninitialized using the CLI command
    /// (following the reset_workflow_cli pattern). The CLI enforces the safety
    /// checks: no active workers and no job may be Running or Pending.
    /// `description` is used in the success message (e.g. "Job 'x'" or
    /// "3 job(s)").
    fn reset_jobs_status_cli(&mut self, job_ids: &[i64], description: &str) -> Result<()> {
        let id_strs: Vec<String> = job_ids.iter().map(|id| id.to_string()).collect();
        let mut args: Vec<&str> = vec!["jobs", "reset-status", "--no-prompts"];
        args.extend(id_strs.iter().map(|s| s.as_str()));

        let output = self.torc_cli_command(&args).output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    self.selected_job_ids.clear();
                    self.set_status(StatusMessage::success(&format!(
                        "{} reset to uninitialized — press 'I' to re-initialize, then \
                         run/submit",
                        description
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
                        "Job reset failed: {}",
                        error_msg
                    )));
                }
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to run jobs reset-status command: {}",
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

    /// Build a `torc` CLI invocation that connects to the same server as the
    /// TUI's own API client: forwards `--url` and the TLS flags, and passes
    /// the basic-auth password via the TORC_PASSWORD environment variable so
    /// it stays out of process listings. The username needs no forwarding —
    /// the CLI derives it from the USER environment variable, same as the
    /// TUI. The cookie header (TORC_COOKIE_HEADER) is already inherited
    /// through the environment.
    fn torc_cli_command(&self, args: &[&str]) -> std::process::Command {
        let mut cmd = std::process::Command::new(self.get_torc_exe_path());
        cmd.arg("--url").arg(self.client.get_base_url());
        if let Some(ref ca_cert) = self.tls.ca_cert_path {
            cmd.arg("--tls-ca-cert").arg(ca_cert);
        }
        if self.tls.insecure {
            cmd.arg("--tls-insecure");
        }
        if let Some((_, Some(password))) = &self.basic_auth {
            cmd.env("TORC_PASSWORD", password);
        }
        cmd.args(args);
        cmd
    }

    fn run_workflow_with_viewer(&mut self, workflow_id: i64, workflow_name: &str) -> Result<()> {
        let mut viewer = ProcessViewer::new(format!("Running: {}", workflow_name));

        let workflow_id_str = workflow_id.to_string();
        let cmd = self.torc_cli_command(&["run", &workflow_id_str]);

        match viewer.start(cmd) {
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

        let workflow_id_str = workflow_id.to_string();

        let args: Vec<&str> = if recover {
            vec!["watch", &workflow_id_str, "--recover", "--show-job-counts"]
        } else {
            vec!["watch", &workflow_id_str, "--show-job-counts"]
        };

        match viewer.start(self.torc_cli_command(&args)) {
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

    fn recover_workflow_with_viewer(
        &mut self,
        workflow_id: i64,
        workflow_name: &str,
        dry_run: bool,
        memory_multiplier: f64,
        runtime_multiplier: f64,
    ) -> Result<()> {
        let title = if dry_run {
            format!("Recover (dry run): {}", workflow_name)
        } else {
            format!("Recovering: {}", workflow_name)
        };
        let mut viewer = ProcessViewer::new(title);

        let workflow_id_str = workflow_id.to_string();
        let output_dir = self.output_dir.display().to_string();
        let mem_str = format!("{}", memory_multiplier);
        let rt_str = format!("{}", runtime_multiplier);

        // --no-prompts is required because the interactive wizard would
        // try to read from stdin, which the TUI owns.
        let mut args = vec![
            "recover",
            &workflow_id_str,
            "--output-dir",
            &output_dir,
            "--memory-multiplier",
            &mem_str,
            "--runtime-multiplier",
            &rt_str,
            "--no-prompts",
        ];
        if dry_run {
            args.push("--dry-run");
        }

        match viewer.start(self.torc_cli_command(&args)) {
            Ok(()) => {
                self.previous_focus = self.focus;
                self.focus = Focus::Popup;
                self.popup = Some(PopupType::ProcessViewer(viewer));
                let msg = if dry_run {
                    format!("Previewing recovery for '{}'...", workflow_name)
                } else {
                    format!("Recovering workflow '{}'...", workflow_name)
                };
                self.set_status(StatusMessage::info(&msg));
            }
            Err(e) => {
                self.set_status(StatusMessage::error(&format!(
                    "Failed to start recovery: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    /// Open the multiplier-input modal that gates the actual `torc recover`
    /// subprocess launch.
    pub fn open_recover_prompt(&mut self, workflow_id: i64, workflow_name: &str, dry_run: bool) {
        let title = if dry_run {
            format!(" Recover '{}' (dry run) ", workflow_name)
        } else {
            format!(" Recover '{}' ", workflow_name)
        };
        let message = if dry_run {
            "Preview proposed resource adjustments without applying them.".to_string()
        } else {
            "Bumps memory/runtime for OOM/timeout jobs, resets failed jobs, \
             and resubmits Slurm allocations."
                .to_string()
        };

        let dialog = RecoverPromptDialog::new(&title, &message, !dry_run);
        self.previous_focus = self.focus;
        self.focus = Focus::RecoverPrompt;
        self.popup = Some(PopupType::RecoverPrompt {
            dialog,
            workflow_id,
            workflow_name: workflow_name.to_string(),
            dry_run,
        });
    }

    pub fn recover_prompt_add_char(&mut self, c: char) {
        if let Some(PopupType::RecoverPrompt { dialog, .. }) = self.popup.as_mut() {
            dialog.add_char(c);
        }
    }

    pub fn recover_prompt_backspace(&mut self) {
        if let Some(PopupType::RecoverPrompt { dialog, .. }) = self.popup.as_mut() {
            dialog.backspace();
        }
    }

    pub fn recover_prompt_toggle_field(&mut self) {
        if let Some(PopupType::RecoverPrompt { dialog, .. }) = self.popup.as_mut() {
            dialog.toggle_field();
        }
    }

    pub fn recover_prompt_cancel(&mut self) {
        if matches!(self.popup, Some(PopupType::RecoverPrompt { .. })) {
            self.popup = None;
            self.focus = self.previous_focus;
        }
    }

    pub fn recover_prompt_submit(&mut self) -> Result<()> {
        // Parse first; if invalid, attach the error to the dialog and keep
        // the modal open.
        let parsed = if let Some(PopupType::RecoverPrompt { dialog, .. }) = self.popup.as_mut() {
            match dialog.parse() {
                Ok(values) => Some(values),
                Err(err) => {
                    dialog.set_error(err);
                    None
                }
            }
        } else {
            None
        };

        let Some((mem, rt)) = parsed else {
            return Ok(());
        };

        // Take ownership of the popup so we can drop it before launching
        // the subprocess (which installs its own popup).
        let (workflow_id, workflow_name, dry_run) = if let Some(PopupType::RecoverPrompt {
            workflow_id,
            workflow_name,
            dry_run,
            ..
        }) = self.popup.take()
        {
            (workflow_id, workflow_name, dry_run)
        } else {
            return Ok(());
        };

        self.focus = self.previous_focus;
        self.recover_workflow_with_viewer(workflow_id, &workflow_name, dry_run, mem, rt)
    }

    // === Job Actions ===

    pub fn get_selected_job(&self) -> Option<&JobModel> {
        self.jobs_state
            .selected()
            .and_then(|idx| self.jobs.get(idx))
    }

    /// Toggle multi-reset selection on the job under the cursor, then advance
    /// to the next row so repeated presses sweep down the list.
    pub fn toggle_job_selection(&mut self) {
        if let Some(job_id) = self.get_selected_job().and_then(|j| j.id) {
            if !self.selected_job_ids.insert(job_id) {
                self.selected_job_ids.remove(&job_id);
            }
            self.next_in_active_table();
        } else {
            self.set_status(StatusMessage::warning("No job selected"));
        }
    }

    /// Select every currently-listed job (respecting the active filter), or
    /// clear the selection if all listed jobs are already selected.
    pub fn toggle_select_all_jobs(&mut self) {
        let listed: Vec<i64> = self.jobs.iter().filter_map(|j| j.id).collect();
        if listed.is_empty() {
            self.set_status(StatusMessage::warning("No jobs listed"));
            return;
        }
        if listed.iter().all(|id| self.selected_job_ids.contains(id)) {
            self.selected_job_ids.clear();
            self.set_status(StatusMessage::info("Selection cleared"));
        } else {
            self.selected_job_ids.extend(&listed);
            self.set_status(StatusMessage::info(&format!(
                "Selected {} listed job(s)",
                listed.len()
            )));
        }
    }

    /// Request a reset for every selected job that is currently listed.
    /// Returns false only when the multi-select is empty, so the caller can
    /// fall back to the single-job (cursor row) path. When a selection exists
    /// but none of its jobs are listed (e.g., the filter changed), this warns
    /// and returns true so the caller does NOT silently reset the cursor row.
    fn request_selected_jobs_reset(&mut self) -> bool {
        // An empty selection is the only case where falling back to the
        // cursor-row job is the intended behavior.
        if self.selected_job_ids.is_empty() {
            return false;
        }
        // Intersect with the listed jobs: selections made under an earlier
        // filter may reference rows that are no longer shown, and acting on
        // invisible jobs would be surprising.
        let targets: Vec<&JobModel> = self
            .jobs
            .iter()
            .filter(|j| j.id.is_some_and(|id| self.selected_job_ids.contains(&id)))
            .collect();
        if targets.is_empty() {
            // A selection exists but none of its jobs are in the current view.
            // Don't fall through to resetting the cursor row, which the user
            // didn't pick; tell them their selection is hidden. The selection
            // is left intact so clearing the filter restores it.
            self.set_status(StatusMessage::warning(
                "Selected jobs are not in the current view; clear the filter or press '*' to reselect",
            ));
            return true;
        }

        let completed = targets
            .iter()
            .filter(|j| j.status == Some(JobStatus::Completed))
            .count();
        let mut message = format!(
            "Reset {} selected job(s) to uninitialized for rerun?\n\
             Downstream dependents are reset when the workflow is re-initialized ('I').",
            targets.len()
        );
        if completed > 0 {
            message.push_str(&format!(
                "\nWarning: {} of them completed successfully; resetting discards their \
                 results and reruns them.",
                completed
            ));
        }

        let job_ids: Vec<i64> = targets.iter().filter_map(|j| j.id).collect();
        self.previous_focus = self.focus;
        self.focus = Focus::Popup;
        self.popup = Some(PopupType::Confirmation {
            dialog: ConfirmationDialog::new("Reset Job Statuses", &message),
            action: PendingAction::JobsResetStatus(job_ids),
        });
        true
    }

    pub fn request_job_action(&mut self, action: JobAction) {
        if action == JobAction::ResetStatus && self.request_selected_jobs_reset() {
            return;
        }

        if let Some(job) = self.get_selected_job() {
            if let Some(job_id) = job.id {
                let job_name = job.name.clone();
                let job_status = job.status;
                let mut message = action.confirmation_message(&job_name);
                if action == JobAction::ResetStatus && job_status == Some(JobStatus::Completed) {
                    message.push_str(
                        "\nWarning: this job completed successfully; resetting discards its \
                         results and reruns it.",
                    );
                }
                let dialog = ConfirmationDialog::new(
                    match action {
                        JobAction::Cancel => "Cancel Job",
                        JobAction::Terminate => "Terminate Job",
                        JobAction::Retry => "Retry Job",
                        JobAction::ResetStatus => "Reset Job Status",
                    },
                    &message,
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
        if action == JobAction::ResetStatus {
            return self.reset_jobs_status_cli(&[job_id], &format!("Job '{}'", job_name));
        }

        let result = match action {
            JobAction::Cancel => self.client.cancel_job(job_id),
            JobAction::Terminate => self.client.terminate_job(job_id),
            JobAction::Retry => self.client.retry_job(job_id),
            JobAction::ResetStatus => unreachable!("handled above"),
        };

        match result {
            Ok(_) => {
                let msg = match action {
                    JobAction::Cancel => format!("Job '{}' canceled", job_name),
                    JobAction::Terminate => format!("Job '{}' terminated", job_name),
                    JobAction::Retry => format!("Job '{}' queued for retry", job_name),
                    JobAction::ResetStatus => unreachable!("handled above"),
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
                job.compute_node_id,
                job.start_time.clone(),
            );
            self.previous_focus = self.focus;
            self.focus = Focus::Popup;
            self.popup = Some(PopupType::JobDetails(popup));
        } else {
            self.set_status(StatusMessage::warning("No job selected"));
        }
    }

    // === Log Viewer ===

    /// Find the loaded `WorkflowModel` for the currently-selected workflow.
    /// Used to resolve log paths against the workflow's recorded
    /// `submission_directory` so logs open regardless of the TUI's CWD.
    fn selected_workflow_model(&self) -> Option<&WorkflowModel> {
        let id = self.selected_workflow_id?;
        self.workflows
            .iter()
            .chain(self.workflows_all.iter())
            .find(|w| w.id == Some(id))
    }

    /// Resolve the effective output directory for locating log files.
    ///
    /// Job and Slurm log files are written relative to the directory the
    /// workflow was submitted from. When the configured `output_dir` is
    /// relative and the workflow recorded a `submission_directory`, resolve
    /// against it so logs open from anywhere on the filesystem -- not just
    /// when the TUI is launched from the original submission directory. An
    /// absolute `output_dir`, or a workflow without a recorded submission
    /// directory (older workflows), falls back to the configured path as-is.
    fn resolve_log_output_dir(&self) -> PathBuf {
        if self.output_dir.is_absolute() {
            return self.output_dir.clone();
        }
        if let Some(dir) = self
            .selected_workflow_model()
            .and_then(|w| w.submission_directory.as_deref())
        {
            return std::path::Path::new(dir).join(&self.output_dir);
        }
        self.output_dir.clone()
    }

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
            let (results, _) = self.client.list_results(workflow_id, None, None)?;

            // Find the most recent result for this job
            // Sort by (run_id, attempt_id) to get the latest attempt of the latest run
            if let Some(result) = results
                .iter()
                .filter(|r| r.job_id == viewer.job_id)
                .max_by_key(|r| (r.run_id, r.attempt_id.unwrap_or(1)))
            {
                let attempt_id = result.attempt_id.unwrap_or(1);
                let job_id = viewer.job_id;
                self.populate_log_viewer(viewer, workflow_id, job_id, result.run_id, attempt_id);
            } else {
                viewer.stdout_content =
                    "No results found for this job.\n\nThe job may not have run yet.".to_string();
                viewer.stderr_content = "No results found for this job.".to_string();
            }
        }

        Ok(())
    }

    /// Open the stdout/stderr logs for the result currently selected on the
    /// Results tab. Reuses the Jobs-tab log-loading logic, but targets the
    /// specific run/attempt of the selected result rather than the job's
    /// latest attempt.
    pub fn show_result_logs(&mut self) {
        let Some(result) = self
            .results_state
            .selected()
            .and_then(|idx| self.results.get(idx))
            .cloned()
        else {
            self.set_status(StatusMessage::warning("No result selected"));
            return;
        };

        let job_name = self
            .jobs_all
            .iter()
            .find(|j| j.id == Some(result.job_id))
            .map(|j| j.name.clone())
            .unwrap_or_else(|| format!("Job {}", result.job_id));

        let mut viewer = LogViewer::new(result.job_id, job_name);
        self.populate_log_viewer(
            &mut viewer,
            result.workflow_id,
            result.job_id,
            result.run_id,
            result.attempt_id.unwrap_or(1),
        );

        self.previous_focus = self.focus;
        self.focus = Focus::Popup;
        self.popup = Some(PopupType::LogViewer(viewer));
    }

    /// Fill a `LogViewer`'s stdout/stderr content for a specific job attempt,
    /// trying the separate `.o`/`.e` files first and falling back to the
    /// combined `.log`. Paths resolve against the workflow's submission
    /// directory via `resolve_log_output_dir` so they open regardless of the
    /// TUI's current directory. Shared by the Jobs and Results tabs.
    fn populate_log_viewer(
        &self,
        viewer: &mut LogViewer,
        workflow_id: i64,
        job_id: i64,
        run_id: i64,
        attempt_id: i64,
    ) {
        let output_dir = self.resolve_log_output_dir();
        let output_dir = output_dir.as_path();

        let stdout_path = get_job_stdout_path(output_dir, workflow_id, job_id, run_id, attempt_id);
        let stderr_path = get_job_stderr_path(output_dir, workflow_id, job_id, run_id, attempt_id);
        let combined_path =
            get_job_combined_path(output_dir, workflow_id, job_id, run_id, attempt_id);

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
        let output_dir = self.resolve_log_output_dir();
        let output_dir = output_dir.as_path();

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

    pub fn get_selected_user_data(&self) -> Option<&UserDataModel> {
        self.user_data_state
            .selected()
            .and_then(|idx| self.user_data.get(idx))
    }

    /// Open a popup showing the full, pretty-printed payload of the selected
    /// user_data record. The table truncates the payload to one line; this shows
    /// the whole object formatted across multiple lines.
    pub fn show_user_data_details(&mut self) {
        if let Some(ud) = self.get_selected_user_data() {
            let popup = UserDataDetailsPopup::new(
                ud.id.unwrap_or(0),
                ud.name.clone(),
                ud.is_ephemeral,
                ud.data.as_ref(),
            );
            self.previous_focus = self.focus;
            self.focus = Focus::Popup;
            self.popup = Some(PopupType::UserDataDetails(popup));
        } else {
            self.set_status(StatusMessage::warning("No user data selected"));
        }
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
                let _ = self.reload_detail_data();
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
        let mut cmd = std::process::Command::new(&server_path);
        cmd.args(["run", "--port", &port_str]);

        match viewer.start(cmd) {
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
        let mut cmd = std::process::Command::new(&server_path);
        cmd.args(["run", "--port", &port_str]);
        if let Some(ref db) = self.standalone_database {
            cmd.arg("--database").arg(db);
        }

        match viewer.start(cmd) {
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
            if let Err(e) = config.apply_cookie_header_from_env() {
                log::error!("Failed to apply cookie header: {e}");
            }

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

/// Restore a previously-captured table selection after a reload. A prior
/// selection is clamped to the new row count (so an out-of-bounds index falls
/// back to the last row); a prior `None`, or an empty table, leaves nothing
/// selected.
fn restore_selection(state: &mut TableState, prev: Option<usize>, len: usize) {
    match prev {
        Some(idx) if len > 0 => state.select(Some(idx.min(len - 1))),
        _ => state.select(None),
    }
}

/// Build the `(label, value)` rows shown in the Workflow Details popup. Covers
/// the scalar fields and summarizes the larger configuration blocks so the
/// user can see everything the Workflows table omits -- most importantly the
/// submission directory, which is needed to locate logs and outputs.
fn build_workflow_detail_rows(w: &WorkflowModel) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    let dash = || "—".to_string();

    rows.push(("Name".to_string(), w.name.clone()));
    rows.push(("User".to_string(), w.user.clone()));
    if let Some(project) = &w.project {
        rows.push(("Project".to_string(), project.clone()));
    }
    if let Some(desc) = &w.description {
        rows.push(("Description".to_string(), desc.clone()));
    }
    rows.push((
        "Submission Directory".to_string(),
        w.submission_directory.clone().unwrap_or_else(dash),
    ));
    rows.push((
        "Timestamp".to_string(),
        w.timestamp
            .as_deref()
            .map(crate::client::utils::format_local_timestamp)
            .unwrap_or_else(dash),
    ));
    rows.push((
        "Run ID".to_string(),
        w.run_id.map(|r| r.to_string()).unwrap_or_else(dash),
    ));
    rows.push((
        "Canceled".to_string(),
        w.is_canceled.unwrap_or(false).to_string(),
    ));
    rows.push((
        "Archived".to_string(),
        w.is_archived.unwrap_or(false).to_string(),
    ));
    if let Some(v) = w.use_pending_failed {
        rows.push(("Use Pending-Failed".to_string(), v.to_string()));
    }
    if let Some(v) = w.enable_ro_crate {
        rows.push(("RO-Crate Enabled".to_string(), v.to_string()));
    }
    if let Some(env) = &w.env
        && !env.is_empty()
    {
        rows.push((
            "Environment Variables".to_string(),
            format!("{} set", env.len()),
        ));
    }
    if let Some(metadata) = &w.metadata
        && !metadata.is_empty()
    {
        rows.push((
            "Metadata Keys".to_string(),
            format!("{} set", metadata.len()),
        ));
    }
    if let Some(defaults) = &w.slurm_defaults
        && !defaults.is_empty()
    {
        rows.push((
            "Slurm Defaults".to_string(),
            format!("{} set", defaults.len()),
        ));
    }
    if w.resource_monitor_config.is_some() {
        rows.push(("Resource Monitor".to_string(), "configured".to_string()));
    }
    if w.execution_config.is_some() {
        rows.push(("Execution Config".to_string(), "configured".to_string()));
    }
    if w.dynamic_jobs.is_some() {
        rows.push(("Dynamic Jobs".to_string(), "configured".to_string()));
    }

    rows
}

/// Outcome of resolving a user-typed Jobs status filter against the known
/// statuses.
enum StatusFilterResolution {
    /// The value resolved to exactly one status (exact match or unique
    /// substring).
    Matched(JobStatus),
    /// The value matched no status name.
    Unknown,
    /// The value is a substring of more than one status name; we refuse to
    /// guess. Holds the matching status names for a helpful message.
    Ambiguous(Vec<String>),
}

/// Resolve a user-typed Jobs status filter to a single [`JobStatus`] for
/// server-side filtering. Matches the status name the user sees in the table
/// (the `{:?}` debug form, e.g. "Completed"), case-insensitively: an exact
/// match wins; otherwise a substring is accepted only when it matches exactly
/// one status. An empty value is treated as `Unknown`. Substrings that match
/// several statuses (e.g. "ed" → Completed/Failed/…) return `Ambiguous` so the
/// caller can surface the ambiguity instead of silently picking one.
fn resolve_job_status_filter(value: &str) -> StatusFilterResolution {
    let v = value.trim().to_lowercase();
    if v.is_empty() {
        return StatusFilterResolution::Unknown;
    }
    const ALL: [JobStatus; 11] = [
        JobStatus::Uninitialized,
        JobStatus::Blocked,
        JobStatus::Ready,
        JobStatus::Pending,
        JobStatus::Running,
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Canceled,
        JobStatus::Terminated,
        JobStatus::Disabled,
        JobStatus::PendingFailed,
    ];
    if let Some(s) = ALL.iter().find(|s| format!("{:?}", s).to_lowercase() == v) {
        return StatusFilterResolution::Matched(*s);
    }
    let matches: Vec<JobStatus> = ALL
        .iter()
        .filter(|s| format!("{:?}", s).to_lowercase().contains(&v))
        .copied()
        .collect();
    match matches.as_slice() {
        [] => StatusFilterResolution::Unknown,
        [s] => StatusFilterResolution::Matched(*s),
        many => {
            StatusFilterResolution::Ambiguous(many.iter().map(|s| format!("{:?}", s)).collect())
        }
    }
}

/// Apply a case-insensitive substring filter on the given column to a list of
/// workflows. Returns a new owned vector containing only matching entries.
pub fn filter_workflow_list(
    workflows: &[WorkflowModel],
    column: &str,
    value: &str,
) -> Vec<WorkflowModel> {
    workflows
        .iter()
        .filter(|w| match column {
            "Name" => w.name.to_lowercase().contains(value),
            "User" => w.user.to_lowercase().contains(value),
            "Description" => w
                .description
                .as_deref()
                .map(|d| d.to_lowercase().contains(value))
                .unwrap_or(false),
            _ => false,
        })
        .cloned()
        .collect()
}
