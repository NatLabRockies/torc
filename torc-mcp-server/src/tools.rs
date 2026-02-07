//! Tool implementations for the Torc MCP server.

use rmcp::{Error as McpError, model::CallToolResult};
use std::fs;
use std::path::Path;
use std::process::Command;

use torc::client::apis::configuration::Configuration;
use torc::client::apis::default_api;
use torc::client::commands::pagination::jobs::{JobListParams, paginate_jobs};
use torc::client::commands::pagination::results::{ResultListParams, paginate_results};
use torc::client::log_paths;
use torc::models::{JobStatus, ResourceRequirementsModel};

/// Helper to create an internal error
fn internal_error(msg: String) -> McpError {
    McpError::internal_error(msg, None)
}

/// Helper to create an invalid params error
fn invalid_params(msg: &str) -> McpError {
    McpError::invalid_request(msg.to_string(), None)
}

/// Get workflow status with job counts.
pub fn get_workflow_status(
    config: &Configuration,
    workflow_id: i64,
) -> Result<CallToolResult, McpError> {
    // Get workflow info
    let workflow = default_api::get_workflow(config, workflow_id)
        .map_err(|e| internal_error(format!("Failed to get workflow: {}", e)))?;

    // Get all jobs
    let jobs = paginate_jobs(config, workflow_id, JobListParams::new())
        .map_err(|e| internal_error(format!("Failed to list jobs: {}", e)))?;

    // Count jobs by status
    let mut status_counts = std::collections::HashMap::new();
    for job in &jobs {
        if let Some(status) = &job.status {
            let status_str = format!("{:?}", status);
            *status_counts.entry(status_str).or_insert(0) += 1;
        }
    }

    let result = serde_json::json!({
        "workflow_id": workflow.id,
        "name": workflow.name,
        "user": workflow.user,
        "description": workflow.description,
        "total_jobs": jobs.len(),
        "job_counts_by_status": status_counts,
    });

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

/// Get detailed job information.
pub fn get_job_details(config: &Configuration, job_id: i64) -> Result<CallToolResult, McpError> {
    let job = default_api::get_job(config, job_id)
        .map_err(|e| internal_error(format!("Failed to get job: {}", e)))?;

    // Get resource requirements if available
    let resource_reqs = if let Some(req_id) = job.resource_requirements_id {
        default_api::get_resource_requirements(config, req_id).ok()
    } else {
        None
    };

    // Get latest result if job has run
    let result = paginate_results(
        config,
        job.workflow_id,
        ResultListParams::new().with_job_id(job_id).with_limit(1),
    )
    .ok()
    .and_then(|items| items.into_iter().next());

    let response = serde_json::json!({
        "job_id": job.id,
        "workflow_id": job.workflow_id,
        "name": job.name,
        "command": job.command,
        "status": format!("{:?}", job.status),
        "invocation_script": job.invocation_script,
        "supports_termination": job.supports_termination,
        "cancel_on_blocking_job_failure": job.cancel_on_blocking_job_failure,
        "depends_on_job_ids": job.depends_on_job_ids,
        "resource_requirements": resource_reqs.map(|r| serde_json::json!({
            "id": r.id,
            "num_cpus": r.num_cpus,
            "num_gpus": r.num_gpus,
            "memory": r.memory,
            "runtime": r.runtime,
        })),
        "latest_result": result.map(|r| serde_json::json!({
            "run_id": r.run_id,
            "return_code": r.return_code,
            "exec_time_minutes": r.exec_time_minutes,
            "completion_time": r.completion_time,
            "peak_memory_bytes": r.peak_memory_bytes,
            "avg_cpu_percent": r.avg_cpu_percent,
        })),
    });

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}

/// Read job logs.
pub fn get_job_logs(
    output_dir: &Path,
    workflow_id: i64,
    job_id: i64,
    run_id: i64,
    attempt_id: i64,
    log_type: &str,
    tail_lines: Option<usize>,
) -> Result<CallToolResult, McpError> {
    let log_path = match log_type.to_lowercase().as_str() {
        "stdout" => {
            log_paths::get_job_stdout_path(output_dir, workflow_id, job_id, run_id, attempt_id)
        }
        "stderr" => {
            log_paths::get_job_stderr_path(output_dir, workflow_id, job_id, run_id, attempt_id)
        }
        _ => return Err(invalid_params("log_type must be 'stdout' or 'stderr'")),
    };

    let content = fs::read_to_string(&log_path)
        .map_err(|e| internal_error(format!("Failed to read log file {}: {}", log_path, e)))?;

    let output = if let Some(n) = tail_lines {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(n);
        lines[start..].join("\n")
    } else {
        content
    };

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        output,
    )]))
}

