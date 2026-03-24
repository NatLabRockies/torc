//! Permanent HTTP transport for the live Torc server.

mod compute_nodes;
mod path_parsing;
mod query_parsing;
mod request_parsing;
mod response_mapping;
mod workflows;

use crate::models;
use crate::server::api_contract::TransportApiCore;
use crate::server::http_server::Server;
use crate::server::response_types::{
    access::*, artifacts::*, events::*, jobs::*, scheduling::*, system::*, workflows::*,
};
use crate::server::transport_types::auth_types::Authorization;
use crate::server::transport_types::context_types::{Has, XSpanIdString};
use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, HeaderValue};
use axum::http::{Request, Response, StatusCode};
use http_body::Body as HttpBody;
use std::collections::HashMap;
use url::form_urlencoded;

use self::query_parsing::*;
pub(crate) use self::response_mapping::*;
pub(crate) use self::workflows::*;

#[cfg(test)]
mod http_transport_tests {
    use super::path_parsing::{
        parse_access_check_path, parse_access_group_members_collection_path,
        parse_group_member_path, parse_user_groups_path, parse_workflow_access_group_item_path,
        parse_workflow_access_groups_collection_path, parse_workflow_events_stream_path,
        parse_workflow_failure_handlers_path,
    };
    use super::*;

    #[test]
    fn parses_workflow_events_stream_path_and_level() {
        assert_eq!(
            parse_workflow_events_stream_path("/torc-service/v1/workflows/7/events/stream"),
            Some(7)
        );
        assert_eq!(
            parse_event_stream_level(Some("level=warning")),
            models::EventSeverity::Warning
        );
        assert_eq!(
            parse_event_stream_level(Some("level=invalid")),
            models::EventSeverity::Info
        );
    }

