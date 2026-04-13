mod common;

use common::{ServerProcess, create_test_job, create_test_workflow, start_server};
use rstest::rstest;
use torc::client::apis;
use torc::models;

fn create_test_slurm_stats(
    config: &torc::Configuration,
    workflow_id: i64,
    job_id: i64,
) -> models::SlurmStatsModel {
    let mut stats = models::SlurmStatsModel::new(workflow_id, job_id, 1, 1);
    stats.slurm_job_id = Some("12345678".to_string());
    stats.max_rss_bytes = Some(1_073_741_824); // 1 GB
    stats.max_vm_size_bytes = Some(2_147_483_648); // 2 GB
    stats.max_disk_read_bytes = Some(104_857_600); // 100 MB
    stats.max_disk_write_bytes = Some(52_428_800); // 50 MB
    stats.ave_cpu_seconds = Some(42.5);
    stats.node_list = Some("node001".to_string());
    apis::slurm_stats_api::create_slurm_stats(config, stats).expect("Failed to create slurm_stats")
}

#[rstest]
fn test_create_slurm_stats(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_create_slurm_stats");
    let workflow_id = workflow.id.unwrap();
    let job = create_test_job(config, workflow_id, "job1");
    let job_id = job.id.unwrap();

    let stats = create_test_slurm_stats(config, workflow_id, job_id);

    assert!(stats.id.is_some());
    assert_eq!(stats.workflow_id, workflow_id);
    assert_eq!(stats.job_id, job_id);
    assert_eq!(stats.run_id, 1);
    assert_eq!(stats.attempt_id, 1);
    assert_eq!(stats.slurm_job_id.as_deref(), Some("12345678"));
    assert_eq!(stats.max_rss_bytes, Some(1_073_741_824));
    assert_eq!(stats.max_vm_size_bytes, Some(2_147_483_648));
    assert_eq!(stats.max_disk_read_bytes, Some(104_857_600));
    assert_eq!(stats.max_disk_write_bytes, Some(52_428_800));
    assert_eq!(stats.ave_cpu_seconds, Some(42.5));
    assert_eq!(stats.node_list.as_deref(), Some("node001"));
}

#[rstest]
fn test_list_slurm_stats_by_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_list_slurm_stats_workflow");
    let workflow_id = workflow.id.unwrap();
    let job1 = create_test_job(config, workflow_id, "job1");
    let job2 = create_test_job(config, workflow_id, "job2");

    create_test_slurm_stats(config, workflow_id, job1.id.unwrap());
    create_test_slurm_stats(config, workflow_id, job2.id.unwrap());

    let response =
        apis::slurm_stats_api::list_slurm_stats(config, workflow_id, None, None, None, None, None)
            .expect("Failed to list slurm_stats");

    assert_eq!(response.total_count, 2);
    let items = response.items;
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|s| s.workflow_id == workflow_id));
}

#[rstest]
fn test_list_slurm_stats_filter_by_job(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_list_slurm_stats_job_filter");
    let workflow_id = workflow.id.unwrap();
    let job1 = create_test_job(config, workflow_id, "job1");
    let job2 = create_test_job(config, workflow_id, "job2");
    let job1_id = job1.id.unwrap();

    create_test_slurm_stats(config, workflow_id, job1_id);
    create_test_slurm_stats(config, workflow_id, job2.id.unwrap());

    let response = apis::slurm_stats_api::list_slurm_stats(
        config,
        workflow_id,
        Some(job1_id),
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list slurm_stats filtered by job");

    assert_eq!(response.total_count, 1);
    let items = response.items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].job_id, job1_id);
}

#[rstest]
fn test_list_slurm_stats_empty_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_list_slurm_stats_empty");
    let workflow_id = workflow.id.unwrap();

    let response =
        apis::slurm_stats_api::list_slurm_stats(config, workflow_id, None, None, None, None, None)
            .expect("Failed to list slurm_stats for empty workflow");

    assert_eq!(response.total_count, 0);
    assert!(response.items.is_empty());
}

#[rstest]
fn test_list_slurm_stats_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_list_slurm_stats_pagination");
    let workflow_id = workflow.id.unwrap();

    // Create 5 stats records across 5 jobs
    for i in 0..5 {
        let job = create_test_job(config, workflow_id, &format!("job{}", i));
        create_test_slurm_stats(config, workflow_id, job.id.unwrap());
    }

    // Fetch first page of 2
    let page1 = apis::slurm_stats_api::list_slurm_stats(
        config,
        workflow_id,
        None,
        None,
        None,
        Some(0),
        Some(2),
    )
    .expect("Failed to list page 1");
    assert_eq!(page1.total_count, 5);
    assert_eq!(page1.items.len(), 2);

    // Fetch second page
    let page2 = apis::slurm_stats_api::list_slurm_stats(
        config,
        workflow_id,
        None,
        None,
        None,
        Some(2),
        Some(2),
    )
    .expect("Failed to list page 2");
    assert_eq!(page2.total_count, 5);
    assert_eq!(page2.items.len(), 2);
}

#[rstest]
fn test_slurm_stats_null_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_slurm_stats_null_fields");
    let workflow_id = workflow.id.unwrap();
    let job = create_test_job(config, workflow_id, "job1");

    // Create stats with all optional fields null (e.g. sacct data unavailable)
    let minimal = models::SlurmStatsModel::new(workflow_id, job.id.unwrap(), 1, 1);
    let created = apis::slurm_stats_api::create_slurm_stats(config, minimal)
        .expect("Failed to create minimal stats");

    assert!(created.id.is_some());
    assert!(created.slurm_job_id.is_none());
    assert!(created.max_rss_bytes.is_none());
    assert!(created.max_vm_size_bytes.is_none());
    assert!(created.max_disk_read_bytes.is_none());
    assert!(created.max_disk_write_bytes.is_none());
    assert!(created.ave_cpu_seconds.is_none());
    assert!(created.node_list.is_none());
}