/// List failed jobs in a workflow.
pub fn list_failed_jobs(
    config: &Configuration,
    workflow_id: i64,
) -> Result<CallToolResult, McpError> {
    let jobs = paginate_jobs(
        config,
        workflow_id,
        JobListParams::new().with_status(JobStatus::Failed),
    )
    .map_err(|e| internal_error(format!("Failed to list jobs: {}", e)))?;

    let failed_jobs: Vec<serde_json::Value> = jobs
        .iter()
        .map(|job| {
            serde_json::json!({
                "job_id": job.id,
                "name": job.name,
                "command": job.command,
            })
        })
        .collect();

    let result = serde_json::json!({
        "workflow_id": workflow_id,
        "failed_job_count": failed_jobs.len(),
        "failed_jobs": failed_jobs,
    });

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

/// List jobs by status.
pub fn list_jobs_by_status(
    config: &Configuration,
    workflow_id: i64,
    status: &str,
) -> Result<CallToolResult, McpError> {
    let status_enum: JobStatus = status
        .to_lowercase()
        .parse()
        .map_err(|_| invalid_params("Invalid status value"))?;

    let jobs = paginate_jobs(
        config,
        workflow_id,
        JobListParams::new().with_status(status_enum),
    )
    .map_err(|e| internal_error(format!("Failed to list jobs: {}", e)))?;

    let job_list: Vec<serde_json::Value> = jobs
        .iter()
        .map(|job| {
            serde_json::json!({
                "job_id": job.id,
                "name": job.name,
                "command": job.command,
            })
        })
        .collect();

    let result = serde_json::json!({
        "workflow_id": workflow_id,
        "status": status,
        "count": job_list.len(),
        "jobs": job_list,
    });

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

/// Check resource utilization for a workflow by running the CLI command.
pub fn check_resource_utilization(
    workflow_id: i64,
    include_failed: bool,
) -> Result<CallToolResult, McpError> {
    let mut cmd = Command::new("torc");
    cmd.args(["-f", "json", "reports", "check-resource-utilization"]);
    cmd.arg(workflow_id.to_string());

    if include_failed {
        cmd.arg("--include-failed");
    }

    let output = cmd
        .output()
        .map_err(|e| internal_error(format!("Failed to execute torc command: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(internal_error(format!(
            "torc command failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the JSON to check for over-utilization violations and add guidance
    let mut response = stdout.to_string();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let over_count = json
            .get("over_utilization_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let resource_violations_count = json
            .get("resource_violations")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        if over_count > 0 {
            response.push_str("\n\n[RECOVERABLE RESOURCE ISSUES DETECTED!");
            response.push_str(&format!(
                "\n{} job(s) exceeded their resource allocations.",
                over_count
            ));
            if resource_violations_count > 0 {
                response.push_str(&format!(
                    "\n{} job(s) with resource violations are ready for analysis.",
                    resource_violations_count
                ));
            }
            response
                .push_str("\n\nUSE THE recover_workflow TOOL TO AUTOMATICALLY FIX THESE ISSUES:");
            response.push_str(
                "\n1. Call recover_workflow with dry_run=true to preview the recovery actions",
            );
            response.push_str(
                "\n2. Show the user the preview (memory/runtime adjustments for each job)",
            );
            response.push_str(
                "\n3. Ask user: 'Would you like me to proceed with these recovery actions?'",
            );
            response
                .push_str("\n4. If approved, call recover_workflow with dry_run=false to execute");
            response.push_str(&format!(
                "\n\nExample: recover_workflow(workflow_id={}, dry_run=true)]",
                workflow_id
            ));
        }
    }

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        response,
    )]))
}

/// Update job resource requirements.
pub fn update_job_resources(
    config: &Configuration,
    job_id: i64,
    num_cpus: Option<i64>,
    memory: Option<String>,
    runtime: Option<String>,
) -> Result<CallToolResult, McpError> {
    // Get the job to find its resource requirements ID
    let job = default_api::get_job(config, job_id)
        .map_err(|e| internal_error(format!("Failed to get job: {}", e)))?;

    let req_id = job
        .resource_requirements_id
        .ok_or_else(|| invalid_params("Job does not have resource requirements to update"))?;

    // Get current requirements
    let mut reqs = default_api::get_resource_requirements(config, req_id)
        .map_err(|e| internal_error(format!("Failed to get resource requirements: {}", e)))?;

    // Update fields if provided
    if let Some(cpus) = num_cpus {
        reqs.num_cpus = cpus;
    }
    if let Some(mem) = memory {
        reqs.memory = mem;
    }
    if let Some(rt) = runtime {
        reqs.runtime = rt;
    }

    // Update the resource requirements
    let updated = default_api::update_resource_requirements(
        config,
        req_id,
        ResourceRequirementsModel {
            id: reqs.id,
            workflow_id: reqs.workflow_id,
            name: reqs.name.clone(),
            num_cpus: reqs.num_cpus,
            num_gpus: reqs.num_gpus,
            num_nodes: reqs.num_nodes,
            memory: reqs.memory.clone(),
            runtime: reqs.runtime.clone(),
        },
    )
    .map_err(|e| internal_error(format!("Failed to update resource requirements: {}", e)))?;

    // Get workflow_id for the restart instructions
    let workflow_id = job.workflow_id;

    let result = serde_json::json!({
        "success": true,
        "job_id": job_id,
        "workflow_id": workflow_id,
        "resource_requirements_id": req_id,
        "updated": {
            "num_cpus": updated.num_cpus,
            "num_gpus": updated.num_gpus,
            "memory": updated.memory,
            "runtime": updated.runtime,
        },
        "next_steps": {
            "note": "Resource updated. To restart the workflow after fixing all issues, \
                    use the recover_workflow tool (recommended) or manual commands.",
            "recommended": format!(
                "recover_workflow(workflow_id={}, dry_run=true) to preview, then dry_run=false to execute",
                workflow_id
            ),
        }
    });

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

/// Create a workflow from a JSON specification.
///
/// Supports:
/// - action: "validate" (validate only), "create_workflow" (create in database) or "save_spec_file" (save to filesystem)
/// - workflow_type: "local" or "slurm"
#[allow(clippy::too_many_arguments)]
pub fn create_workflow(
    config: &Configuration,
    spec_json: &str,
    user: &str,
    action: &str,
    workflow_type: &str,
    account: Option<&str>,
    hpc_profile: Option<&str>,
    output_path: Option<&str>,
) -> Result<CallToolResult, McpError> {
    use std::io::Write;
    use torc::client::workflow_spec::WorkflowSpec;

    // Validate action
    if action != "create_workflow" && action != "save_spec_file" && action != "validate" {
        return Err(invalid_params(
            "action must be 'validate', 'create_workflow' or 'save_spec_file'",
        ));
    }

    // Validate workflow_type
    if workflow_type != "local" && workflow_type != "slurm" {
        return Err(invalid_params("workflow_type must be 'local' or 'slurm'"));
    }

    // For slurm workflows, prompt user for account if not provided
    if workflow_type == "slurm" && account.is_none() {
        let prompt_msg = serde_json::json!({
            "status": "need_input",
            "message": "Slurm workflows require an account for job submission.",
            "action_required": "Please ask the user: What Slurm account should be used for this workflow? (This is typically a project or allocation name like 'myproject' or 'research-gpu')",
            "then": "Call this tool again with the account parameter set to the user's response."
        });
        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string_pretty(&prompt_msg).unwrap_or_default(),
        )]));
    }

    // Validate save_spec_file requirements
    if action == "save_spec_file" && output_path.is_none() {
        return Err(invalid_params(
            "output_path is required for save_spec_file action",
        ));
    }

    // Parse the spec
    let spec: serde_json::Value = serde_json::from_str(spec_json)
        .map_err(|e| invalid_params(&format!("Invalid workflow spec JSON: {}", e)))?;

    let name = spec
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed")
        .to_string();

    // For Slurm workflows, validate resource_requirements exist
    if workflow_type == "slurm" {
        // Check if resource_requirements section exists and has entries
        let has_resource_reqs = spec
            .get("resource_requirements")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        // Find jobs missing resource_requirements
        let jobs_missing_reqs: Vec<String> = spec
            .get("jobs")
            .and_then(|v| v.as_array())
            .map(|jobs| {
                jobs.iter()
                    .filter(|job| {
                        job.get("resource_requirements").is_none()
                            || job.get("resource_requirements") == Some(&serde_json::Value::Null)
                    })
                    .filter_map(|job| job.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // If missing resource_requirements section or jobs without assignments, return helpful error
        if !has_resource_reqs || !jobs_missing_reqs.is_empty() {
            let mut issues = Vec::new();

            if !has_resource_reqs {
                issues.push(
                    "The workflow spec is missing a 'resource_requirements' section.".to_string(),
                );
            }

            if !jobs_missing_reqs.is_empty() {
                issues.push(format!(
                    "The following jobs are missing resource_requirements: {}",
                    jobs_missing_reqs.join(", ")
                ));
            }

            let error_msg = serde_json::json!({
                "error": "missing_resource_requirements",
                "message": "Slurm workflows require resource requirements for all jobs.",
                "issues": issues,
                "help": "Please ask the user to specify resource requirements for their jobs. Each resource requirement needs: name, num_cpus (integer), memory (e.g., '4g', '512m'), runtime (ISO8601 duration like 'PT1H' for 1 hour, 'PT30M' for 30 minutes). Jobs can share requirements by referencing the same name. Example structure:",
                "example": {
                    "resource_requirements": [
                        {"name": "small", "num_cpus": 1, "memory": "2g", "runtime": "PT30M", "num_gpus": 0, "num_nodes": 1},
                        {"name": "large", "num_cpus": 8, "memory": "32g", "runtime": "PT4H", "num_gpus": 0, "num_nodes": 1}
                    ],
                    "jobs": [
                        {"name": "job1", "command": "...", "resource_requirements": "small"},
                        {"name": "job2", "command": "...", "resource_requirements": "large"}
                    ]
                }
            });

            return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&error_msg).unwrap_or_default(),
            )]));
        }
    }

    // Write spec to a temp file (needed for CLI commands)
    let mut temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| internal_error(format!("Failed to create temp file: {}", e)))?;

    temp_file
        .write_all(spec_json.as_bytes())
        .map_err(|e| internal_error(format!("Failed to write spec to temp file: {}", e)))?;

    let temp_path = temp_file.path();

    // Handle validate action - returns validation results without creating anything
    if action == "validate" {
        let validation_result = WorkflowSpec::validate_spec(temp_path);

        let result = serde_json::json!({
            "action": "validate",
            "valid": validation_result.valid,
            "errors": validation_result.errors,
            "warnings": validation_result.warnings,
            "summary": {
                "workflow_name": validation_result.summary.workflow_name,
                "workflow_description": validation_result.summary.workflow_description,
                "job_count": validation_result.summary.job_count,
                "job_count_before_expansion": validation_result.summary.job_count_before_expansion,
                "file_count": validation_result.summary.file_count,
                "file_count_before_expansion": validation_result.summary.file_count_before_expansion,
                "user_data_count": validation_result.summary.user_data_count,
                "resource_requirements_count": validation_result.summary.resource_requirements_count,
                "slurm_scheduler_count": validation_result.summary.slurm_scheduler_count,
                "action_count": validation_result.summary.action_count,
                "has_schedule_nodes_action": validation_result.summary.has_schedule_nodes_action,
                "job_names": validation_result.summary.job_names,
                "scheduler_names": validation_result.summary.scheduler_names,
            },
            "next_steps": if validation_result.valid {
                "Validation passed! Call this tool again with action='create_workflow' to create the workflow."
            } else {
                "Please fix the errors listed above and call validate again."
            }
        });

        return Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]));
    }

    match (action, workflow_type) {
        ("create_workflow", "local") => {
            // Create local workflow using the library function
            let workflow_id = torc::client::workflow_spec::WorkflowSpec::create_workflow_from_spec(
                config, temp_path, user, false, false,
            )
            .map_err(|e| internal_error(format!("Failed to create workflow: {}", e)))?;

            let result = serde_json::json!({
                "success": true,
                "workflow_id": workflow_id,
                "message": format!("Created local workflow '{}' with ID {}", name, workflow_id),
            });

            Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )]))
        }
        ("create_workflow", "slurm") => {
            // Create slurm workflow using CLI: torc workflows create-slurm
            let mut cmd = Command::new("torc");
            cmd.args(["-f", "json", "workflows", "create-slurm"]);
            cmd.args(["--account", account.unwrap()]);
            cmd.args(["--user", user]);

            if let Some(profile) = hpc_profile {
                cmd.args(["--hpc-profile", profile]);
            }

            cmd.arg(temp_path);

            let output = cmd
                .output()
                .map_err(|e| internal_error(format!("Failed to run torc command: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(internal_error(format!(
                    "Failed to create slurm workflow: {}",
                    stderr.trim()
                )));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Try to parse the workflow ID from the output
            let result = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout) {
                parsed
            } else {
                serde_json::json!({
                    "success": true,
                    "message": format!("Created slurm workflow '{}'", name),
                    "output": stdout.trim(),
                })
            };

            Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )]))
        }
        ("save_spec_file", "local") => {
            // Save the spec as JSON to the output path
            let output_path = output_path.unwrap();
            let content = serde_json::to_string_pretty(&spec)
                .map_err(|e| internal_error(format!("Failed to serialize spec: {}", e)))?;
            std::fs::write(output_path, &content)
                .map_err(|e| internal_error(format!("Failed to write spec file: {}", e)))?;

            let result = serde_json::json!({
                "success": true,
                "message": format!("Saved workflow spec '{}' to {}", name, output_path),
                "output_path": output_path,
            });

            Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )]))
        }
        ("save_spec_file", "slurm") => {
            // Generate slurm schedulers and save as JSON
            let output_path = output_path.unwrap();

            let mut cmd = Command::new("torc");
            cmd.args(["slurm", "generate"]);
            cmd.args(["--account", account.unwrap()]);
            cmd.args(["--output", output_path]);

            if let Some(profile) = hpc_profile {
                cmd.args(["--profile", profile]);
            }

            cmd.arg(temp_path);

            let output = cmd
                .output()
                .map_err(|e| internal_error(format!("Failed to run torc command: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(internal_error(format!(
                    "Failed to generate slurm spec: {}",
                    stderr.trim()
                )));
            }

            let result = serde_json::json!({
                "success": true,
                "message": format!("Generated slurm workflow spec '{}' at {}", name, output_path),
                "output_path": output_path,
            });

            Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&result).unwrap_or_default(),
            )]))
        }
        _ => Err(invalid_params("Invalid action/workflow_type combination")),
    }
}

