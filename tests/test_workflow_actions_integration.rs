mod common;

use common::{ServerProcess, start_server};
use rstest::rstest;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use torc::client::default_api;
use torc::client::workflow_manager::WorkflowManager;
use torc::client::workflow_spec::WorkflowSpec;
use torc::config::TorcConfig;

/// Helper to create a temporary workflow spec file
fn create_spec_file(temp_dir: &Path, content: &str) -> std::path::PathBuf {
    let spec_path = temp_dir.join("workflow_spec.yaml");
    fs::write(&spec_path, content).expect("Failed to write spec file");
    spec_path
}

/// Helper to wait for a condition with timeout
fn wait_for<F>(mut condition: F, timeout_secs: u64) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < timeout_secs {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

#[rstest]
fn test_on_workflow_start_run_commands_action(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");

    // Create a workflow spec with on_workflow_start run_commands action
    let spec_content = format!(
        r#"
name: "test_on_workflow_start"
user: "test_user"
description: "Test on_workflow_start action"

jobs:
  - name: "test_job"
    command: "echo 'Job running'"

actions:
  - trigger_type: "on_workflow_start"
    action_type: "run_commands"
    commands:
      - "mkdir -p {}"
      - "echo 'Workflow started' > {}/startup.txt"
      - "date > {}/timestamp.txt"
"#,
        output_dir.display(),
        output_dir.display(),
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify action was created
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(&actions[0].trigger_type, "on_workflow_start");

    // Create a job runner to execute the action
    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");

    // Initialize workflow using WorkflowManager
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create compute node
    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_node = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");
    let compute_node_id = created_node.id.unwrap();

    // Create job runner
    let mut job_runner = torc::client::job_runner::JobRunner::new(
        config.clone(),
        workflow,
        1,
        compute_node_id,
        output_dir.clone(),
        0.1,
        None,
        None,
        None,
        torc::models::ComputeNodesResources {
            id: None,
            num_cpus: 4,
            memory_gb: 8.0,
            num_gpus: 0,
            num_nodes: 1,
            time_limit: None,
            scheduler_config_id: None,
        },
        None,
        None,
        None,
        false,
        "test".to_string(),
        None,
    );

    // Run the job runner for a short time to execute on_workflow_start actions
    thread::spawn(move || {
        let _ = job_runner.run_worker();
    });

    // Wait for the action to be claimed and executed
    let action_executed = wait_for(
        || {
            let actions =
                default_api::get_pending_actions(config, workflow_id, None).unwrap_or_default();
            actions.is_empty() // Action should no longer be pending
        },
        10,
    );

    assert!(action_executed, "Action was not executed within timeout");

    // Verify the commands were executed by checking for created files
    thread::sleep(Duration::from_millis(500)); // Give time for file writes
    assert!(output_dir.exists(), "Output directory was not created");
    assert!(
        output_dir.join("startup.txt").exists(),
        "startup.txt was not created"
    );
    assert!(
        output_dir.join("timestamp.txt").exists(),
        "timestamp.txt was not created"
    );

    // Verify file contents
    let startup_content =
        fs::read_to_string(output_dir.join("startup.txt")).expect("Failed to read startup.txt");
    assert!(startup_content.contains("Workflow started"));
}

#[rstest]
fn test_on_jobs_ready_action(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with on_jobs_ready action
    let spec_content = format!(
        r#"
name: "test_on_jobs_ready"
user: "test_user"
description: "Test on_jobs_ready action"

jobs:
  - name: "setup_job"
    command: "echo 'Setup complete'"

  - name: "process_job_1"
    command: "echo 'Processing 1'"
    depends_on: ["setup_job"]

  - name: "process_job_2"
    command: "echo 'Processing 2'"
    depends_on: ["setup_job"]

actions:
  - trigger_type: "on_jobs_ready"
    action_type: "run_commands"
    jobs: ["process_job_1", "process_job_2"]
    commands:
      - "echo 'Processing jobs are ready' > {}/ready.txt"
"#,
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify action was created
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 1);

    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create job runner to check and execute actions
    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_node = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");

    let mut job_runner = torc::client::job_runner::JobRunner::new(
        config.clone(),
        workflow,
        1,
        created_node.id.unwrap(),
        output_dir.clone(),
        0.1,
        None,
        None,
        None,
        torc::models::ComputeNodesResources {
            id: None,
            num_cpus: 4,
            memory_gb: 8.0,
            num_gpus: 0,
            num_nodes: 1,
            time_limit: None,
            scheduler_config_id: None,
        },
        None,
        None,
        None,
        false,
        "test".to_string(),
        None,
    );

    // Run job runner briefly
    thread::spawn(move || {
        let _ = job_runner.run_worker();
    });

    // Wait for action to execute
    let action_executed = wait_for(|| output_dir.join("ready.txt").exists(), 10);

    assert!(action_executed, "on_jobs_ready action was not executed");

    let ready_content =
        fs::read_to_string(output_dir.join("ready.txt")).expect("Failed to read ready.txt");
    assert!(ready_content.contains("Processing jobs are ready"));
}

#[rstest]
fn test_on_jobs_complete_action(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with on_jobs_complete action
    let spec_content = format!(
        r#"
name: "test_on_jobs_complete"
user: "test_user"
description: "Test on_jobs_complete action"

jobs:
  - name: "task_1"
    command: "echo 'Task 1 done'"

  - name: "task_2"
    command: "echo 'Task 2 done'"

actions:
  - trigger_type: "on_jobs_complete"
    action_type: "run_commands"
    jobs: ["task_1", "task_2"]
    commands:
      - "echo 'All tasks completed' > {}/complete.txt"
      - "date >> {}/complete.txt"
"#,
        output_dir.display(),
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Get workflow and initialize using WorkflowManager
    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create and run job runner
    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_node = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");

    let mut job_runner = torc::client::job_runner::JobRunner::new(
        config.clone(),
        workflow,
        1,
        created_node.id.unwrap(),
        output_dir.clone(),
        0.1,
        None,
        None,
        None,
        torc::models::ComputeNodesResources {
            id: None,
            num_cpus: 16,
            memory_gb: 32.0,
            num_gpus: 0,
            num_nodes: 1,
            time_limit: None,
            scheduler_config_id: None,
        },
        None,
        None,
        None,
        false,
        "test".to_string(),
        None,
    );

    thread::spawn(move || {
        let _ = job_runner.run_worker();
    });

    // Wait for action to execute
    let action_executed = wait_for(|| output_dir.join("complete.txt").exists(), 10);

    assert!(action_executed, "on_jobs_complete action was not executed");

    let complete_content =
        fs::read_to_string(output_dir.join("complete.txt")).expect("Failed to read complete.txt");
    assert!(complete_content.contains("All tasks completed"));
}

#[rstest]
fn test_action_with_regex_job_selection(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec using regex for job selection
    let spec_content = format!(
        r#"
name: "test_regex_selection"
user: "test_user"
description: "Test action with regex job selection"

jobs:
  - name: "train_model_001"
    command: "echo 'Training model 1'"

  - name: "train_model_002"
    command: "echo 'Training model 2'"

  - name: "train_model_003"
    command: "echo 'Training model 3'"

  - name: "evaluate_model"
    command: "echo 'Evaluating'"

actions:
  - trigger_type: "on_jobs_ready"
    action_type: "run_commands"
    job_name_regexes: ["train_model_[0-9]+"]
    commands:
      - "echo 'Training jobs ready' > {}/training_ready.txt"
"#,
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify action was created with correct job_ids (should match 3 training jobs)
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 1);

    let job_ids = actions[0].job_ids.as_ref().unwrap();
    assert_eq!(job_ids.len(), 3, "Should match 3 training jobs");

    // Get workflow and initialize using WorkflowManager
    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_node = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");

    let mut job_runner = torc::client::job_runner::JobRunner::new(
        config.clone(),
        workflow,
        1,
        created_node.id.unwrap(),
        output_dir.clone(),
        0.1,
        None,
        None,
        None,
        torc::models::ComputeNodesResources {
            id: None,
            num_cpus: 4,
            memory_gb: 8.0,
            num_gpus: 0,
            num_nodes: 1,
            time_limit: None,
            scheduler_config_id: None,
        },
        None,
        None,
        None,
        false,
        "test".to_string(),
        None,
    );

    thread::spawn(move || {
        let _ = job_runner.run_worker();
    });

    // Wait for action to execute
    let action_executed = wait_for(|| output_dir.join("training_ready.txt").exists(), 10);

    assert!(action_executed, "Regex-based action was not executed");
}

#[rstest]
fn test_multiple_actions_same_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with multiple actions
    let spec_content = format!(
        r#"
name: "test_multiple_actions"
user: "test_user"
description: "Test multiple actions in same workflow"

jobs:
  - name: "job_1"
    command: "echo 'Job 1'"

actions:
  - trigger_type: "on_workflow_start"
    action_type: "run_commands"
    commands:
      - "echo 'Workflow started' > {}/start.txt"

  - trigger_type: "on_jobs_ready"
    action_type: "run_commands"
    jobs: ["job_1"]
    commands:
      - "echo 'Job ready' > {}/ready.txt"

  - trigger_type: "on_jobs_complete"
    action_type: "run_commands"
    jobs: ["job_1"]
    commands:
      - "echo 'Job complete' > {}/complete.txt"
"#,
        output_dir.display(),
        output_dir.display(),
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify all 3 actions were created
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 3);

    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_node = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");

    let mut job_runner = torc::client::job_runner::JobRunner::new(
        config.clone(),
        workflow,
        1,
        created_node.id.unwrap(),
        output_dir.clone(),
        0.1,
        None,
        None,
        None,
        torc::models::ComputeNodesResources {
            id: None,
            num_cpus: 16,
            memory_gb: 32.0,
            num_gpus: 0,
            num_nodes: 1,
            time_limit: None,
            scheduler_config_id: None,
        },
        None,
        None,
        None,
        false,
        "test".to_string(),
        None,
    );

    thread::spawn(move || {
        let _ = job_runner.run_worker();
    });

    // Wait for all actions to execute
    let all_executed = wait_for(
        || {
            output_dir.join("start.txt").exists()
                && output_dir.join("ready.txt").exists()
                && output_dir.join("complete.txt").exists()
        },
        15,
    );

    assert!(all_executed, "Not all actions were executed");

    // Verify contents
    let start_content =
        fs::read_to_string(output_dir.join("start.txt")).expect("Failed to read start.txt");
    assert!(start_content.contains("Workflow started"));

    let ready_content =
        fs::read_to_string(output_dir.join("ready.txt")).expect("Failed to read ready.txt");
    assert!(ready_content.contains("Job ready"));

    let complete_content =
        fs::read_to_string(output_dir.join("complete.txt")).expect("Failed to read complete.txt");
    assert!(complete_content.contains("Job complete"));
}

#[rstest]
fn test_action_idempotency_multiple_runners(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with an action that writes a counter
    let spec_content = format!(
        r#"
name: "test_idempotency"
user: "test_user"
description: "Test action is executed only once"

jobs:
  - name: "test_job1"
    command: "echo 'test'"
  - name: "test_job2"
    command: "echo 'test'"

actions:
  - trigger_type: "on_workflow_start"
    action_type: "run_commands"
    commands:
      - "echo 'executed' >> {}/counter.txt"
"#,
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");

    // Initialize workflow using WorkflowManager
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create two compute nodes and job runners (simulating concurrent execution)
    let mut runners = vec![];
    for i in 0..2 {
        let compute_node = torc::models::ComputeNodeModel::new(
            workflow_id,
            format!("test-host-{}", i),
            (std::process::id() + i as u32) as i64,
            chrono::Utc::now().to_rfc3339(),
            16,
            32.0,
            0,
            1,
            "local".to_string(),
            None,
        );
        let created_node = default_api::create_compute_node(config, compute_node)
            .expect("Failed to create compute node");

        let runner = torc::client::job_runner::JobRunner::new(
            config.clone(),
            workflow.clone(),
            1,
            created_node.id.unwrap(),
            output_dir.clone(),
            0.1,
            Some(1),
            None,
            None,
            torc::models::ComputeNodesResources {
                id: None,
                num_cpus: created_node.num_cpus,
                memory_gb: created_node.memory_gb,
                num_gpus: created_node.num_gpus,
                num_nodes: created_node.num_nodes,
                time_limit: None,
                scheduler_config_id: None,
            },
            None,
            None,
            None,
            false,
            format!("test-{}", i),
            None,
        );
        runners.push(runner);
    }

    // Run both runners concurrently
    let handles: Vec<_> = runners
        .into_iter()
        .map(|mut runner| {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                let _ = runner.run_worker();
            })
        })
        .collect();

    // Wait for execution
    thread::sleep(Duration::from_secs(2));

    // Verify the action was executed only once (counter.txt should have only one line)
    if output_dir.join("counter.txt").exists() {
        let counter_content =
            fs::read_to_string(output_dir.join("counter.txt")).expect("Failed to read counter.txt");
        let line_count = counter_content.lines().count();
        assert_eq!(
            line_count, 1,
            "Action should be executed exactly once, but was executed {} times",
            line_count
        );
    } else {
        panic!("Action was not executed at all");
    }

    // Clean up threads
    for handle in handles {
        let _ = handle.join();
    }
}

#[rstest]
fn test_on_worker_start_persistent_multiple_workers(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with persistent on_worker_start action
    let spec_content = format!(
        r#"
name: "test_on_worker_start_persistent"
user: "test_user"
description: "Test persistent on_worker_start action with multiple workers"

jobs:
  - name: "test_job"
    command: "echo 'Job running'"

actions:
  - trigger_type: "on_worker_start"
    action_type: "run_commands"
    persistent: true
    commands:
      - "echo 'Worker started' >> {}/worker_starts.txt"
"#,
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify action was created with persistent=true
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(&actions[0].trigger_type, "on_worker_start");
    assert!(actions[0].persistent, "Action should be persistent");

    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");

    // Initialize workflow using WorkflowManager
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create three compute nodes and job runners
    let num_workers = 3;
    let mut runners = vec![];
    for i in 0..num_workers {
        let compute_node = torc::models::ComputeNodeModel::new(
            workflow_id,
            format!("test-host-{}", i),
            (std::process::id() + i as u32) as i64,
            chrono::Utc::now().to_rfc3339(),
            4,
            8.0,
            0,
            1,
            "local".to_string(),
            None,
        );
        let created_node = default_api::create_compute_node(config, compute_node)
            .expect("Failed to create compute node");

        let runner = torc::client::job_runner::JobRunner::new(
            config.clone(),
            workflow.clone(),
            1,
            created_node.id.unwrap(),
            output_dir.clone(),
            0.1,
            Some(1),
            None,
            None,
            torc::models::ComputeNodesResources {
                id: None,
                num_cpus: 4,
                memory_gb: 8.0,
                num_gpus: 0,
                num_nodes: 1,
                time_limit: None,
                scheduler_config_id: None,
            },
            None,
            None,
            None,
            false,
            format!("test-worker-{}", i),
            None,
        );
        runners.push(runner);
    }

    // Run all workers concurrently
    let handles: Vec<_> = runners
        .into_iter()
        .map(|mut runner| {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                let _ = runner.run_worker();
            })
        })
        .collect();

    // Wait for all workers to start and execute the action
    thread::sleep(Duration::from_secs(3));

    // Verify the action was executed by ALL workers (should have multiple lines)
    assert!(
        output_dir.join("worker_starts.txt").exists(),
        "worker_starts.txt should exist"
    );

    let worker_starts_content = fs::read_to_string(output_dir.join("worker_starts.txt"))
        .expect("Failed to read worker_starts.txt");
    let line_count = worker_starts_content
        .lines()
        .filter(|l| !l.is_empty())
        .count();

    assert!(
        line_count >= num_workers,
        "Persistent action should be executed by all {} workers, but only executed {} times",
        num_workers,
        line_count
    );

    // Clean up threads
    for handle in handles {
        let _ = handle.join();
    }
}

