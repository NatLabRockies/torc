#![allow(dead_code)]

use rstest::*;
use serde_json::{self, Value};
use std::collections::HashMap;
use std::net::TcpListener;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;
use torc::client::{Configuration, apis};
use torc::models;

const PREPROCESS: &str = "tests/scripts/preprocess.sh";
const WORK: &str = "tests/scripts/work.sh";
const POSTPROCESS: &str = "tests/scripts/postprocess.sh";

/// Global list of server PIDs to clean up at exit
static SERVER_PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static CLEANUP_REGISTERED: std::sync::Once = std::sync::Once::new();

/// Register the atexit cleanup handler (only once)
/// On Windows, we rely on the Drop implementation of ServerProcess instead
#[cfg(unix)]
fn register_cleanup() {
    CLEANUP_REGISTERED.call_once(|| {
        extern "C" fn cleanup_servers() {
            if let Ok(pids) = SERVER_PIDS.lock() {
                for &pid in pids.iter() {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }
        }
        unsafe {
            libc::atexit(cleanup_servers);
        }
    });
}

#[cfg(windows)]
fn register_cleanup() {
    // On Windows, we rely on the Drop implementation of ServerProcess
    // to clean up child processes
}

/// Track a server PID for cleanup at exit
fn track_server_pid(pid: u32) {
    register_cleanup();
    if let Ok(mut pids) = SERVER_PIDS.lock() {
        pids.push(pid);
    }
}

/// Helper function to get the correct executable path for the current platform
/// On Windows, appends .exe; on Unix, returns path as-is
pub fn get_exe_path(base_path: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", base_path)
    } else {
        base_path.to_string()
    }
}

pub struct ServerProcess {
    pub child: Child,
    pub db_file: NamedTempFile, // Keep the temp file alive
    pub port: u16,
    pub config: Configuration,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        // Try to terminate the process gracefully
        if let Err(e) = self.child.kill() {
            eprintln!("Failed to kill process: {e}");
        }
        let _ = self.child.wait(); // Reap zombie

        // Wait a bit for the port to be released
        thread::sleep(Duration::from_millis(100));
    }
}

fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to random port")
        .local_addr()
        .expect("Failed to get local address")
        .port()
}

fn wait_for_server_ready(port: u16, timeout_secs: u64) -> Result<(), String> {
    let url = get_server_url(port);
    let client = reqwest::blocking::Client::new();
    let start = std::time::Instant::now();

    while start.elapsed().as_secs() < timeout_secs {
        if client.get(&url).send().is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(format!(
        "Server on port {} did not become ready within {} seconds",
        port, timeout_secs
    ))
}

fn build_test_binaries() {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--workspace")
        .arg("--bins")
        .arg("--features")
        .arg("server-bin,slurm-runner")
        .status()
        .expect("Failed to execute cargo build");
    if !status.success() {
        panic!("cargo build failed with status: {}", status);
    }
}

fn start_process(db_url: &str, db_file: NamedTempFile) -> ServerProcess {
    let port = find_available_port();
    println!("Setting up database with url: {}", db_url);
    let status = Command::new("sqlx")
        .arg("--no-dotenv")
        .arg("database")
        .arg("setup")
        .arg("--source")
        .arg("torc-server/migrations")
        .env("DATABASE_URL", db_url)
        .status()
        .expect("failed to execute sqlx");
    if !status.success() {
        panic!("sqlx database setup failed with status: {}", status);
    }
    build_test_binaries();

    // Ensure torc-slurm-job-runner binary is in the PATH for tests
    // The binary is built by build_test_binaries but we need to ensure it's accessible.
    let slurm_runner_path = std::env::current_dir()
        .expect("Failed to get current dir")
        .join(get_exe_path("target/debug/torc-slurm-job-runner"));
    if !slurm_runner_path.exists() {
        panic!(
            "torc-slurm-job-runner binary not found at {:?}",
            slurm_runner_path
        );
    }
    eprintln!("Starting server on port {}", port);
    let child = Command::new(get_exe_path("./target/debug/torc-server"))
        .arg("run")
        .arg("--port")
        .arg(port.to_string())
        .arg("--completion-check-interval-secs")
        .arg("0.1")
        .env("DATABASE_URL", db_url)
        .env("RUST_LOG", "info")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("failed to start server process");

    let pid = child.id();

    // Track this PID for cleanup at program exit (handles #[once] fixture limitation)
    track_server_pid(pid);

    if let Err(e) = wait_for_server_ready(port, 10) {
        panic!("Server startup failed: {}", e);
    }

    eprintln!("Server ready on port {} (PID: {})", port, pid);
    let mut config = Configuration::new();
    config.base_path = get_server_url(port);
    ServerProcess {
        child,
        db_file,
        port,
        config,
    }
}

/// Start a test server instance
///
/// This fixture uses `#[once]` to create a single server per test file for performance.
///
/// Server cleanup is handled via `libc::atexit` - all server PIDs are tracked and
/// terminated when the test process exits, even though `Drop` is not called for
/// `#[once]` fixtures stored in static variables.
#[fixture]
#[once]
pub fn start_server() -> ServerProcess {
    // Initialize logger for client-side code running in tests
    let _ = env_logger::try_init();

    let db_file = NamedTempFile::new().expect("Failed to create temporary file");
    let url = format!("sqlite:{}", db_file.path().display());
    let process = start_process(&url, db_file);
    eprint!(
        "Started server process with database file {:?} on port {}",
        process.db_file, process.port
    );
    process
}

pub fn create_test_workflow(config: &Configuration, workflow_name: &str) -> models::WorkflowModel {
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(workflow_name.to_string(), user);
    let created_workflow = apis::workflows_api::create_workflow(config, workflow)
        .expect("Failed to create test workflow");

    // Create a default compute node for this workflow so tests can use compute_node_id=1
    let workflow_id = created_workflow.id.unwrap();
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,                   // num_cpus
        16.0,                // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,
    );
    let _ = apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create default compute node for test workflow");

    created_workflow
}