    #[test]
    fn parses_compute_nodes_query() {
        let parsed = parse_compute_nodes_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=hostname&reverse_sort=true&hostname=node01&is_active=false&scheduled_compute_node_id=9",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ComputeNodesQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("hostname".to_string()),
                reverse_sort: Some(true),
                hostname: Some("node01".to_string()),
                is_active: Some(false),
                scheduled_compute_node_id: Some(9),
            }
        );
    }

    #[test]
    fn rejects_missing_workflow_id() {
        let err = parse_compute_nodes_query(Some("limit=2")).expect_err("missing workflow id");
        assert!(err.contains("workflow_id"));
    }

    #[test]
    fn parses_events_query() {
        let parsed = parse_events_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=timestamp&reverse_sort=false&category=system&after_timestamp=42",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            EventsQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("timestamp".to_string()),
                reverse_sort: Some(false),
                category: Some("system".to_string()),
                after_timestamp: Some(42),
            }
        );
    }

    #[test]
    fn parses_files_query() {
        let parsed = parse_files_query(Some(
            "workflow_id=7&produced_by_job_id=3&offset=1&limit=2&sort_by=name&reverse_sort=true&name=out.txt&path=%2Ftmp&is_output=false",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            FilesQuery {
                workflow_id: 7,
                produced_by_job_id: Some(3),
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(true),
                name: Some("out.txt".to_string()),
                path: Some("/tmp".to_string()),
                is_output: Some(false),
            }
        );
    }

    #[test]
    fn parses_local_schedulers_query() {
        let parsed = parse_local_schedulers_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=memory&reverse_sort=false&memory=4g&num_cpus=8",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            LocalSchedulersQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("memory".to_string()),
                reverse_sort: Some(false),
                memory: Some("4g".to_string()),
                num_cpus: Some(8),
            }
        );
    }

    #[test]
    fn parses_results_query() {
        let parsed = parse_results_query(Some(
            "workflow_id=7&job_id=3&run_id=5&return_code=0&status=completed&compute_node_id=9&offset=1&limit=2&sort_by=run_id&reverse_sort=true&all_runs=false",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ResultsQuery {
                workflow_id: 7,
                job_id: Some(3),
                run_id: Some(5),
                return_code: Some(0),
                status: Some(models::JobStatus::Completed),
                compute_node_id: Some(9),
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("run_id".to_string()),
                reverse_sort: Some(true),
                all_runs: Some(false),
            }
        );
    }

    #[test]
    fn parses_user_data_query() {
        let parsed = parse_user_data_query(Some(
            "workflow_id=7&consumer_job_id=3&producer_job_id=5&offset=1&limit=2&sort_by=name&reverse_sort=false&name=blob&is_ephemeral=true",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            UserDataQuery {
                workflow_id: 7,
                consumer_job_id: Some(3),
                producer_job_id: Some(5),
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(false),
                name: Some("blob".to_string()),
                is_ephemeral: Some(true),
            }
        );
    }

    #[test]
    fn parses_scheduled_compute_nodes_query() {
        let parsed = parse_scheduled_compute_nodes_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=status&reverse_sort=true&scheduler_id=sched-1&scheduler_config_id=config-2&status=running",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ScheduledComputeNodesQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("status".to_string()),
                reverse_sort: Some(true),
                scheduler_id: Some("sched-1".to_string()),
                scheduler_config_id: Some("config-2".to_string()),
                status: Some("running".to_string()),
            }
        );
    }

    #[test]
    fn parses_slurm_schedulers_query() {
        let parsed = parse_slurm_schedulers_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=name&reverse_sort=false",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            SlurmSchedulersQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(false),
            }
        );
    }

    #[test]
    fn parses_access_pagination_query() {
        let parsed = parse_access_pagination_query(Some("offset=3&limit=25")).expect("valid query");

        assert_eq!(
            parsed,
            AccessPaginationQuery {
                offset: Some(3),
                limit: Some(25),
            }
        );
    }

    #[test]
    fn parses_resource_requirements_query() {
        let parsed = parse_resource_requirements_query(Some(
            "workflow_id=7&job_id=3&name=default&memory=16g&num_cpus=4&num_gpus=1&num_nodes=2&runtime=3600&offset=1&limit=10&sort_by=name&reverse_sort=true",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ResourceRequirementsQuery {
                workflow_id: 7,
                job_id: Some(3),
                name: Some("default".to_string()),
                memory: Some("16g".to_string()),
                num_cpus: Some(4),
                num_gpus: Some(1),
                num_nodes: Some(2),
                runtime: Some(3600),
                offset: Some(1),
                limit: Some(10),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(true),
            }
        );
    }

    #[test]
    fn parses_slurm_stats_query() {
        let parsed = parse_slurm_stats_query(Some(
            "workflow_id=7&job_id=3&run_id=4&attempt_id=5&offset=1&limit=10",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            SlurmStatsQuery {
                workflow_id: 7,
                job_id: Some(3),
                run_id: Some(4),
                attempt_id: Some(5),
                offset: Some(1),
                limit: Some(10),
            }
        );
    }

    #[test]
    fn parses_access_control_paths() {
        assert_eq!(
            parse_access_group_members_collection_path("/torc-service/v1/access_groups/12/members"),
            Some(12)
        );
        assert_eq!(
            parse_group_member_path("/torc-service/v1/access_groups/12/members/alice"),
            Some((12, "alice".to_string()))
        );
        assert_eq!(
            parse_user_groups_path("/torc-service/v1/users/alice/groups"),
            Some("alice".to_string())
        );
        assert_eq!(
            parse_workflow_access_groups_collection_path(
                "/torc-service/v1/workflows/7/access_groups",
            ),
            Some(7)
        );
        assert_eq!(
            parse_workflow_access_group_item_path("/torc-service/v1/workflows/7/access_groups/8",),
            Some((7, 8))
        );
        assert_eq!(
            parse_access_check_path("/torc-service/v1/access_check/7/alice"),
            Some((7, "alice".to_string()))
        );
        assert_eq!(
            parse_workflow_failure_handlers_path("/torc-service/v1/workflows/7/failure_handlers"),
            Some(7)
        );
    }
}