#[rstest]
fn test_on_worker_complete_persistent_multiple_workers(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with persistent on_worker_complete action
    let spec_content = format!(
        r#"
name: "test_on_worker_complete_persistent"
user: "test_user"
description: "Test persistent on_worker_complete action with multiple workers"

jobs:
  - name: "quick_job"
    command: "echo 'Quick job done'"

actions:
  - trigger_type: "on_worker_complete"
    action_type: "run_commands"
    persistent: true
    commands:
      - "echo 'Worker completed' >> {}/worker_completions.txt"
"#,
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify action was created with persistent=true
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(&actions[0].trigger_type, "on_worker_complete");
    assert!(actions[0].persistent, "Action should be persistent");

    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");

    // Initialize workflow using WorkflowManager
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create three compute nodes and job runners
    let num_workers = 3;
    let mut runners = vec![];
    for i in 0..num_workers {
        let compute_node = torc::models::ComputeNodeModel::new(
            workflow_id,
            format!("test-host-{}", i),
            (std::process::id() + i as u32) as i64,
            chrono::Utc::now().to_rfc3339(),
            4,
            8.0,
            0,
            1,
            "local".to_string(),
            None,
        );
        let created_node = default_api::create_compute_node(config, compute_node)
            .expect("Failed to create compute node");

        let runner = torc::client::job_runner::JobRunner::new(
            config.clone(),
            workflow.clone(),
            1,
            created_node.id.unwrap(),
            output_dir.clone(),
            0.1,
            Some(1),
            None,
            None,
            torc::models::ComputeNodesResources {
                id: None,
                num_cpus: 16,
                memory_gb: 32.0,
                num_gpus: 0,
                num_nodes: 1,
                time_limit: None,
                scheduler_config_id: None,
            },
            None,
            None,
            None,
            false,
            format!("test-worker-{}", i),
            None,
        );
        runners.push(runner);
    }

    // Run all workers concurrently (they should complete quickly and execute the action)
    let handles: Vec<_> = runners
        .into_iter()
        .map(|mut runner| {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                let _ = runner.run_worker();
            })
        })
        .collect();

    // Wait for all workers to complete before checking results
    for handle in handles {
        let _ = handle.join();
    }

    // Small delay to ensure file writes are flushed
    thread::sleep(Duration::from_millis(500));

    // Verify the action was executed by ALL workers
    assert!(
        output_dir.join("worker_completions.txt").exists(),
        "worker_completions.txt should exist"
    );

    let worker_completions_content = fs::read_to_string(output_dir.join("worker_completions.txt"))
        .expect("Failed to read worker_completions.txt");
    let line_count = worker_completions_content
        .lines()
        .filter(|l| !l.is_empty())
        .count();

    assert!(
        line_count >= num_workers,
        "Persistent action should be executed by all {} workers, but only executed {} times",
        num_workers,
        line_count
    );
}