pub fn create_test_job(
    config: &Configuration,
    workflow_id: i64,
    job_name: &str,
) -> models::JobModel {
    let job = models::JobModel::new(
        workflow_id,
        job_name.to_string(),
        format!("echo 'Running {}'", job_name),
    );
    apis::jobs_api::create_job(config, job).expect("Failed to create test job")
}

pub fn create_diamond_workflow(
    config: &Configuration,
    init_jobs: bool,
    work_dir: &std::path::Path,
) -> HashMap<String, models::JobModel> {
    let name = "test_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create a default compute node for this workflow so tests can use compute_node_id=1
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,                   // num_cpus
        16.0,                // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,
    );
    let _ = apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create default compute node for test workflow");

    // Create local variables for file paths
    let f1_path = work_dir.join("f1.json").to_string_lossy().to_string();
    let f2_path = work_dir.join("f2.json").to_string_lossy().to_string();
    let f3_path = work_dir.join("f3.json").to_string_lossy().to_string();
    let f4_path = work_dir.join("f4.json").to_string_lossy().to_string();
    let f5_path = work_dir.join("f5.json").to_string_lossy().to_string();
    let f6_path = work_dir.join("f6.json").to_string_lossy().to_string();

    let f1 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id as i64, "f1".to_string(), f1_path.clone()),
    )
    .expect("Failed to add file");
    let f2 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id as i64, "f2".to_string(), f2_path.clone()),
    )
    .expect("Failed to add file");
    let f3 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id as i64, "f3".to_string(), f3_path.clone()),
    )
    .expect("Failed to add file");
    let f4 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id as i64, "f4".to_string(), f4_path.clone()),
    )
    .expect("Failed to add file");
    let f5 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id as i64, "f5".to_string(), f5_path.clone()),
    )
    .expect("Failed to add file");
    let f6 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id as i64, "f6".to_string(), f6_path.clone()),
    )
    .expect("Failed to add file");
    let mut preprocess_pre = models::JobModel::new(
        workflow_id as i64,
        "preprocess".to_string(),
        format!(
            "bash {} -i {} -o {} -o {}",
            PREPROCESS, f1_path, f2_path, f3_path
        ),
    );
    let mut work1_pre = models::JobModel::new(
        workflow_id as i64,
        "work1".to_string(),
        format!("bash {} -i {} -o {}", WORK, f2_path, f4_path),
    );
    let mut work2_pre = models::JobModel::new(
        workflow_id as i64,
        "work2".to_string(),
        format!("bash {} -i {} -o {}", WORK, f3_path, f5_path),
    );
    let mut postprocess_pre = models::JobModel::new(
        workflow_id as i64,
        "postprocess".to_string(),
        format!(
            "bash {} -i {} -i {} -o {}",
            POSTPROCESS, f4_path, f5_path, f6_path
        ),
    );
    preprocess_pre.input_file_ids = Some(vec![f1.id.unwrap()]);
    preprocess_pre.output_file_ids = Some(vec![f2.id.unwrap(), f3.id.unwrap()]);
    work1_pre.input_file_ids = Some(vec![f2.id.unwrap()]);
    work1_pre.output_file_ids = Some(vec![f4.id.unwrap()]);
    work2_pre.input_file_ids = Some(vec![f3.id.unwrap()]);
    work2_pre.output_file_ids = Some(vec![f5.id.unwrap()]);
    postprocess_pre.input_file_ids = Some(vec![f4.id.unwrap(), f5.id.unwrap()]);
    postprocess_pre.output_file_ids = Some(vec![f6.id.unwrap()]);
    let preprocess =
        apis::jobs_api::create_job(config, preprocess_pre).expect("Failed to add preprocess");
    let work1 = apis::jobs_api::create_job(config, work1_pre).expect("Failed to add work1");
    let work2 = apis::jobs_api::create_job(config, work2_pre).expect("Failed to add work2");
    let postprocess =
        apis::jobs_api::create_job(config, postprocess_pre).expect("Failed to add postprocess");

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id as i64, None, None)
            .expect("Failed to initialize jobs");
    }

    let mut jobs = HashMap::new();
    jobs.insert("preprocess".to_string(), preprocess);
    jobs.insert("work1".to_string(), work1);
    jobs.insert("work2".to_string(), work2);
    jobs.insert("postprocess".to_string(), postprocess);

    for job in jobs.values() {
        assert!(job.resource_requirements_id.is_some());
        let rr_id = job.resource_requirements_id.unwrap();
        let rr = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
            .expect("Failed to get resource requirements");
        assert_eq!(rr.name, "default".to_string());
    }
    jobs
}

pub fn get_server_url(port: u16) -> String {
    format!("http://localhost:{}/torc-service/v1", port)
}

/// Create a test compute node directly via API
pub fn create_test_compute_node(
    config: &Configuration,
    workflow_id: i64,
) -> models::ComputeNodeModel {
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,                   // num_cpus
        16.0,                // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,
    );

    apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create test compute node")
}