/// Get the execution plan for a workflow.
///
/// Accepts either:
/// - A workflow ID (integer as string) for existing workflows
/// - A JSON workflow specification string for previewing before creation
pub fn get_execution_plan(
    config: &Configuration,
    spec_or_id: &str,
) -> Result<CallToolResult, McpError> {
    use std::io::Write;
    use torc::client::commands::pagination::resource_requirements::{
        ResourceRequirementsListParams, paginate_resource_requirements,
    };
    use torc::client::commands::pagination::slurm_schedulers::{
        SlurmSchedulersListParams, paginate_slurm_schedulers,
    };
    use torc::client::execution_plan::ExecutionPlan;
    use torc::client::workflow_spec::WorkflowSpec;

    // Try to parse as workflow ID first
    if let Ok(workflow_id) = spec_or_id.parse::<i64>() {
        // Get execution plan for existing workflow from database
        let workflow = default_api::get_workflow(config, workflow_id)
            .map_err(|e| internal_error(format!("Failed to get workflow: {}", e)))?;

        let jobs = paginate_jobs(
            config,
            workflow_id,
            JobListParams::new().with_include_relationships(true),
        )
        .map_err(|e| internal_error(format!("Failed to list jobs: {}", e)))?;

        let actions = default_api::get_workflow_actions(config, workflow_id)
            .map_err(|e| internal_error(format!("Failed to get workflow actions: {}", e)))?;

        let slurm_schedulers =
            paginate_slurm_schedulers(config, workflow_id, SlurmSchedulersListParams::new())
                .unwrap_or_default();

        let resource_requirements = paginate_resource_requirements(
            config,
            workflow_id,
            ResourceRequirementsListParams::new(),
        )
        .unwrap_or_default();

        let plan = ExecutionPlan::from_database_models(
            &workflow,
            &jobs,
            &actions,
            &slurm_schedulers,
            &resource_requirements,
        )
        .map_err(|e| internal_error(format!("Failed to build execution plan: {}", e)))?;

        // Build output JSON
        let events_json: Vec<serde_json::Value> = plan
            .events
            .values()
            .map(|event| {
                serde_json::json!({
                    "id": event.id,
                    "trigger": event.trigger,
                    "trigger_description": event.trigger_description,
                    "scheduler_allocations": event.scheduler_allocations.iter().map(|alloc| {
                        serde_json::json!({
                            "scheduler": alloc.scheduler,
                            "scheduler_type": alloc.scheduler_type,
                            "num_allocations": alloc.num_allocations,
                            "jobs": alloc.jobs,
                        })
                    }).collect::<Vec<_>>(),
                    "jobs_becoming_ready": event.jobs_becoming_ready,
                    "depends_on_events": event.depends_on_events,
                    "unlocks_events": event.unlocks_events,
                })
            })
            .collect();

        let result = serde_json::json!({
            "source": "database",
            "workflow_id": workflow_id,
            "workflow_name": workflow.name,
            "total_events": plan.events.len(),
            "total_jobs": jobs.len(),
            "root_events": plan.root_events,
            "leaf_events": plan.leaf_events,
            "events": events_json,
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    } else {
        // Try to parse as JSON workflow specification
        // Write spec to a temp file for WorkflowSpec::from_spec_file
        let mut temp_file = tempfile::NamedTempFile::new()
            .map_err(|e| internal_error(format!("Failed to create temp file: {}", e)))?;

        temp_file
            .write_all(spec_or_id.as_bytes())
            .map_err(|e| internal_error(format!("Failed to write spec to temp file: {}", e)))?;

        let temp_path = temp_file.path();

        // Parse the workflow spec
        let mut spec = WorkflowSpec::from_spec_file(temp_path).map_err(|e| {
            internal_error(format!("Failed to parse workflow specification: {}", e))
        })?;

        // Expand parameters
        spec.expand_parameters()
            .map_err(|e| internal_error(format!("Failed to expand parameters: {}", e)))?;

        // Validate actions
        spec.validate_actions()
            .map_err(|e| internal_error(format!("Failed to validate actions: {}", e)))?;

        // Perform variable substitution
        spec.substitute_variables()
            .map_err(|e| internal_error(format!("Failed to substitute variables: {}", e)))?;

        // Build execution plan from spec
        let plan = ExecutionPlan::from_spec(&spec)
            .map_err(|e| internal_error(format!("Failed to build execution plan: {}", e)))?;

        // Build output JSON
        let events_json: Vec<serde_json::Value> = plan
            .events
            .values()
            .map(|event| {
                serde_json::json!({
                    "id": event.id,
                    "trigger": event.trigger,
                    "trigger_description": event.trigger_description,
                    "scheduler_allocations": event.scheduler_allocations.iter().map(|alloc| {
                        serde_json::json!({
                            "scheduler": alloc.scheduler,
                            "scheduler_type": alloc.scheduler_type,
                            "num_allocations": alloc.num_allocations,
                            "jobs": alloc.jobs,
                        })
                    }).collect::<Vec<_>>(),
                    "jobs_becoming_ready": event.jobs_becoming_ready,
                    "depends_on_events": event.depends_on_events,
                    "unlocks_events": event.unlocks_events,
                })
            })
            .collect();

        let result = serde_json::json!({
            "source": "spec",
            "workflow_name": spec.name,
            "total_events": plan.events.len(),
            "total_jobs": spec.jobs.len(),
            "root_events": plan.root_events,
            "leaf_events": plan.leaf_events,
            "events": events_json,
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }
}

/// Analyze workflow logs for errors.
///
/// Scans all log files for a workflow and detects common error patterns like:
/// OOM, timeout, segfaults, permission denied, disk full, connection errors,
/// Python exceptions, Rust panics, and Slurm errors.
pub fn analyze_workflow_logs(
    output_dir: &Path,
    workflow_id: i64,
) -> Result<CallToolResult, McpError> {
    use torc::client::commands::logs::analyze_workflow_logs as analyze_logs;

    let result = analyze_logs(output_dir, workflow_id)
        .map_err(|e| internal_error(format!("Failed to analyze logs: {}", e)))?;

    // Build a concise summary for the AI
    let summary = if result.error_count == 0 && result.warning_count == 0 {
        "No errors or warnings detected in log files.".to_string()
    } else {
        format!(
            "Found {} error(s) and {} warning(s) across {} log files.",
            result.error_count, result.warning_count, result.files_parsed
        )
    };

    // Group errors by type for easy reading
    let errors_by_type: Vec<serde_json::Value> = result
        .errors_by_type
        .iter()
        .map(|(pattern, count)| {
            serde_json::json!({
                "type": pattern,
                "count": count,
            })
        })
        .collect();

    // Get sample errors (limit to 10 to avoid overwhelming the AI)
    let sample_errors: Vec<serde_json::Value> = result
        .errors
        .iter()
        .filter(|e| e.severity == torc::client::commands::logs::ErrorSeverity::Error)
        .take(10)
        .map(|e| {
            serde_json::json!({
                "file": e.file,
                "line": e.line_number,
                "type": e.pattern_name,
                "content": e.line_content,
            })
        })
        .collect();

    // Check for recoverable errors (OOM, timeout)
    let oom_count = result.errors_by_type.get("oom").copied().unwrap_or(0)
        + result
            .errors_by_type
            .get("memory_allocation_failed")
            .copied()
            .unwrap_or(0);
    let timeout_count = result.errors_by_type.get("timeout").copied().unwrap_or(0)
        + result
            .errors_by_type
            .get("time_limit")
            .copied()
            .unwrap_or(0);
    let has_recoverable_errors = oom_count > 0 || timeout_count > 0;

    let mut response = serde_json::json!({
        "workflow_id": workflow_id,
        "summary": summary,
        "files_parsed": result.files_parsed,
        "error_count": result.error_count,
        "warning_count": result.warning_count,
        "errors_by_type": errors_by_type,
        "sample_errors": sample_errors,
        "files_with_errors": result.errors_by_file.keys().collect::<Vec<_>>(),
    });

    // If recoverable errors found, add recovery guidance
    if has_recoverable_errors {
        let mut recovery_info = serde_json::json!({
            "oom_errors": oom_count,
            "timeout_errors": timeout_count,
        });

        recovery_info["recommendation"] = serde_json::json!(
            "RECOVERABLE ERRORS DETECTED! Use the recover_workflow tool to automatically fix these issues."
        );

        recovery_info["recovery_workflow"] = serde_json::json!([
            "1. Call recover_workflow with dry_run=true to preview the recovery actions",
            "2. Show the user the preview results (memory/runtime adjustments)",
            "3. Ask user: 'Would you like me to proceed with these recovery actions?'",
            "4. If approved, call recover_workflow with dry_run=false to execute"
        ]);

        recovery_info["tool_call_example"] = serde_json::json!({
            "tool": "recover_workflow",
            "parameters": {
                "workflow_id": workflow_id,
                "dry_run": true,
                "memory_multiplier": 1.5,
                "runtime_multiplier": 1.4,
            },
            "note": "Start with dry_run=true to preview changes"
        });

        if oom_count > 0 {
            recovery_info["oom_fix"] = serde_json::json!(format!(
                "{} job(s) ran out of memory. Recovery will increase memory by 1.5x (configurable).",
                oom_count
            ));
        }
        if timeout_count > 0 {
            recovery_info["timeout_fix"] = serde_json::json!(format!(
                "{} job(s) exceeded time limit. Recovery will increase runtime by 1.4x (configurable).",
                timeout_count
            ));
        }

        response["recovery"] = recovery_info;
    }

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}

/// Get workflow summary by running the CLI command.
pub fn get_workflow_summary(workflow_id: i64) -> Result<CallToolResult, McpError> {
    let output = Command::new("torc")
        .args(["-f", "json", "reports", "summary", &workflow_id.to_string()])
        .output()
        .map_err(|e| internal_error(format!("Failed to execute torc command: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(internal_error(format!(
            "torc command failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        stdout.to_string(),
    )]))
}

/// List job results with filtering options.
#[allow(clippy::too_many_arguments)]
pub fn list_results(
    workflow_id: i64,
    job_id: Option<i64>,
    run_id: Option<i64>,
    return_code: Option<i64>,
    failed_only: bool,
    status: Option<String>,
    limit: i64,
    sort_by: Option<String>,
    reverse_sort: bool,
) -> Result<CallToolResult, McpError> {
    let mut cmd = Command::new("torc");
    cmd.args(["-f", "json", "results", "list", &workflow_id.to_string()]);

    if let Some(jid) = job_id {
        cmd.args(["--job-id", &jid.to_string()]);
    }
    if let Some(rid) = run_id {
        cmd.args(["--run-id", &rid.to_string()]);
    }
    if let Some(rc) = return_code {
        cmd.args(["--return-code", &rc.to_string()]);
    }
    if failed_only {
        cmd.arg("--failed");
    }
    if let Some(s) = status {
        cmd.args(["--status", &s]);
    }
    cmd.args(["--limit", &limit.to_string()]);
    if let Some(sb) = sort_by {
        cmd.args(["--sort-by", &sb]);
    }
    if reverse_sort {
        cmd.arg("--reverse-sort");
    }

    let output = cmd
        .output()
        .map_err(|e| internal_error(format!("Failed to execute torc command: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(internal_error(format!(
            "torc command failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        stdout.to_string(),
    )]))
}

/// Get Slurm sacct accounting data for a workflow with walltime summary.
pub fn get_slurm_sacct(workflow_id: i64) -> Result<CallToolResult, McpError> {
    let output = Command::new("torc")
        .args(["-f", "json", "slurm", "sacct", &workflow_id.to_string()])
        .output()
        .map_err(|e| internal_error(format!("Failed to execute torc command: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(internal_error(format!(
            "torc command failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the JSON to calculate total walltime
    let mut response = stdout.to_string();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout)
        && let Some(rows) = json.get("rows").and_then(|r| r.as_array())
    {
        let mut total_seconds: i64 = 0;
        for row in rows {
            if let Some(elapsed) = row.get("elapsed").and_then(|e| e.as_str()) {
                total_seconds += parse_elapsed_to_seconds(elapsed);
            }
        }
        if total_seconds > 0 {
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            let summary = format!(
                "\n\n[SUMMARY: Total walltime consumed: {}h {}m {}s ({} seconds)]",
                hours, minutes, seconds, total_seconds
            );
            response.push_str(&summary);
        }
    }

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        response,
    )]))
}