#[rstest]
fn test_on_worker_start_non_persistent_single_execution(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with non-persistent on_worker_start action (default behavior)
    let spec_content = format!(
        r#"
name: "test_on_worker_start_non_persistent"
user: "test_user"
description: "Test non-persistent on_worker_start action with multiple workers"

jobs:
  - name: "test_job"
    command: "echo 'Job running'"

actions:
  - trigger_type: "on_worker_start"
    action_type: "run_commands"
    commands:
      - "echo 'Worker started once' >> {}/worker_start_once.txt"
"#,
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify action was created with persistent=false (default)
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 1);
    assert!(!actions[0].persistent, "Action should not be persistent");

    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");

    // Initialize workflow using WorkflowManager
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create three compute nodes and job runners
    let num_workers = 3;
    let mut runners = vec![];
    for i in 0..num_workers {
        let compute_node = torc::models::ComputeNodeModel::new(
            workflow_id,
            format!("test-host-{}", i),
            (std::process::id() + i as u32) as i64,
            chrono::Utc::now().to_rfc3339(),
            4,
            8.0,
            0,
            1,
            "local".to_string(),
            None,
        );
        let created_node = default_api::create_compute_node(config, compute_node)
            .expect("Failed to create compute node");

        let runner = torc::client::job_runner::JobRunner::new(
            config.clone(),
            workflow.clone(),
            1,
            created_node.id.unwrap(),
            output_dir.clone(),
            0.1,
            Some(1),
            None,
            None,
            torc::models::ComputeNodesResources {
                id: None,
                num_cpus: 4,
                memory_gb: 8.0,
                num_gpus: 0,
                num_nodes: 1,
                time_limit: None,
                scheduler_config_id: None,
            },
            None,
            None,
            None,
            false,
            format!("test-worker-{}", i),
            None,
        );
        runners.push(runner);
    }

    // Run all workers concurrently
    let handles: Vec<_> = runners
        .into_iter()
        .map(|mut runner| {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                let _ = runner.run_worker();
            })
        })
        .collect();

    // Wait for workers to start
    thread::sleep(Duration::from_secs(3));

    // Verify the action was executed only ONCE (non-persistent behavior)
    if output_dir.join("worker_start_once.txt").exists() {
        let worker_start_content = fs::read_to_string(output_dir.join("worker_start_once.txt"))
            .expect("Failed to read worker_start_once.txt");
        let line_count = worker_start_content
            .lines()
            .filter(|l| !l.is_empty())
            .count();

        assert_eq!(
            line_count, 1,
            "Non-persistent action should be executed exactly once, but was executed {} times",
            line_count
        );
    } else {
        panic!("Action was not executed at all");
    }

    // Clean up threads
    for handle in handles {
        let _ = handle.join();
    }
}