/// Create a test result directly via API for testing other commands
pub fn create_test_result(
    config: &Configuration,
    workflow_id: i64,
    job_id: i64,
) -> models::ResultModel {
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        1,                                      // run_id
        1,                                      // attempt_id
        1,   // compute_node_id (default created by create_test_workflow)
        0,   // return_code (success)
        5.5, // exec_time_minutes
        "2024-01-01T12:00:00.000Z".to_string(), // completion_time
        models::JobStatus::Completed,
    );

    apis::results_api::create_result(config, result).expect("Failed to create test result")
}

/// Helper function to create test user data directly via API
pub fn create_test_user_data(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
    data: serde_json::Value,
    ephemeral: bool,
) -> models::UserDataModel {
    let mut user_data = models::UserDataModel::new(workflow_id, name.to_string());
    user_data.data = Some(data);
    user_data.is_ephemeral = Some(ephemeral);

    apis::user_data_api::create_user_data(config, user_data, None, None)
        .expect("Failed to create test user data")
}

/// Helper function to create test events directly via API
pub fn create_test_event(
    config: &Configuration,
    workflow_id: i64,
    data: serde_json::Value,
) -> models::EventModel {
    let event = models::EventModel::new(workflow_id, data);
    apis::events_api::create_event(config, event).expect("Failed to create test event")
}

/// Helper function to create test files directly via API
pub fn create_test_file(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
    path: &str,
) -> models::FileModel {
    let file = models::FileModel::new(workflow_id, name.to_string(), path.to_string());
    apis::files_api::create_file(config, file).expect("Failed to create test file")
}

/// Helper function to create test workflows with additional options directly via API
pub fn create_test_workflow_with_description(
    config: &Configuration,
    name: &str,
    user: &str,
    description: Option<String>,
) -> models::WorkflowModel {
    let mut workflow = models::WorkflowModel::new(name.to_string(), user.to_string());
    workflow.description = description;
    let created_workflow = apis::workflows_api::create_workflow(config, workflow)
        .expect("Failed to create test workflow");

    // Create a default compute node for this workflow so tests can use compute_node_id=1
    let workflow_id = created_workflow.id.unwrap();
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,                   // num_cpus
        16.0,                // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,
    );
    let _ = apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create default compute node for test workflow");

    created_workflow
}

/// Helper function to create test workflows with advanced configuration
pub fn create_test_workflow_advanced(
    config: &Configuration,
    name: &str,
    user: &str,
    description: Option<String>,
) -> models::WorkflowModel {
    let mut workflow = models::WorkflowModel::new(name.to_string(), user.to_string());
    workflow.description = description;
    let created_workflow = apis::workflows_api::create_workflow(config, workflow)
        .expect("Failed to create test workflow");

    // Create a default compute node for this workflow so tests can use compute_node_id=1
    let workflow_id = created_workflow.id.unwrap();
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,                   // num_cpus
        16.0,                // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,
    );
    let _ = apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create default compute node for test workflow");

    created_workflow
}

/// Helper function to create test resource requirements directly via API
#[allow(clippy::too_many_arguments)]
pub fn create_test_resource_requirements(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
    num_cpus: i64,
    num_gpus: i64,
    num_nodes: i64,
    memory: &str,
    runtime: &str,
) -> models::ResourceRequirementsModel {
    let mut req = models::ResourceRequirementsModel::new(workflow_id, name.to_string());
    req.num_cpus = num_cpus;
    req.num_gpus = num_gpus;
    req.num_nodes = num_nodes;
    req.memory = memory.to_string();
    req.runtime = runtime.to_string();

    apis::resource_requirements_api::create_resource_requirements(config, req)
        .expect("Failed to create test resource requirements")
}

