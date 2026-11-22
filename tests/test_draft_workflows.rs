//! Draft workflows, finalize_workflow, abandoned-draft cleanup, and name_refs on create_jobs
//! (#524).

mod common;

use common::{ServerProcess, start_server, start_server_with_env};
use rstest::rstest;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use torc::client::{Configuration, apis};
use torc::models::{self, JobNameRefs};

fn create_workflow(config: &Configuration, name: &str, is_draft: bool) -> i64 {
    let mut workflow = models::WorkflowModel::new(name.to_string(), "test_user".to_string());
    workflow.is_draft = Some(is_draft);
    apis::workflows_api::create_workflow(config, workflow)
        .expect("create workflow")
        .id
        .unwrap()
}

fn create_files(config: &Configuration, workflow_id: i64, names: &[&str]) -> HashMap<String, i64> {
    let files = names
        .iter()
        .map(|n| models::FileModel::new(workflow_id, n.to_string(), format!("/tmp/{}", n)))
        .collect();
    apis::files_api::create_files(config, models::FilesModel::new(files))
        .expect("create files")
        .files
        .unwrap()
        .into_iter()
        .map(|f| (f.name, f.id.unwrap()))
        .collect()
}

fn job(workflow_id: i64, name: &str, refs: JobNameRefs) -> models::JobModel {
    let mut job = models::JobModel::new(workflow_id, name.to_string(), "true".to_string());
    job.name_refs = Some(refs);
    job
}

/// Create jobs in one request; return name -> created job.
fn create_jobs(
    config: &Configuration,
    jobs: Vec<models::JobModel>,
) -> HashMap<String, models::JobModel> {
    apis::jobs_api::create_jobs(config, models::JobsModel::new(jobs))
        .expect("create jobs")
        .jobs
        .unwrap()
        .into_iter()
        .map(|j| (j.name.clone(), j))
        .collect()
}

/// Status and message of a failed create_jobs call.
fn create_jobs_error(config: &Configuration, jobs: Vec<models::JobModel>) -> (u16, String) {
    match apis::jobs_api::create_jobs(config, models::JobsModel::new(jobs)) {
        Err(apis::Error::ResponseError(r)) => (r.status.as_u16(), r.content),
        other => panic!("expected an error response, got {:?}", other.map(|_| ())),
    }
}

fn sorted(mut ids: Vec<i64>) -> Vec<i64> {
    ids.sort();
    ids
}