#[rstest]
fn test_on_workflow_complete_action(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with on_workflow_complete action
    let spec_content = format!(
        r#"
name: "test_on_workflow_complete"
user: "test_user"
description: "Test on_workflow_complete action"

jobs:
  - name: "task_1"
    command: "echo 'Task 1 done'"

  - name: "task_2"
    command: "echo 'Task 2 done'"

actions:
  - trigger_type: "on_workflow_complete"
    action_type: "run_commands"
    commands:
      - "echo 'Workflow completed successfully' > {}/workflow_complete.txt"
      - "date >> {}/workflow_complete.txt"
"#,
        output_dir.display(),
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    // Verify action was created
    let actions =
        default_api::get_workflow_actions(config, workflow_id).expect("Failed to get actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(&actions[0].trigger_type, "on_workflow_complete");

    // Get workflow and initialize using WorkflowManager
    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create and run job runner
    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_node = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");

    let mut job_runner = torc::client::job_runner::JobRunner::new(
        config.clone(),
        workflow,
        1,
        created_node.id.unwrap(),
        output_dir.clone(),
        0.1,
        None,
        None,
        None,
        torc::models::ComputeNodesResources {
            id: None,
            num_cpus: 16,
            memory_gb: 32.0,
            num_gpus: 0,
            num_nodes: 1,
            time_limit: None,
            scheduler_config_id: None,
        },
        None,
        None,
        None,
        false,
        "test".to_string(),
        None,
    );

    thread::spawn(move || {
        let _ = job_runner.run_worker();
    });

    // Wait for action to execute - the workflow should complete and trigger the action
    let action_executed = wait_for(|| output_dir.join("workflow_complete.txt").exists(), 15);

    assert!(
        action_executed,
        "on_workflow_complete action was not executed"
    );

    let complete_content = fs::read_to_string(output_dir.join("workflow_complete.txt"))
        .expect("Failed to read workflow_complete.txt");
    assert!(complete_content.contains("Workflow completed successfully"));
}

#[rstest]
fn test_on_workflow_complete_idempotency_multiple_workers(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("torc_output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    // Create a workflow spec with on_workflow_complete action that writes a counter
    // This verifies the action is executed only once even with multiple workers
    let spec_content = format!(
        r#"
name: "test_on_workflow_complete_idempotency"
user: "test_user"
description: "Test on_workflow_complete action is executed only once"

jobs:
  - name: "quick_job"
    command: "echo 'Quick job done'"

actions:
  - trigger_type: "on_workflow_complete"
    action_type: "run_commands"
    commands:
      - "echo 'executed' >> {}/workflow_complete_counter.txt"
"#,
        output_dir.display()
    );

    let spec_path = create_spec_file(temp_dir.path(), &spec_content);

    // Create workflow from spec
    let workflow_id =
        WorkflowSpec::create_workflow_from_spec(config, &spec_path, "test_user", false, false)
            .expect("Failed to create workflow from spec");

    let workflow = default_api::get_workflow(config, workflow_id).expect("Failed to get workflow");

    // Initialize workflow using WorkflowManager
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    // Create two compute nodes and job runners (simulating concurrent execution)
    let mut runners = vec![];
    for i in 0..2 {
        let compute_node = torc::models::ComputeNodeModel::new(
            workflow_id,
            format!("test-host-{}", i),
            (std::process::id() + i as u32) as i64,
            chrono::Utc::now().to_rfc3339(),
            16,
            32.0,
            0,
            1,
            "local".to_string(),
            None,
        );
        let created_node = default_api::create_compute_node(config, compute_node)
            .expect("Failed to create compute node");

        let runner = torc::client::job_runner::JobRunner::new(
            config.clone(),
            workflow.clone(),
            1,
            created_node.id.unwrap(),
            output_dir.clone(),
            0.1,
            Some(1),
            None,
            None,
            torc::models::ComputeNodesResources {
                id: None,
                num_cpus: created_node.num_cpus,
                memory_gb: created_node.memory_gb,
                num_gpus: created_node.num_gpus,
                num_nodes: created_node.num_nodes,
                time_limit: None,
                scheduler_config_id: None,
            },
            None,
            None,
            None,
            false,
            format!("test-{}", i),
            None,
        );
        runners.push(runner);
    }

    // Run both runners concurrently
    let handles: Vec<_> = runners
        .into_iter()
        .map(|mut runner| {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                let _ = runner.run_worker();
            })
        })
        .collect();

    // Wait for execution
    thread::sleep(Duration::from_secs(5));

    // Verify the action was executed only once
    if output_dir.join("workflow_complete_counter.txt").exists() {
        let counter_content = fs::read_to_string(output_dir.join("workflow_complete_counter.txt"))
            .expect("Failed to read workflow_complete_counter.txt");
        let line_count = counter_content.lines().count();
        assert_eq!(
            line_count, 1,
            "on_workflow_complete action should be executed exactly once, but was executed {} times",
            line_count
        );
    } else {
        panic!("on_workflow_complete action was not executed at all");
    }

    // Clean up threads
    for handle in handles {
        let _ = handle.join();
    }
}