/// Parse elapsed time string (e.g., "2h 15m", "45m 30s", "1d 2h 30m") to seconds.
fn parse_elapsed_to_seconds(elapsed: &str) -> i64 {
    let mut total = 0i64;
    let parts: Vec<&str> = elapsed.split_whitespace().collect();

    for part in parts {
        if let Some(num_str) = part.strip_suffix('d') {
            if let Ok(days) = num_str.parse::<i64>() {
                total += days * 86400;
            }
        } else if let Some(num_str) = part.strip_suffix('h') {
            if let Ok(hours) = num_str.parse::<i64>() {
                total += hours * 3600;
            }
        } else if let Some(num_str) = part.strip_suffix('m') {
            if let Ok(minutes) = num_str.parse::<i64>() {
                total += minutes * 60;
            }
        } else if let Some(num_str) = part.strip_suffix('s')
            && let Ok(seconds) = num_str.parse::<i64>()
        {
            total += seconds;
        }
    }

    total
}

/// Recover a Slurm workflow from failures.
///
/// This function runs `torc recover` with the specified parameters.
/// When dry_run is true, it shows what would be done without making changes.
pub fn recover_workflow(
    workflow_id: i64,
    output_dir: &Path,
    dry_run: bool,
    memory_multiplier: f64,
    runtime_multiplier: f64,
    retry_unknown: bool,
) -> Result<CallToolResult, McpError> {
    let mut cmd = Command::new("torc");
    cmd.args(["recover", &workflow_id.to_string()]);
    cmd.args(["--output-dir", &output_dir.display().to_string()]);
    cmd.args(["--memory-multiplier", &memory_multiplier.to_string()]);
    cmd.args(["--runtime-multiplier", &runtime_multiplier.to_string()]);

    if dry_run {
        cmd.arg("--dry-run");
    }

    if retry_unknown {
        cmd.arg("--retry-unknown");
    }

    let output = cmd
        .output()
        .map_err(|e| internal_error(format!("Failed to execute torc recover: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(internal_error(format!(
            "torc recover failed: {}",
            stderr.trim()
        )));
    }

    // Build a structured response
    let mut response = serde_json::json!({
        "workflow_id": workflow_id,
        "dry_run": dry_run,
        "memory_multiplier": memory_multiplier,
        "runtime_multiplier": runtime_multiplier,
        "retry_unknown": retry_unknown,
        "output": stdout.trim(),
    });

    // Add guidance based on dry_run mode
    if dry_run {
        response["next_steps"] = serde_json::json!({
            "instruction": "Review the recovery preview above. If the proposed changes look correct, \
                           ask the user: 'Would you like me to proceed with these recovery actions?'",
            "if_approved": "Call recover_workflow again with dry_run=false to execute the recovery.",
        });
    } else {
        response["status"] = serde_json::json!("Recovery complete");
        response["message"] = serde_json::json!(
            "The workflow has been recovered. Failed jobs have been reset, resources adjusted, \
             and Slurm allocations regenerated and submitted."
        );
    }

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}