/// Create a workflow with 4 independent jobs that have minimal resource requirements
/// Each job will require: 1 CPU, 1.0 GB memory, 0 GPUs, 1 node
/// This allows testing resource allocation limits (e.g., resources for 2 jobs with 4 jobs available)
pub fn create_minimal_resources_workflow(
    config: &Configuration,
    init_jobs: bool,
) -> HashMap<String, models::JobModel> {
    let name = "minimal_resources_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements that match the test scenario
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "minimal",
        1,        // num_cpus
        0,        // num_gpus
        1,        // num_nodes
        "1g",     // memory
        "P0DT1H", // runtime
    );

    // Create 4 independent jobs to test resource allocation
    let job_names = vec![
        "minimal_job_1",
        "minimal_job_2",
        "minimal_job_3",
        "minimal_job_4",
    ];
    let mut jobs = HashMap::new();

    for job_name in job_names {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo 'minimal resources job: {}'", job_name),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name.to_string(), created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with 3 independent jobs that have high CPU requirements
/// Each job will require: 64 CPUs, 128.0 GB memory, 0 GPUs, 1 node
/// This allows testing resource allocation with high-CPU jobs
pub fn create_high_cpu_workflow(
    config: &Configuration,
    init_jobs: bool,
) -> HashMap<String, models::JobModel> {
    let name = "high_cpu_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements that match the test scenario
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "high_cpu",
        64,        // num_cpus
        0,         // num_gpus
        1,         // num_nodes
        "128g",    // memory
        "P0DT12H", // runtime
    );

    // Create 3 independent high-CPU jobs
    let job_names = vec!["high_cpu_job_1", "high_cpu_job_2", "high_cpu_job_3"];
    let mut jobs = HashMap::new();

    for job_name in job_names {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo 'high CPU job: {}'", job_name),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name.to_string(), created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with 2 independent jobs that have high memory requirements
/// Each job will require: 4 CPUs, 512.0 GB memory, 0 GPUs, 1 node
/// This allows testing resource allocation with memory-intensive jobs
pub fn create_high_memory_workflow(
    config: &Configuration,
    init_jobs: bool,
) -> HashMap<String, models::JobModel> {
    let name = "high_memory_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements that match the test scenario
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "high_memory",
        4,        // num_cpus
        0,        // num_gpus
        1,        // num_nodes
        "512g",   // memory
        "P0DT8H", // runtime
    );

    // Create 2 independent high-memory jobs
    let job_names = vec!["high_memory_job_1", "high_memory_job_2"];
    let mut jobs = HashMap::new();

    for job_name in job_names {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo 'high memory job: {}'", job_name),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name.to_string(), created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with 3 independent jobs that have GPU requirements
/// Each job will require: 8 CPUs, 32.0 GB memory, 4 GPUs, 1 node
/// This allows testing resource allocation with GPU jobs
pub fn create_gpu_workflow(
    config: &Configuration,
    init_jobs: bool,
) -> HashMap<String, models::JobModel> {
    let name = "gpu_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements that match the test scenario
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "gpu",
        8,        // num_cpus
        4,        // num_gpus
        1,        // num_nodes
        "32g",    // memory
        "P0DT6H", // runtime
    );

    // Create 3 independent GPU jobs
    let job_names = vec!["gpu_job_1", "gpu_job_2", "gpu_job_3"];
    let mut jobs = HashMap::new();

    for job_name in job_names {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo 'GPU job: {}'", job_name),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name.to_string(), created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with 2 independent jobs that have multi-node requirements
/// Each job will require: 16 CPUs, 64.0 GB memory, 0 GPUs, 4 nodes
/// This allows testing resource allocation with multi-node jobs
pub fn create_multi_node_workflow(
    config: &Configuration,
    init_jobs: bool,
) -> HashMap<String, models::JobModel> {
    let name = "multi_node_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements that match the test scenario
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "multi_node",
        16,        // num_cpus
        0,         // num_gpus
        4,         // num_nodes
        "64g",     // memory
        "P0DT10H", // runtime
    );

    // Create 2 independent multi-node jobs
    let job_names = vec!["multi_node_job_1", "multi_node_job_2"];
    let mut jobs = HashMap::new();

    for job_name in job_names {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo 'multi-node job: {}'", job_name),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name.to_string(), created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with 2 independent jobs that have maximum resource requirements
/// Each job will require: 128 CPUs, 1024.0 GB memory, 8 GPUs, 8 nodes
/// This allows testing resource allocation with high-end jobs
pub fn create_maximum_resources_workflow(
    config: &Configuration,
    init_jobs: bool,
) -> HashMap<String, models::JobModel> {
    let name = "maximum_resources_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements that match the test scenario
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "maximum",
        128,       // num_cpus
        8,         // num_gpus
        8,         // num_nodes
        "1024g",   // memory
        "P0DT24H", // runtime
    );

    // Create 2 independent maximum-resource jobs
    let job_names = vec!["maximum_job_1", "maximum_job_2"];
    let mut jobs = HashMap::new();

    for job_name in job_names {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo 'maximum resources job: {}'", job_name),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name.to_string(), created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with a dependency chain to test job blocking
/// Pattern: job1 → job2 → job3 → job4 (only job1 should be ready initially)
/// All jobs have the same resource requirements for consistent testing
pub fn create_dependency_chain_workflow(
    config: &Configuration,
    init_jobs: bool,
    num_cpus: i64,
    memory_gb: f64,
    num_gpus: i64,
    num_nodes: i64,
) -> HashMap<String, models::JobModel> {
    let name = format!(
        "dependency_chain_{}cpu_{}gb_{}gpu_{}node_workflow",
        num_cpus, memory_gb, num_gpus, num_nodes
    );
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create files for job dependencies
    let f1 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id, "input".to_string(), "input.txt".to_string()),
    )
    .expect("Failed to add input file");

    let f2 = apis::files_api::create_file(
        config,
        models::FileModel::new(
            workflow_id,
            "intermediate1".to_string(),
            "temp1.txt".to_string(),
        ),
    )
    .expect("Failed to add intermediate file 1");

    let f3 = apis::files_api::create_file(
        config,
        models::FileModel::new(
            workflow_id,
            "intermediate2".to_string(),
            "temp2.txt".to_string(),
        ),
    )
    .expect("Failed to add intermediate file 2");

    let f4 = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id, "output".to_string(), "output.txt".to_string()),
    )
    .expect("Failed to add output file");

    // Create resource requirements
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "dependency_chain",
        num_cpus,
        num_gpus,
        num_nodes,
        &format!("{}g", memory_gb as i64),
        "P0DT2H",
    );

    // Create job1 (ready immediately - takes input, produces intermediate1)
    let mut job1 = models::JobModel::new(
        workflow_id,
        "chain_job_1".to_string(),
        "echo 'dependency chain job 1'".to_string(),
    );
    job1.resource_requirements_id = Some(resource_req.id.unwrap());
    job1.input_file_ids = Some(vec![f1.id.unwrap()]);
    job1.output_file_ids = Some(vec![f2.id.unwrap()]);

    // Create job2 (blocked by job1 - takes intermediate1, produces intermediate2)
    let mut job2 = models::JobModel::new(
        workflow_id,
        "chain_job_2".to_string(),
        "echo 'dependency chain job 2'".to_string(),
    );
    job2.resource_requirements_id = Some(resource_req.id.unwrap());
    job2.input_file_ids = Some(vec![f2.id.unwrap()]);
    job2.output_file_ids = Some(vec![f3.id.unwrap()]);

    // Create job3 (blocked by job2 - takes intermediate2, produces output)
    let mut job3 = models::JobModel::new(
        workflow_id,
        "chain_job_3".to_string(),
        "echo 'dependency chain job 3'".to_string(),
    );
    job3.resource_requirements_id = Some(resource_req.id.unwrap());
    job3.input_file_ids = Some(vec![f3.id.unwrap()]);
    job3.output_file_ids = Some(vec![f4.id.unwrap()]);

    // Create the jobs
    let created_job1 = apis::jobs_api::create_job(config, job1).expect("Failed to create job1");
    let created_job2 = apis::jobs_api::create_job(config, job2).expect("Failed to create job2");
    let created_job3 = apis::jobs_api::create_job(config, job3).expect("Failed to create job3");

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    let mut jobs = HashMap::new();
    jobs.insert("chain_job_1".to_string(), created_job1);
    jobs.insert("chain_job_2".to_string(), created_job2);
    jobs.insert("chain_job_3".to_string(), created_job3);
    jobs
}

/// Create a workflow with multiple jobs having custom resource requirements
/// Allows specifying exact resource requirements for flexible testing
/// Creates between 2-6 independent jobs depending on the resource requirements
pub fn create_custom_resources_workflow(
    config: &Configuration,
    init_jobs: bool,
    num_cpus: i64,
    memory_gb: f64,
    num_gpus: i64,
    num_nodes: i64,
) -> HashMap<String, models::JobModel> {
    let name = format!(
        "custom_{}cpu_{}gb_{}gpu_{}node_workflow",
        num_cpus, memory_gb, num_gpus, num_nodes
    );
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements that match the test scenario
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "custom",
        num_cpus,
        num_gpus,
        num_nodes,
        &format!("{}g", memory_gb as i64), // Convert to string format like "32g"
        "P0DT4H",                          // default runtime
    );

    // Create multiple jobs based on resource intensity
    // More resource-intensive jobs get fewer instances to test resource limits
    let num_jobs = if num_cpus >= 64 || memory_gb >= 256.0 || num_gpus >= 4 || num_nodes >= 4 {
        2 // High-resource jobs: create 2
    } else if num_cpus >= 16 || memory_gb >= 32.0 || num_gpus >= 2 || num_nodes >= 2 {
        4 // Medium-resource jobs: create 4
    } else {
        6 // Low-resource jobs: create 6
    };

    let mut jobs = HashMap::new();
    for i in 1..=num_jobs {
        let job_name = format!("custom_job_{}", i);
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.clone(),
            format!(
                "echo 'custom job {}: {}CPU {}GB {}GPU {}nodes'",
                i, num_cpus, memory_gb, num_gpus, num_nodes
            ),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name, created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with many independent jobs for testing limit parameter behavior
/// Creates jobs_count jobs, each needing 1 CPU, 1GB memory, 0 GPUs, 1 node
/// This allows testing scenarios where limit parameter is more restrictive than resources
pub fn create_many_jobs_workflow(
    config: &Configuration,
    init_jobs: bool,
    jobs_count: usize,
) -> HashMap<String, models::JobModel> {
    let name = format!("many_jobs_workflow_{}_jobs", jobs_count);
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements for minimal jobs
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "many_jobs",
        1,        // num_cpus
        0,        // num_gpus
        1,        // num_nodes
        "1g",     // memory
        "P0DT1H", // runtime
    );

    // Create many independent jobs
    let mut jobs = HashMap::new();
    for i in 1..=jobs_count {
        let job_name = format!("job_{}", i);
        let mut job =
            models::JobModel::new(workflow_id, job_name.clone(), format!("echo 'job {}'", i));
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name, created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Create a workflow with jobs having diverse resource requirements for testing sort methods
/// Creates jobs with different GPU, CPU, memory, and runtime requirements to validate sorting
pub fn create_diverse_jobs_workflow(
    config: &Configuration,
    init_jobs: bool,
) -> HashMap<String, models::JobModel> {
    let name = "diverse_jobs_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create different resource requirement profiles for sorting tests
    let resource_profiles = [
        // (name, cpus, gpus, memory_gb, runtime_hours)
        ("low_gpu_short_small", 4, 1, 8, 1.0), // 1 GPU, 1 hour, 8GB
        ("no_gpu_long_large", 16, 0, 64, 24.0), // 0 GPU, 24 hours, 64GB
        ("high_gpu_medium_medium", 8, 4, 32, 12.0), // 4 GPU, 12 hours, 32GB
        ("medium_gpu_short_large", 12, 2, 128, 2.0), // 2 GPU, 2 hours, 128GB
        ("no_gpu_short_small", 2, 0, 4, 0.5),  // 0 GPU, 30 min, 4GB
        ("high_gpu_long_small", 6, 8, 16, 48.0), // 8 GPU, 48 hours, 16GB
    ];

    let mut jobs = HashMap::new();

    for (i, (name_suffix, cpus, gpus, memory_gb, runtime_hours)) in
        resource_profiles.iter().enumerate()
    {
        // Create resource requirements for this job
        let runtime_str = if *runtime_hours < 1.0 {
            format!("P0DT{}M", (*runtime_hours * 60.0) as i32) // Minutes for < 1 hour
        } else {
            format!("P0DT{}H", *runtime_hours as i32) // Hours for >= 1 hour
        };

        let resource_req = create_test_resource_requirements(
            config,
            workflow_id,
            &format!("diverse_{}", name_suffix),
            *cpus,
            *gpus,
            1, // num_nodes
            &format!("{}g", memory_gb),
            &runtime_str,
        );

        // Create job with these resource requirements
        let job_name = format!("diverse_job_{}", i + 1);
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.clone(),
            format!(
                "echo 'diverse job {}: {}CPU {}GPU {}GB {}h'",
                i + 1,
                cpus,
                gpus,
                memory_gb,
                runtime_hours
            ),
        );
        job.resource_requirements_id = Some(resource_req.id.unwrap());

        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.insert(job_name, created_job);
    }

    if init_jobs {
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
            .expect("Failed to initialize jobs");
    }

    jobs
}

/// Helper function to delete all workflows for all users
/// This is useful for test cleanup to ensure test isolation
/// Returns an error if any workflow deletion fails. Success criteria: ALL workflows must be deleted.
pub fn delete_all_workflows(config: &Configuration) -> Result<(), Box<dyn std::error::Error>> {
    // List all workflows (no user filter to get all workflows)
    let response = apis::workflows_api::list_workflows(
        config, None, // offset
        None, // sort_by
        None, // reverse_sort
        None, // limit (use default to get all)
        None, // name filter
        None, // user filter (no filter = all users)
        None, // description filter
        None, // is_archive filter
    )?;

    let workflows = response.items;
    let mut failed_deletions = Vec::new();

    // Delete each workflow, collecting any failures
    for workflow in workflows {
        if let Some(workflow_id) = workflow.id
            && let Err(e) = apis::workflows_api::delete_workflow(config, workflow_id)
        {
            failed_deletions.push((workflow_id, e.to_string()));
        }
    }

    // Return error if any deletions failed
    if !failed_deletions.is_empty() {
        let error_messages: Vec<String> = failed_deletions
            .iter()
            .map(|(id, err)| format!("Workflow {}: {}", id, err))
            .collect();
        return Err(format!(
            "Failed to delete {} workflow(s): {}",
            failed_deletions.len(),
            error_messages.join("; ")
        )
        .into());
    }

    Ok(())
}

/// Helper function to run CLI commands and capture JSON output
/// If `user` is provided, sets the USER environment variable for the command
pub fn run_cli_with_json(
    args: &[&str],
    server: &ServerProcess,
    user: Option<&str>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut cmd = Command::new(get_exe_path("./target/debug/torc"));
    cmd.arg("--format");
    cmd.arg("json");
    cmd.args(args);
    cmd.env("TORC_API_URL", &server.config.base_path);
    if let Some(u) = user {
        cmd.env("USER", u);
    }

    let output = cmd.output()?;

    if output.status.success() {
        let json_str = String::from_utf8(output.stdout)?;
        let json_value: Value = serde_json::from_str(&json_str)?;
        Ok(json_value)
    } else {
        let error_str = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_str).into())
    }
}

