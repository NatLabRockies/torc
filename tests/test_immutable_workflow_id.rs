//! Update endpoints reject a changed `workflow_id` (422) and leave the record untouched, while
//! a re-send with the stored `workflow_id` still succeeds.

// The generated client's error type is large; the closures here just forward it.
#![allow(clippy::result_large_err)]

mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_file, create_test_job,
    create_test_resource_requirements, create_test_result, create_test_slurm_scheduler,
    create_test_user_data, create_test_workflow, start_server,
};
use rstest::rstest;
use std::collections::HashMap;
use std::fmt::Debug;
use torc::client::Configuration;
use torc::client::apis;
use torc::models;

/// Assert that updating `record` into workflow `other` is a 422 that leaves it unchanged, and
/// that updating it with its own `workflow_id` succeeds.
fn assert_immutable<M: Clone + Debug + PartialEq, E1: Debug, E2: Debug>(
    record: M,
    other: i64,
    set_workflow_id: impl Fn(&mut M, i64),
    get: impl Fn() -> Result<M, apis::Error<E1>>,
    update: impl Fn(M) -> Result<M, apis::Error<E2>>,
) {
    let before = get().expect("get before update");
    let mut moved = record.clone();
    set_workflow_id(&mut moved, other);
    match update(moved) {
        Err(apis::Error::ResponseError(r)) => {
            assert_eq!(r.status.as_u16(), 422, "{}", r.content);
            assert!(
                r.content.contains("workflow_id cannot be changed"),
                "{}",
                r.content
            );
        }
        other => panic!("expected 422, got {other:?}"),
    }
    assert_eq!(get().expect("get after rejected update"), before);
    update(record).expect("update with unchanged workflow_id");
}

/// Two workflows: the record's own and another one it must not move to.
fn workflows(config: &Configuration, name: &str) -> (i64, i64) {
    (
        create_test_workflow(config, &format!("{name}_a"))
            .id
            .unwrap(),
        create_test_workflow(config, &format!("{name}_b"))
            .id
            .unwrap(),
    )
}

#[rstest]
fn test_resource_requirements(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_rr");
    let rr = create_test_resource_requirements(config, wf, "rr", 1, 0, 1, "1g", "PT1M");
    let id = rr.id.unwrap();
    assert_immutable(
        rr,
        other,
        |m, w| m.workflow_id = w,
        || apis::resource_requirements_api::get_resource_requirements(config, id),
        |m| apis::resource_requirements_api::update_resource_requirements(config, id, m),
    );
}

#[rstest]
fn test_file(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_file");
    let file = create_test_file(config, wf, "f", "/tmp/f");
    let id = file.id.unwrap();
    assert_immutable(
        file,
        other,
        |m, w| m.workflow_id = w,
        || apis::files_api::get_file(config, id),
        |m| apis::files_api::update_file(config, id, m),
    );
}

#[rstest]
fn test_user_data(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_ud");
    let ud = create_test_user_data(config, wf, "ud", serde_json::json!({"a": 1}), false);
    let id = ud.id.unwrap();
    assert_immutable(
        ud,
        other,
        |m, w| m.workflow_id = w,
        || apis::user_data_api::get_user_data(config, id),
        |m| apis::user_data_api::update_user_data(config, id, m),
    );
}

#[rstest]
fn test_slurm_scheduler(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_slurm");
    let id = create_test_slurm_scheduler(config, wf);
    let sched = apis::slurm_schedulers_api::get_slurm_scheduler(config, id).unwrap();
    assert_immutable(
        sched,
        other,
        |m, w| m.workflow_id = w,
        || apis::slurm_schedulers_api::get_slurm_scheduler(config, id),
        |m| apis::slurm_schedulers_api::update_slurm_scheduler(config, id, m),
    );
}

#[rstest]
fn test_local_scheduler(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_local");
    let mut sched = models::LocalSchedulerModel::new(wf);
    sched.memory = Some("1g".to_string());
    sched.num_cpus = Some(1);
    let sched = apis::local_schedulers_api::create_local_scheduler(config, sched).unwrap();
    let id = sched.id.unwrap();
    assert_immutable(
        sched,
        other,
        |m, w| m.workflow_id = w,
        || apis::local_schedulers_api::get_local_scheduler(config, id),
        |m| apis::local_schedulers_api::update_local_scheduler(config, id, m),
    );
}

#[rstest]
fn test_scheduled_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_scn");
    let scheduler_id = create_test_slurm_scheduler(config, wf);
    let node = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(
        config,
        models::ScheduledComputeNodesModel::new(
            wf,
            12345,
            scheduler_id,
            "slurm".to_string(),
            "pending".to_string(),
        ),
    )
    .unwrap();
    let id = node.id.unwrap();
    assert_immutable(
        node,
        other,
        |m, w| m.workflow_id = w,
        || apis::scheduled_compute_nodes_api::get_scheduled_compute_node(config, id),
        |m| apis::scheduled_compute_nodes_api::update_scheduled_compute_node(config, id, m),
    );
}

#[rstest]
fn test_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_cn");
    let node = create_test_compute_node(config, wf);
    let id = node.id.unwrap();
    assert_immutable(
        node,
        other,
        |m, w| m.workflow_id = w,
        || apis::compute_nodes_api::get_compute_node(config, id),
        |m| apis::compute_nodes_api::update_compute_node(config, id, m),
    );
}

#[rstest]
fn test_result(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_result");
    let job_id = create_test_job(config, wf, "job").id.unwrap();
    let result = create_test_result(config, wf, job_id);
    let id = result.id.unwrap();
    assert_immutable(
        result,
        other,
        |m, w| m.workflow_id = w,
        || apis::results_api::get_result(config, id),
        |m| apis::results_api::update_result(config, id, m),
    );
}

#[rstest]
fn test_ro_crate_entity(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_ro_crate");
    let entity = apis::ro_crate_entities_api::create_ro_crate_entity(
        config,
        models::RoCrateEntityModel::new(
            wf,
            "#entity".to_string(),
            "Dataset".to_string(),
            HashMap::new(),
        ),
    )
    .unwrap();
    let id = entity.id.unwrap();
    assert_immutable(
        entity,
        other,
        |m, w| m.workflow_id = w,
        || apis::ro_crate_entities_api::get_ro_crate_entity(config, id),
        |m| apis::ro_crate_entities_api::update_ro_crate_entity(config, id, m),
    );
}

#[rstest]
fn test_job(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, other) = workflows(config, "immutable_job");
    let job = create_test_job(config, wf, "job");
    let id = job.id.unwrap();
    assert_immutable(
        job,
        other,
        |m, w| m.workflow_id = w,
        || apis::jobs_api::get_job(config, id),
        |m| apis::jobs_api::update_job(config, id, m),
    );
}

#[rstest]
fn test_update_missing_record_is_404(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf, _) = workflows(config, "immutable_missing");
    let file = create_test_file(config, wf, "f", "/tmp/f");
    let id = file.id.unwrap();
    apis::files_api::delete_file(config, id).unwrap();
    match apis::files_api::update_file(config, id, file) {
        Err(apis::Error::ResponseError(r)) => assert_eq!(r.status.as_u16(), 404, "{}", r.content),
        other => panic!("expected 404, got {other:?}"),
    }
}