fn names(v: &[&str]) -> Option<Vec<String>> {
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn listed(config: &Configuration, workflow_id: i64) -> bool {
    apis::workflows_api::list_workflows(
        config, None, None, None, None, None, None, None, None, None,
    )
    .expect("list workflows")
    .items
    .iter()
    .any(|w| w.id == Some(workflow_id))
}

fn finalize(
    config: &Configuration,
    workflow_id: i64,
    force: bool,
) -> models::FinalizeWorkflowResponse {
    apis::workflows_api::finalize_workflow(config, workflow_id, Some(force)).expect("finalize")
}

#[rstest]
fn test_name_refs_resolve_within_workflow_and_batch(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_workflow(config, "name_refs", true);
    let files = create_files(config, wf, &["f_in", "f_mid", "f_out"]);
    let ud = apis::user_data_api::create_user_data_list(
        config,
        models::UserDataListModel::new(vec![models::UserDataModel::new(wf, "params".into())]),
    )
    .unwrap()
    .user_data
    .unwrap()[0]
        .id
        .unwrap();
    let rr = apis::resource_requirements_api::create_resource_requirements(
        config,
        models::ResourceRequirementsModel::new(wf, "small".into()),
    )
    .unwrap()
    .id
    .unwrap();
    let mut sched = models::SlurmSchedulerModel::new(wf, "acct".into(), 1, "01:00:00".into());
    sched.name = Some("short".into());
    let sched = apis::slurm_schedulers_api::create_slurm_scheduler(config, sched)
        .unwrap()
        .id
        .unwrap();
    let fh = apis::failure_handlers_api::create_failure_handler(
        config,
        models::FailureHandlerModel::new(wf, "retry".into(), "[]".into()),
    )
    .unwrap()
    .id
    .unwrap();

    // Batch 1: "b" depends on "a" from the same request; the request lists "b" first.
    let batch1 = create_jobs(
        config,
        vec![
            job(
                wf,
                "b",
                JobNameRefs {
                    depends_on: names(&["a"]),
                    input_files: names(&["f_mid"]),
                    output_files: names(&["f_out"]),
                    input_user_data: names(&["params"]),
                    resource_requirements: Some("small".into()),
                    scheduler: Some("short".into()),
                    failure_handler: Some("retry".into()),
                    ..Default::default()
                },
            ),
            job(
                wf,
                "a",
                JobNameRefs {
                    input_files: names(&["f_in"]),
                    output_files: names(&["f_mid"]),
                    ..Default::default()
                },
            ),
        ],
    );
    let a = batch1["a"].id.unwrap();
    let b = batch1["b"].id.unwrap();
    assert!(batch1["b"].name_refs.is_none());
    let stored_b = apis::jobs_api::get_job(config, b).unwrap();
    assert_eq!(stored_b.depends_on_job_ids, Some(vec![a]));
    assert_eq!(stored_b.input_file_ids, Some(vec![files["f_mid"]]));
    assert_eq!(stored_b.output_file_ids, Some(vec![files["f_out"]]));
    assert_eq!(stored_b.input_user_data_ids, Some(vec![ud]));
    assert_eq!(stored_b.resource_requirements_id, Some(rr));
    assert_eq!(stored_b.scheduler_id, Some(sched));
    assert_eq!(stored_b.failure_handler_id, Some(fh));

    // Batch 2: regexes match rows created by earlier requests.
    let batch2 = create_jobs(
        config,
        vec![job(
            wf,
            "c",
            JobNameRefs {
                depends_on_regexes: names(&["^[ab]$"]),
                input_file_regexes: names(&["^f_(in|out)$"]),
                ..Default::default()
            },
        )],
    );
    let c = apis::jobs_api::get_job(config, batch2["c"].id.unwrap()).unwrap();
    assert_eq!(sorted(c.depends_on_job_ids.unwrap()), sorted(vec![a, b]));
    assert_eq!(
        sorted(c.input_file_ids.unwrap()),
        sorted(vec![files["f_in"], files["f_out"]])
    );
    // A job without a named resource requirement still gets the "default" record.
    assert_ne!(c.resource_requirements_id, Some(rr));
    assert!(c.resource_requirements_id.is_some());
}

#[rstest]
fn test_name_refs_errors(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_workflow(config, "name_ref_errors", true);
    create_files(config, wf, &["f1"]);
    create_jobs(config, vec![job(wf, "existing", JobNameRefs::default())]);

    let cases: Vec<(Vec<models::JobModel>, &str)> = vec![
        (
            vec![job(
                wf,
                "x",
                JobNameRefs {
                    depends_on: names(&["later"]),
                    ..Default::default()
                },
            )],
            "Job dependency 'later' not found for job 'x' (a name must be a job already in",
        ),
        (
            vec![job(
                wf,
                "x",
                JobNameRefs {
                    input_file_regexes: names(&["^nomatch"]),
                    ..Default::default()
                },
            )],
            "Input file regex '^nomatch' did not match any names for job 'x'",
        ),
        (
            vec![job(
                wf,
                "x",
                JobNameRefs {
                    output_file_regexes: names(&["("]),
                    ..Default::default()
                },
            )],
            "Invalid regex '(' for output file in job 'x'",
        ),
        (
            vec![job(
                wf,
                "x",
                JobNameRefs {
                    resource_requirements: Some("missing".into()),
                    ..Default::default()
                },
            )],
            "Resource requirements 'missing' not found for job 'x'",
        ),
        (
            vec![
                job(
                    wf,
                    "p",
                    JobNameRefs {
                        depends_on: names(&["q"]),
                        ..Default::default()
                    },
                ),
                job(
                    wf,
                    "q",
                    JobNameRefs {
                        depends_on: names(&["p"]),
                        ..Default::default()
                    },
                ),
            ],
            "Job dependencies in this request contain a cycle",
        ),
        (
            vec![job(
                wf,
                "x",
                JobNameRefs {
                    depends_on_regexes: names(&["^x$"]),
                    ..Default::default()
                },
            )],
            "contain a cycle (job depends on job): 'x' -> 'x'",
        ),
        (
            // A batch job named like an existing one makes the name ambiguous.
            vec![
                job(wf, "existing", JobNameRefs::default()),
                job(
                    wf,
                    "y",
                    JobNameRefs {
                        depends_on: names(&["existing"]),
                        ..Default::default()
                    },
                ),
            ],
            "Job dependency 'existing' for job 'y' is ambiguous",
        ),
    ];
    for (jobs, expected) in cases {
        let (status, body) = create_jobs_error(config, jobs);
        assert_eq!(status, 422, "{}", body);
        assert!(
            body.contains(expected),
            "expected '{}' in {}",
            expected,
            body
        );
    }

    // Rejected requests create nothing.
    let listed = apis::jobs_api::list_jobs(
        config, wf, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None,
    )
    .unwrap();
    assert_eq!(listed.total_count, 1);

    // The single-job endpoint does not resolve names.
    let single = apis::jobs_api::create_job(
        config,
        job(
            wf,
            "z",
            JobNameRefs {
                depends_on: names(&["existing"]),
                ..Default::default()
            },
        ),
    );
    match single {
        Err(apis::Error::ResponseError(r)) => assert_eq!(r.status.as_u16(), 422),
        other => panic!("expected 422, got {:?}", other.map(|_| ())),
    }
}

#[rstest]
fn test_draft_hidden_and_not_initializable_until_finalized(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_workflow(config, "draft_visibility", true);
    create_jobs(config, vec![job(wf, "only", JobNameRefs::default())]);

    assert!(!listed(config, wf));
    assert_eq!(
        apis::workflows_api::get_workflow(config, wf)
            .unwrap()
            .is_draft,
        Some(true)
    );
    match apis::workflows_api::initialize_jobs(config, wf, None, None, None) {
        Err(apis::Error::ResponseError(r)) => {
            assert_eq!(r.status.as_u16(), 422);
            assert!(r.content.contains("is a draft"), "{}", r.content);
        }
        other => panic!("expected 422, got {:?}", other.map(|_| ())),
    }

    let result = finalize(config, wf, false);
    assert!(result.finalized, "{:?}", result);
    assert!(result.errors.is_empty() && result.warnings.is_empty());
    assert!(listed(config, wf));
    assert_eq!(
        apis::workflows_api::get_workflow(config, wf)
            .unwrap()
            .is_draft,
        Some(false)
    );
    apis::workflows_api::initialize_jobs(config, wf, None, None, None).expect("initialize");

    // Finalizing again, or finalizing a workflow that was never a draft, is a no-op.
    assert!(finalize(config, wf, false).finalized);
    let regular = create_workflow(config, "not_a_draft", false);
    assert!(listed(config, regular));
    assert!(finalize(config, regular, false).finalized);
}

#[rstest]
fn test_finalize_rejects_cross_batch_problems(start_server: &ServerProcess) {
    let config = &start_server.config;

    // A cycle through files: x reads f2 and writes f1, y reads f1 and writes f2. Each request
    // is valid on its own.
    let wf = create_workflow(config, "finalize_cycle", true);
    create_files(config, wf, &["f1", "f2"]);
    for (name, input, output) in [("x", "f2", "f1"), ("y", "f1", "f2")] {
        create_jobs(
            config,
            vec![job(
                wf,
                name,
                JobNameRefs {
                    input_files: names(&[input]),
                    output_files: names(&[output]),
                    ..Default::default()
                },
            )],
        );
    }
    let result = finalize(config, wf, true);
    assert!(!result.finalized);
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert!(result.errors[0].contains("cycle"), "{:?}", result.errors);
    assert!(
        result.errors[0].contains("'x'") && result.errors[0].contains("'y'"),
        "{:?}",
        result.errors
    );
    assert!(!listed(config, wf));

    // An action pointing at a scheduler outside the workflow, plus an execution_config rule.
    let mut workflow = models::WorkflowModel::new("finalize_refs".into(), "test_user".into());
    workflow.is_draft = Some(true);
    workflow.execution_config = Some(models::ExecutionConfig {
        mode: models::ExecutionMode::Slurm,
        termination_signal: Some("SIGTERM".into()),
        ..Default::default()
    });
    let wf = apis::workflows_api::create_workflow(config, workflow)
        .unwrap()
        .id
        .unwrap();
    let action = models::WorkflowActionModel {
        id: None,
        workflow_id: wf,
        trigger_type: "on_workflow_start".into(),
        action_type: "schedule_nodes".into(),
        action_config: serde_json::json!({
            "scheduler_type": "slurm",
            "scheduler_id": 999_999,
            "num_allocations": 1,
        }),
        job_ids: None,
        trigger_count: 0,
        required_triggers: 1,
        executed: false,
        executed_at: None,
        executed_by: None,
        persistent: false,
        is_recovery: false,
        state: None,
        error: None,
    };
    apis::workflow_actions_api::create_workflow_action(config, wf, action).unwrap();
    let result = finalize(config, wf, true);
    assert!(!result.finalized);
    assert!(
        result
            .errors
            .iter()
            .any(|e| e.contains("scheduler_id=999999")),
        "{:?}",
        result.errors
    );
    assert!(
        result
            .errors
            .iter()
            .any(|e| e.contains("termination_signal is only supported in direct mode")),
        "{:?}",
        result.errors
    );
}

#[rstest]
fn test_finalize_warnings_need_force(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_workflow(config, "finalize_warnings", true);
    let mut rr = models::ResourceRequirementsModel::new(wf, "big".into());
    rr.memory = "100g".into();
    apis::resource_requirements_api::create_resource_requirements(config, rr).unwrap();
    let mut sched = models::SlurmSchedulerModel::new(wf, "acct".into(), 1, "01:00:00".into());
    sched.name = Some("small_nodes".into());
    sched.mem = Some("1g".into());
    apis::slurm_schedulers_api::create_slurm_scheduler(config, sched).unwrap();
    create_jobs(
        config,
        vec![job(
            wf,
            "hungry",
            JobNameRefs {
                resource_requirements: Some("big".into()),
                scheduler: Some("small_nodes".into()),
                ..Default::default()
            },
        )],
    );

    let result = finalize(config, wf, false);
    assert!(!result.finalized);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.warnings.len(), 1, "{:?}", result.warnings);
    assert!(
        result.warnings[0].contains("Job 'hungry': memory '100g' exceeds scheduler 'small_nodes'"),
        "{:?}",
        result.warnings
    );
    assert!(!listed(config, wf));

    let result = finalize(config, wf, true);
    assert!(result.finalized);
    assert_eq!(result.warnings.len(), 1);
    assert!(listed(config, wf));
}

#[test]
fn test_abandoned_drafts_are_deleted() {
    let server = start_server_with_env(&[("TORC_DRAFT_WORKFLOW_TIMEOUT_SECS", "2")]);
    let config = &server.config;
    let draft = create_workflow(config, "abandoned", true);
    create_jobs(config, vec![job(draft, "j", JobNameRefs::default())]);
    let finalized = create_workflow(config, "kept_draft", true);
    assert!(finalize(config, finalized, false).finalized);
    let regular = create_workflow(config, "kept", false);

    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match apis::workflows_api::get_workflow(config, draft) {
            Err(apis::Error::ResponseError(r)) if r.status.as_u16() == 404 => break,
            Ok(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(200)),
            other => panic!("draft was not deleted: {:?}", other.map(|w| w.id)),
        }
    }
    apis::workflows_api::get_workflow(config, finalized).expect("finalized workflow kept");
    apis::workflows_api::get_workflow(config, regular).expect("regular workflow kept");
}