/// Helper function to run CLI commands without JSON output
/// If `user` is provided, sets the USER environment variable for the command
pub fn run_cli_command(
    args: &[&str],
    server: &ServerProcess,
    user: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut cmd = Command::new(get_exe_path("./target/debug/torc"));
    cmd.args(["--url", &server.config.base_path]);
    cmd.args(args);
    cmd.env("TORC_API_URL", &server.config.base_path);
    if let Some(u) = user {
        cmd.env("USER", u);
    }

    // Add target/debug to PATH so spawned binaries like torc-slurm-job-runner can be found
    let current_dir = std::env::current_dir()?;
    let target_debug = current_dir.join("target/debug");
    let path_var = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", target_debug.display(), path_var);
    cmd.env("PATH", new_path);

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout)
    } else {
        let error_str = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_str).into())
    }
}

/// Helper function to run the torc job runner without JSON output
pub fn run_jobs_cli_command(
    args: &[&str],
    server: &ServerProcess,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut cmd = Command::new(get_exe_path("./target/debug/torc"));
    cmd.args(["--url", &server.config.base_path, "run"]);
    cmd.args(args);

    // Add target/debug to PATH so spawned binaries like torc-slurm-job-runner can be found
    let current_dir = std::env::current_dir()?;
    let target_debug = current_dir.join("target/debug");
    let path_var = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", target_debug.display(), path_var);
    cmd.env("PATH", new_path);

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout)
    } else {
        let error_str = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_str).into())
    }
}