/// List jobs with pending_failed status in a workflow.
///
/// These are jobs that failed without a matching failure handler and are awaiting
/// AI-assisted classification to determine whether they should be retried or marked as failed.
pub fn list_pending_failed_jobs(
    config: &Configuration,
    workflow_id: i64,
    output_dir: &Path,
) -> Result<CallToolResult, McpError> {
    let jobs = paginate_jobs(
        config,
        workflow_id,
        JobListParams::new().with_status(JobStatus::PendingFailed),
    )
    .map_err(|e| internal_error(format!("Failed to list jobs: {}", e)))?;

    // Get results and logs for each pending_failed job
    let mut pending_jobs: Vec<serde_json::Value> = Vec::new();
    for job in &jobs {
        let job_id = match job.id {
            Some(id) => id,
            None => {
                return Err(internal_error(format!(
                    "Encountered pending_failed job without an ID: name={:?}",
                    job.name
                )));
            }
        };

        // Get latest result
        let result = paginate_results(
            config,
            workflow_id,
            ResultListParams::new().with_job_id(job_id).with_limit(1),
        )
        .ok()
        .and_then(|items| items.into_iter().next());

        // Try to read stderr tail (last 50 lines)
        let (stderr_tail, stderr_read_error) = if let Some(ref res) = result {
            let run_id = res.run_id;
            let attempt_id = job.attempt_id.unwrap_or(1);
            let stderr_path =
                log_paths::get_job_stderr_path(output_dir, workflow_id, job_id, run_id, attempt_id);
            match fs::read_to_string(&stderr_path) {
                Ok(content) => {
                    // Efficiently get last 50 lines without loading full file into memory twice
                    let mut lines: Vec<&str> = content.lines().collect();
                    let tail_lines = if lines.len() > 50 {
                        lines.split_off(lines.len() - 50)
                    } else {
                        lines
                    };
                    (tail_lines.join("\n"), None)
                }
                Err(e) => (String::new(), Some(format!("Failed to read stderr: {}", e))),
            }
        } else {
            (String::new(), Some("No result found".to_string()))
        };

        pending_jobs.push(serde_json::json!({
            "job_id": job_id,
            "name": job.name,
            "command": job.command,
            "attempt_id": job.attempt_id,
            "return_code": result.as_ref().map(|r| r.return_code),
            "exec_time_minutes": result.as_ref().map(|r| r.exec_time_minutes),
            "stderr_tail": stderr_tail,
            "stderr_read_error": stderr_read_error,
        }));
    }

    let result = serde_json::json!({
        "workflow_id": workflow_id,
        "pending_failed_count": pending_jobs.len(),
        "pending_failed_jobs": pending_jobs,
        "guidance": if pending_jobs.is_empty() {
            "No jobs are awaiting classification."
        } else {
            "These jobs failed without a matching failure handler. Analyze the stderr output \
             to classify each failure as transient (retry) or permanent (fail). Use \
             classify_and_resolve_failures to act on your classification."
        },
    });

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    )]))
}

