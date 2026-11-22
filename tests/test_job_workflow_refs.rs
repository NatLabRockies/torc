//! A job may only reference records (jobs, files, user data, resource requirements, schedulers,
//! failure handlers) in its own workflow.

mod common;

use common::{
    ServerProcess, create_test_file, create_test_job, create_test_resource_requirements,
    create_test_slurm_scheduler, create_test_user_data, create_test_workflow, start_server,
};
use rstest::rstest;
use torc::client::Configuration;
use torc::client::apis;
use torc::models;

/// One record of each kind a job can reference, all in one workflow.
struct Refs {
    workflow_id: i64,
    job: i64,
    file: i64,
    user_data: i64,
    rr: i64,
    scheduler: i64,
    failure_handler: i64,
}

fn create_refs(config: &Configuration, name: &str) -> Refs {
    let workflow_id = create_test_workflow(config, name).id.unwrap();
    let failure_handler =
        models::FailureHandlerModel::new(workflow_id, "fh".to_string(), "[]".to_string());
    Refs {
        workflow_id,
        job: create_test_job(config, workflow_id, "upstream").id.unwrap(),
        file: create_test_file(config, workflow_id, "f", "/tmp/f")
            .id
            .unwrap(),
        user_data: create_test_user_data(config, workflow_id, "ud", serde_json::json!({}), false)
            .id
            .unwrap(),
        rr: create_test_resource_requirements(config, workflow_id, "rr", 1, 0, 1, "1g", "PT1M")
            .id
            .unwrap(),
        scheduler: create_test_slurm_scheduler(config, workflow_id),
        failure_handler: apis::failure_handlers_api::create_failure_handler(
            config,
            failure_handler,
        )
        .unwrap()
        .id
        .unwrap(),
    }
}

/// Set `field` on `job` to the matching record from `refs`.
fn set_field(job: &mut models::JobModel, field: &str, refs: &Refs) {
    match field {
        "depends_on_job_ids" => job.depends_on_job_ids = Some(vec![refs.job]),
        "input_file_ids" => job.input_file_ids = Some(vec![refs.file]),
        "output_file_ids" => job.output_file_ids = Some(vec![refs.file]),
        "input_user_data_ids" => job.input_user_data_ids = Some(vec![refs.user_data]),
        "output_user_data_ids" => job.output_user_data_ids = Some(vec![refs.user_data]),
        "resource_requirements_id" => job.resource_requirements_id = Some(refs.rr),
        "scheduler_id" => job.scheduler_id = Some(refs.scheduler),
        "failure_handler_id" => job.failure_handler_id = Some(refs.failure_handler),
        _ => unreachable!(),
    }
}

const FIELDS: [&str; 8] = [
    "depends_on_job_ids",
    "input_file_ids",
    "output_file_ids",
    "input_user_data_ids",
    "output_user_data_ids",
    "resource_requirements_id",
    "scheduler_id",
    "failure_handler_id",
];

fn assert_422<T: std::fmt::Debug>(err: apis::Error<T>, field: &str) {
    match &err {
        apis::Error::ResponseError(r) => {
            assert_eq!(r.status.as_u16(), 422, "{field}: {:?}", r.content);
            assert!(r.content.contains(field), "{field}: {}", r.content);
        }
        _ => panic!("{field}: expected ResponseError, got {err:?}"),
    }
}

#[rstest]
fn test_create_job_rejects_cross_workflow_refs(start_server: &ServerProcess) {
    let config = &start_server.config;
    let a = create_refs(config, "refs_create_a");
    let b = create_refs(config, "refs_create_b");

    for field in FIELDS {
        let mut job = models::JobModel::new(a.workflow_id, format!("j_{field}"), "true".into());
        set_field(&mut job, field, &b);
        assert_422(apis::jobs_api::create_job(config, job).unwrap_err(), field);

        let mut job = models::JobModel::new(a.workflow_id, format!("bulk_{field}"), "true".into());
        set_field(&mut job, field, &b);
        let ok = models::JobModel::new(a.workflow_id, format!("ok_{field}"), "true".into());
        assert_422(
            apis::jobs_api::create_jobs(config, models::JobsModel::new(vec![ok, job])).unwrap_err(),
            field,
        );
    }

    // Nothing from the rejected batches was committed.
    let jobs = apis::jobs_api::list_jobs(
        config,
        a.workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(jobs.items.len(), 1, "only the upstream job should exist");
}

#[rstest]
fn test_create_job_accepts_same_workflow_refs(start_server: &ServerProcess) {
    let config = &start_server.config;
    let a = create_refs(config, "refs_same");

    let all_refs = |name: &str| {
        let mut job = models::JobModel::new(a.workflow_id, name.to_string(), "true".into());
        for field in FIELDS {
            set_field(&mut job, field, &a);
        }
        job
    };
    let created = apis::jobs_api::create_job(config, all_refs("single")).unwrap();
    assert_eq!(created.scheduler_id, Some(a.scheduler));
    let bulk = apis::jobs_api::create_jobs(
        config,
        models::JobsModel::new(vec![all_refs("bulk1"), all_refs("bulk2")]),
    )
    .unwrap();
    assert_eq!(bulk.jobs.unwrap().len(), 2);
}

#[rstest]
fn test_update_job_rejects_cross_workflow_refs(start_server: &ServerProcess) {
    let config = &start_server.config;
    let a = create_refs(config, "refs_update_a");
    let b = create_refs(config, "refs_update_b");
    let mut job = create_test_job(config, a.workflow_id, "target");
    let job_id = job.id.unwrap();

    for field in [
        "depends_on_job_ids",
        "resource_requirements_id",
        "scheduler_id",
    ] {
        let mut body = job.clone();
        set_field(&mut body, field, &b);
        assert_422(
            apis::jobs_api::update_job(config, job_id, body).unwrap_err(),
            field,
        );

        set_field(&mut job, field, &a);
        job = apis::jobs_api::update_job(config, job_id, job).unwrap();
    }

    let updated = apis::jobs_api::get_job(config, job_id).unwrap();
    assert_eq!(updated.depends_on_job_ids, Some(vec![a.job]));
    assert_eq!(updated.resource_requirements_id, Some(a.rr));
    assert_eq!(updated.scheduler_id, Some(a.scheduler));
}

#[rstest]
fn test_create_job_accepts_local_scheduler(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "refs_local_sched").id.unwrap();
    let mut local = models::LocalSchedulerModel::new(workflow_id);
    local.memory = Some("1g".into());
    local.num_cpus = Some(1);
    let local = apis::local_schedulers_api::create_local_scheduler(config, local)
        .unwrap()
        .id
        .unwrap();
    let mut job = models::JobModel::new(workflow_id, "local".into(), "true".into());
    job.scheduler_id = Some(local);
    let created = apis::jobs_api::create_job(config, job).unwrap();
    assert_eq!(created.scheduler_id, Some(local));
}