/// Helper function to run CLI commands with basic authentication
/// Uses USER env var as username and TORC_PASSWORD env var for password
pub fn run_cli_command_with_auth(
    args: &[&str],
    server: &AccessControlServerProcess,
    username: &str,
    password: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut cmd = Command::new(get_exe_path("./target/debug/torc"));
    cmd.args(["--url", &server.config.base_path]);
    cmd.args(args);
    cmd.env("TORC_API_URL", &server.config.base_path);
    cmd.env("USER", username);
    cmd.env("TORC_PASSWORD", password);

    // Add target/debug to PATH so spawned binaries like torc-slurm-job-runner can be found
    let current_dir = std::env::current_dir()?;
    let target_debug = current_dir.join("target/debug");
    let path_var = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", target_debug.display(), path_var);
    cmd.env("PATH", new_path);

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout)
    } else {
        let error_str = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_str).into())
    }
}

/// Helper function to run CLI commands with basic authentication, returning full output
/// even on failure. Useful for testing error cases.
pub fn run_cli_command_with_auth_full(
    args: &[&str],
    server: &AccessControlServerProcess,
    username: &str,
    password: &str,
) -> std::process::Output {
    let mut cmd = Command::new(get_exe_path("./target/debug/torc"));
    cmd.args(["--url", &server.config.base_path]);
    cmd.args(args);
    cmd.env("TORC_API_URL", &server.config.base_path);
    cmd.env("USER", username);
    cmd.env("TORC_PASSWORD", password);

    // Add target/debug to PATH so spawned binaries like torc-slurm-job-runner can be found
    let current_dir = std::env::current_dir().expect("Failed to get current dir");
    let target_debug = current_dir.join("target/debug");
    let path_var = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", target_debug.display(), path_var);
    cmd.env("PATH", new_path);

    cmd.output().expect("Failed to execute command")
}