/// Classification decision for a pending_failed job.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FailureClassification {
    /// The job ID to classify
    pub job_id: i64,
    /// The classification action: "retry" or "fail"
    pub action: String,
    /// Optional new memory requirement (e.g., "8g")
    pub memory: Option<String>,
    /// Optional new runtime (ISO8601 duration, e.g., "PT2H")
    pub runtime: Option<String>,
    /// Reason for the classification (for logging)
    pub reason: Option<String>,
}

/// Classify and resolve pending_failed jobs.
///
/// This tool takes a list of classifications for pending_failed jobs and either:
/// - Sets them to "failed" status (triggering downstream cancellation)
/// - Resets them to "ready" status with bumped attempt_id for retry
///
/// Resource requirements can optionally be adjusted before retry.
pub fn classify_and_resolve_failures(
    config: &Configuration,
    workflow_id: i64,
    classifications: Vec<FailureClassification>,
    dry_run: bool,
) -> Result<CallToolResult, McpError> {
    // Check if workflow has use_pending_failed enabled
    let workflow = match default_api::get_workflow(config, workflow_id) {
        Ok(w) => w,
        Err(e) => {
            return Err(internal_error(format!(
                "Failed to get workflow {}: {}",
                workflow_id, e
            )));
        }
    };

    if !workflow.use_pending_failed.unwrap_or(false) {
        return Err(invalid_params(&format!(
            "Workflow {} does not have use_pending_failed enabled. \
             AI-assisted recovery is disabled for this workflow. \
             Jobs will use Failed status instead of PendingFailed.",
            workflow_id
        )));
    }

    // Note: We still validate individual job status below because there could be
    // edge cases where:
    // 1. use_pending_failed was toggled after jobs entered pending_failed status
    // 2. Jobs were manually set to pending_failed via API
    // The per-job validation ensures we only act on jobs genuinely in pending_failed status.

    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut jobs_to_retry: Vec<i64> = Vec::new();
    let mut jobs_to_fail: Vec<i64> = Vec::new();

    for classification in &classifications {
        let job_id = classification.job_id;
        let action = classification.action.to_lowercase();

        // Validate the job exists and is in pending_failed status
        let job = match default_api::get_job(config, job_id) {
            Ok(j) => j,
            Err(e) => {
                results.push(serde_json::json!({
                    "job_id": job_id,
                    "status": "error",
                    "message": format!("Failed to get job: {}", e),
                }));
                continue;
            }
        };

        if job.status != Some(JobStatus::PendingFailed) {
            results.push(serde_json::json!({
                "job_id": job_id,
                "status": "skipped",
                "message": format!("Job is not in pending_failed status (current: {:?})", job.status),
            }));
            continue;
        }

        // Validate action
        if action != "retry" && action != "fail" {
            results.push(serde_json::json!({
                "job_id": job_id,
                "status": "error",
                "message": format!("Invalid action '{}'. Must be 'retry' or 'fail'.", action),
            }));
            continue;
        }

        if dry_run {
            results.push(serde_json::json!({
                "job_id": job_id,
                "job_name": job.name,
                "action": action,
                "memory_adjustment": classification.memory,
                "runtime_adjustment": classification.runtime,
                "reason": classification.reason,
                "status": "would_apply",
            }));
        } else {
            // Apply resource adjustments if specified
            let resource_adjustment_warning = if (classification.memory.is_some()
                || classification.runtime.is_some())
                && action == "retry"
            {
                if let Some(req_id) = job.resource_requirements_id {
                    match default_api::get_resource_requirements(config, req_id) {
                        Ok(mut reqs) => {
                            if let Some(ref mem) = classification.memory {
                                reqs.memory = mem.clone();
                            }
                            if let Some(ref rt) = classification.runtime {
                                reqs.runtime = rt.clone();
                            }
                            match default_api::update_resource_requirements(
                                config,
                                req_id,
                                ResourceRequirementsModel {
                                    id: reqs.id,
                                    workflow_id: reqs.workflow_id,
                                    name: reqs.name.clone(),
                                    num_cpus: reqs.num_cpus,
                                    num_gpus: reqs.num_gpus,
                                    num_nodes: reqs.num_nodes,
                                    memory: reqs.memory.clone(),
                                    runtime: reqs.runtime.clone(),
                                },
                            ) {
                                Ok(_) => None,
                                Err(e) => Some(format!("Failed to update resources: {}", e)),
                            }
                        }
                        Err(e) => Some(format!("Failed to get resource requirements: {}", e)),
                    }
                } else {
                    Some("No resource requirements defined for this job".to_string())
                }
            } else {
                None
            };

            if action == "retry" {
                jobs_to_retry.push(job_id);
            } else {
                jobs_to_fail.push(job_id);
            }

            let mut job_result = serde_json::json!({
                "job_id": job_id,
                "job_name": job.name,
                "action": action,
                "reason": classification.reason,
                "status": "pending_application",
            });
            if let Some(warning) = resource_adjustment_warning {
                job_result["resource_adjustment_warning"] = serde_json::json!(warning);
            }
            results.push(job_result);
        }
    }

    // Apply the status changes using CLI commands (they handle the complex state transitions)
    if !dry_run && (!jobs_to_retry.is_empty() || !jobs_to_fail.is_empty()) {
        // For jobs to retry: reset to ready and bump attempt_id
        for job_id in &jobs_to_retry {
            // Use torc jobs update to reset to ready
            let output = Command::new("torc")
                .args(["jobs", "update", &job_id.to_string(), "--status", "ready"])
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    for r in &mut results {
                        if r.get("job_id").and_then(|v| v.as_i64()) == Some(*job_id) {
                            r["status"] = serde_json::json!("applied");
                        }
                    }
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    for r in &mut results {
                        if r.get("job_id").and_then(|v| v.as_i64()) == Some(*job_id) {
                            r["status"] = serde_json::json!("error");
                            r["message"] = serde_json::json!(format!(
                                "Failed to reset job: {}",
                                stderr.trim()
                            ));
                        }
                    }
                }
                Err(e) => {
                    for r in &mut results {
                        if r.get("job_id").and_then(|v| v.as_i64()) == Some(*job_id) {
                            r["status"] = serde_json::json!("error");
                            r["message"] = serde_json::json!(format!(
                                "Failed to spawn 'torc' command: {}. Is torc in PATH?",
                                e
                            ));
                        }
                    }
                }
            }
        }

        // For jobs to fail: set to failed status
        for job_id in &jobs_to_fail {
            let output = Command::new("torc")
                .args(["jobs", "update", &job_id.to_string(), "--status", "failed"])
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    for r in &mut results {
                        if r.get("job_id").and_then(|v| v.as_i64()) == Some(*job_id) {
                            r["status"] = serde_json::json!("applied");
                        }
                    }
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    for r in &mut results {
                        if r.get("job_id").and_then(|v| v.as_i64()) == Some(*job_id) {
                            r["status"] = serde_json::json!("error");
                            r["message"] = serde_json::json!(format!(
                                "Failed to mark as failed: {}",
                                stderr.trim()
                            ));
                        }
                    }
                }
                Err(e) => {
                    for r in &mut results {
                        if r.get("job_id").and_then(|v| v.as_i64()) == Some(*job_id) {
                            r["status"] = serde_json::json!("error");
                            r["message"] = serde_json::json!(format!(
                                "Failed to spawn 'torc' command: {}. Is torc in PATH?",
                                e
                            ));
                        }
                    }
                }
            }
        }
    }

    let response = serde_json::json!({
        "workflow_id": workflow_id,
        "dry_run": dry_run,
        "total_classifications": classifications.len(),
        "jobs_to_retry": jobs_to_retry.len(),
        "jobs_to_fail": jobs_to_fail.len(),
        "results": results,
        "next_steps": if dry_run {
            "Review the classifications above. If they look correct, call this tool again with dry_run=false to apply them."
        } else if !jobs_to_retry.is_empty() {
            "Classifications applied. Jobs marked for retry are now in 'ready' status. \
             You may need to regenerate Slurm schedulers if running on HPC."
        } else {
            "Classifications applied."
        },
    });

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}