/// Helper function to run the torc job runner with authentication
pub fn run_jobs_cli_command_with_auth(
    args: &[&str],
    server: &AccessControlServerProcess,
    username: &str,
    password: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut cmd = Command::new(get_exe_path("./target/debug/torc"));
    cmd.args(["--url", &server.config.base_path, "run"]);
    cmd.args(args);
    cmd.env("TORC_API_URL", &server.config.base_path);
    cmd.env("USER", username);
    cmd.env("TORC_PASSWORD", password);

    // Add target/debug to PATH so spawned binaries like torc-slurm-job-runner can be found
    let current_dir = std::env::current_dir()?;
    let target_debug = current_dir.join("target/debug");
    let path_var = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", target_debug.display(), path_var);
    cmd.env("PATH", new_path);

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout)
    } else {
        let error_str = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_str).into())
    }
}

/// Helper function to run the torc job runner with authentication, returning full output
/// even on failure. Useful for testing error cases.
pub fn run_jobs_cli_command_with_auth_full(
    args: &[&str],
    server: &AccessControlServerProcess,
    username: &str,
    password: &str,
) -> std::process::Output {
    let mut cmd = Command::new(get_exe_path("./target/debug/torc"));
    cmd.args(["--url", &server.config.base_path, "run"]);
    cmd.args(args);
    cmd.env("TORC_API_URL", &server.config.base_path);
    cmd.env("USER", username);
    cmd.env("TORC_PASSWORD", password);

    // Add target/debug to PATH so spawned binaries like torc-slurm-job-runner can be found
    let current_dir = std::env::current_dir().expect("Failed to get current dir");
    let target_debug = current_dir.join("target/debug");
    let path_var = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", target_debug.display(), path_var);
    cmd.env("PATH", new_path);

    cmd.output().expect("Failed to execute command")
}

// ============================================================================
// Access Control Server Fixture
// ============================================================================

/// Create an htpasswd file with test users for access control tests.
/// Uses torc-htpasswd to add users with bcrypt-hashed passwords.
fn create_htpasswd_file(users: &[&str]) -> NamedTempFile {
    let htpasswd_file = NamedTempFile::new().expect("Failed to create htpasswd temp file");
    let htpasswd_path = htpasswd_file.path().to_string_lossy().to_string();

    // Create each user with a strong password using torc-htpasswd add
    // Use cost 4 (minimum) for fast test execution - cost 12 default takes ~250ms per hash
    for user in users.iter() {
        let status = Command::new(get_exe_path("./target/debug/torc-htpasswd"))
            .arg("add")
            .arg("--file")
            .arg(&htpasswd_path)
            .arg("--password")
            .arg("correct horse battery staple")
            .arg("--cost")
            .arg("4")
            .arg(user)
            .status()
            .expect("Failed to run torc-htpasswd");

        if !status.success() {
            panic!(
                "torc-htpasswd failed for user {} with status: {}",
                user, status
            );
        }
    }

    htpasswd_file
}

/// Struct to hold both the server process and the htpasswd file
pub struct AccessControlServerProcess {
    pub server: ServerProcess,
    pub htpasswd_file: NamedTempFile, // Keep the htpasswd file alive
}

impl AccessControlServerProcess {
    /// Get the path to the htpasswd file for direct manipulation in tests.
    pub fn htpasswd_path(&self) -> String {
        self.htpasswd_file.path().to_string_lossy().to_string()
    }

    /// Create a Configuration authenticated as a specific user.
    pub fn config_for_user(&self, username: &str) -> Configuration {
        let mut config = Configuration::new();
        config.base_path = get_server_url(self.server.port);
        config.basic_auth = Some((
            username.to_string(),
            Some("correct horse battery staple".to_string()),
        ));
        config
    }
}

impl std::ops::Deref for AccessControlServerProcess {
    type Target = ServerProcess;

    fn deref(&self) -> &Self::Target {
        &self.server
    }
}

/// Start a server with access control enforcement enabled
fn start_process_with_access_control(
    db_url: &str,
    db_file: NamedTempFile,
    htpasswd_file: NamedTempFile,
) -> AccessControlServerProcess {
    start_process_with_access_control_impl(db_url, db_file, htpasswd_file, false)
}

fn start_process_with_access_control_impl(
    db_url: &str,
    db_file: NamedTempFile,
    htpasswd_file: NamedTempFile,
    require_auth: bool,
) -> AccessControlServerProcess {
    let port = find_available_port();
    println!("Setting up database with url: {}", db_url);
    let status = Command::new("sqlx")
        .arg("--no-dotenv")
        .arg("database")
        .arg("setup")
        .arg("--source")
        .arg("torc-server/migrations")
        .env("DATABASE_URL", db_url)
        .status()
        .expect("failed to execute sqlx");
    if !status.success() {
        panic!("sqlx database setup failed with status: {}", status);
    }
    build_test_binaries();

    eprintln!("Starting server with access control on port {}", port);
    let htpasswd_path = htpasswd_file.path().to_string_lossy().to_string();
    let mut cmd = Command::new(get_exe_path("./target/debug/torc-server"));
    cmd.arg("run")
        .arg("--port")
        .arg(port.to_string())
        .arg("--completion-check-interval-secs")
        .arg("0.1")
        .arg("--enforce-access-control") // Enable access control enforcement
        .arg("--auth-file")
        .arg(&htpasswd_path)
        // Add admin users - these can see all workflows
        // Note: alice and bob are NOT admins because they're used as regular team members
        // in group-based access control tests
        .arg("--admin-user")
        .arg("owner")
        .arg("--admin-user")
        .arg("api_owner")
        .arg("--admin-user")
        .arg("ml_owner")
        .arg("--admin-user")
        .arg("data_owner")
        .arg("--admin-user")
        .arg("ml_api_owner")
        .arg("--admin-user")
        .arg("data_api_owner");
    if require_auth {
        cmd.arg("--require-auth");
    }
    let child = cmd
        .env("DATABASE_URL", db_url)
        .env("RUST_LOG", "info")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("failed to start server process");

    let pid = child.id();

    // Track this PID for cleanup at program exit
    track_server_pid(pid);

    if let Err(e) = wait_for_server_ready(port, 10) {
        panic!("Server startup failed: {}", e);
    }

    eprintln!(
        "Server with access control ready on port {} (PID: {})",
        port, pid
    );
    let mut config = Configuration::new();
    config.base_path = get_server_url(port);
    // Set up basic auth as "owner" (one of the admin users)
    // Note: alice and bob are NOT admins - they're used as regular team members in tests
    config.basic_auth = Some((
        "owner".to_string(),
        Some("correct horse battery staple".to_string()),
    ));
    AccessControlServerProcess {
        server: ServerProcess {
            child,
            db_file,
            port,
            config,
        },
        htpasswd_file,
    }
}

/// Start a test server instance with access control + require-auth enabled.
///
/// This creates a server that **rejects** requests with invalid or missing
/// credentials (returns 401). Use this when testing authentication behavior
/// like reload-auth, where you need to verify that invalid credentials are
/// truly rejected rather than treated as anonymous.
///
/// Test users (all with password "correct horse battery staple"):
/// - alice, bob (regular team members, NOT admins)
/// - carol, dave (regular team members)
/// - owner, api_owner, etc. (admin users)
#[fixture]
#[once]
pub fn start_server_with_required_auth() -> AccessControlServerProcess {
    let _ = env_logger::try_init();

    // Include "owner" who is an admin user for tests that need admin access
    let test_users = ["alice", "bob", "carol", "dave", "owner"];
    let htpasswd_file = create_htpasswd_file(&test_users);
    eprintln!(
        "Created htpasswd file with {} users (require-auth mode)",
        test_users.len()
    );

    let db_file = NamedTempFile::new().expect("Failed to create temporary file");
    let url = format!("sqlite:{}", db_file.path().display());
    let process = start_process_with_access_control_impl(&url, db_file, htpasswd_file, true);
    eprint!(
        "Started server with required auth, database file {:?} on port {}",
        process.server.db_file, process.server.port
    );
    process
}

/// Start a test server instance with access control enforcement enabled
///
/// This fixture creates a server where users can only access workflows they own
/// or have group access to. It creates an htpasswd file with test users that
/// can be authenticated.
///
/// Test users (all with password "correct horse battery staple"):
/// - alice, bob (ML team members)
/// - carol, dave (Data team members)
/// - shared_user (member of both teams)
/// - owner, api_owner, ml_owner, data_owner, job_owner, etc. (workflow owners)
#[fixture]
#[once]
pub fn start_server_with_access_control() -> AccessControlServerProcess {
    // Initialize logger for client-side code running in tests
    let _ = env_logger::try_init();

    // Create htpasswd file with all test users
    // These are the users used in access control tests
    let test_users = [
        // Team members
        "alice",
        "bob",
        "carol",
        "dave",
        "shared_user",
        // Workflow owners
        "owner",
        "owner_user",
        "api_owner",
        "ml_owner",
        "data_owner",
        "ml_api_owner",
        "data_api_owner",
        "job_owner",
        "workflow_creator",
        "some_owner",
        "wf_owner",
        "creator",
        "private-owner",
        "wf-user",
        "wf-user-2",
        "wf-user-3",
        // Test-specific users
        "unauthorized_user",
        "unauthorized_job_user",
        "removable_user",
        "outsider",
        // Resource access test users
        "res_owner",
        "resource_intruder",
        "nf_user",
        "file_owner",
        "grp_res_owner",
        // Admin list test users (non-admin users)
        "admin-test-user-x",
        "admin-test-user-y",
        "admin-test-user-z",
    ];

    let htpasswd_file = create_htpasswd_file(&test_users);
    eprintln!("Created htpasswd file with {} users", test_users.len());

    let db_file = NamedTempFile::new().expect("Failed to create temporary file");
    let url = format!("sqlite:{}", db_file.path().display());
    let process = start_process_with_access_control(&url, db_file, htpasswd_file);
    eprint!(
        "Started server with access control, database file {:?} on port {}",
        process.server.db_file, process.server.port
    );
    process
}
