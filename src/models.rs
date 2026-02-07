#![allow(unused_qualifications)]

use chrono::Utc;

#[cfg(feature = "server")]
use crate::server::header;
use serde_json;

// Self-reference for models module
use crate::models;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    Default,
)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Debug,
    #[default]
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for EventSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSeverity::Debug => write!(f, "debug"),
            EventSeverity::Info => write!(f, "info"),
            EventSeverity::Warning => write!(f, "warning"),
            EventSeverity::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for EventSeverity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(EventSeverity::Debug),
            "info" => Ok(EventSeverity::Info),
            "warning" => Ok(EventSeverity::Warning),
            "error" => Ok(EventSeverity::Error),
            _ => Err(format!("Invalid severity level: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct CreateJobsResponse {
    #[serde(rename = "jobs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<models::JobModel>>,
}

impl CreateJobsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new() -> CreateJobsResponse {
        CreateJobsResponse { jobs: None }
    }
}

/// Converts the CreateJobsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for CreateJobsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type jobs in query parameter serialization
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a CreateJobsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for CreateJobsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub jobs: Vec<Vec<models::JobModel>>,
        }

        let intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "jobs" => {
                        return std::result::Result::Err(
                            "Parsing a container in this style is not supported in CreateJobsResponse"
                                .to_string(),
                        );
                    }
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing CreateJobsResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(CreateJobsResponse {
            jobs: intermediate_rep.jobs.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<CreateJobsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<CreateJobsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<CreateJobsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for CreateJobsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<CreateJobsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <CreateJobsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into CreateJobsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<CreateJobsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<CreateJobsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<CreateJobsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<CreateJobsResponse> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <CreateJobsResponse as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into CreateJobsResponse - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ComputeNodeModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    #[serde(rename = "hostname")]
    pub hostname: String,

    #[serde(rename = "pid")]
    pub pid: i64,

    #[serde(rename = "start_time")]
    pub start_time: String,

    #[serde(rename = "duration_seconds")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,

    #[serde(rename = "is_active")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,

    #[serde(rename = "num_cpus")]
    pub num_cpus: i64,

    #[serde(rename = "memory_gb")]
    pub memory_gb: f64,

    #[serde(rename = "num_gpus")]
    pub num_gpus: i64,

    #[serde(rename = "num_nodes")]
    pub num_nodes: i64,

    #[serde(rename = "time_limit")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit: Option<String>,

    #[serde(rename = "scheduler_config_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_config_id: Option<i64>,

    #[serde(rename = "compute_node_type")]
    pub compute_node_type: String,

    #[serde(rename = "scheduler")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<serde_json::Value>,
}

impl ComputeNodeModel {
    #[allow(clippy::new_without_default)]
    pub fn new(
        workflow_id: i64,
        hostname: String,
        pid: i64,
        start_time: String,
        num_cpus: i64,
        memory_gb: f64,
        num_gpus: i64,
        num_nodes: i64,
        compute_node_type: String,
        scheduler: Option<serde_json::Value>,
    ) -> ComputeNodeModel {
        ComputeNodeModel {
            id: None,
            workflow_id,
            hostname,
            pid,
            start_time,
            duration_seconds: None,
            is_active: None,
            num_cpus,
            memory_gb,
            num_gpus,
            num_nodes,
            time_limit: None,
            scheduler_config_id: None,
            compute_node_type,
            scheduler,
        }
    }
}

/// Converts the ComputeNodeModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ComputeNodeModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some(self.workflow_id.to_string()),
            Some("hostname".to_string()),
            Some(self.hostname.to_string()),
            Some("pid".to_string()),
            Some(self.pid.to_string()),
            Some("start_time".to_string()),
            Some(self.start_time.to_string()),
            self.duration_seconds.as_ref().map(|duration_seconds| {
                ["duration_seconds".to_string(), duration_seconds.to_string()].join(",")
            }),
            self.is_active
                .as_ref()
                .map(|is_active| ["is_active".to_string(), is_active.to_string()].join(",")),
            Some("num_cpus".to_string()),
            Some(self.num_cpus.to_string()),
            Some("memory_gb".to_string()),
            Some(self.memory_gb.to_string()),
            Some("num_gpus".to_string()),
            Some(self.num_gpus.to_string()),
            Some("num_nodes".to_string()),
            Some(self.num_nodes.to_string()),
            self.time_limit
                .as_ref()
                .map(|time_limit| ["time_limit".to_string(), time_limit.to_string()].join(",")),
            self.scheduler_config_id
                .as_ref()
                .map(|scheduler_config_id| {
                    [
                        "scheduler_config_id".to_string(),
                        scheduler_config_id.to_string(),
                    ]
                    .join(",")
                }),
            // Skipping non-primitive type scheduler in query parameter serialization
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ComputeNodeModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ComputeNodeModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub hostname: Vec<String>,
            pub pid: Vec<i64>,
            pub start_time: Vec<String>,
            pub duration_seconds: Vec<f64>,
            pub is_active: Vec<bool>,
            pub num_cpus: Vec<i64>,
            pub memory_gb: Vec<f64>,
            pub num_gpus: Vec<i64>,
            pub num_nodes: Vec<i64>,
            pub time_limit: Vec<String>,
            pub scheduler_config_id: Vec<i64>,
            pub compute_node_type: Vec<String>,
            pub scheduler: Vec<serde_json::Value>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ComputeNodeModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "hostname" => intermediate_rep.hostname.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "pid" => intermediate_rep.pid.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "start_time" => intermediate_rep.start_time.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "duration_seconds" => intermediate_rep.duration_seconds.push(
                        <f64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "is_active" => intermediate_rep.is_active.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_cpus" => intermediate_rep.num_cpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "memory_gb" => intermediate_rep.memory_gb.push(
                        <f64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_gpus" => intermediate_rep.num_gpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_nodes" => intermediate_rep.num_nodes.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "time_limit" => intermediate_rep.time_limit.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler_config_id" => intermediate_rep.scheduler_config_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "compute_node_type" => intermediate_rep.compute_node_type.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler" => intermediate_rep.scheduler.push(
                        <serde_json::Value as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ComputeNodeModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ComputeNodeModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in ComputeNodeModel".to_string())?,
            hostname: intermediate_rep
                .hostname
                .into_iter()
                .next()
                .ok_or_else(|| "hostname missing in ComputeNodeModel".to_string())?,
            pid: intermediate_rep
                .pid
                .into_iter()
                .next()
                .ok_or_else(|| "pid missing in ComputeNodeModel".to_string())?,
            start_time: intermediate_rep
                .start_time
                .into_iter()
                .next()
                .ok_or_else(|| "start_time missing in ComputeNodeModel".to_string())?,
            duration_seconds: intermediate_rep.duration_seconds.into_iter().next(),
            is_active: intermediate_rep.is_active.into_iter().next(),
            num_cpus: intermediate_rep
                .num_cpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_cpus missing in ComputeNodeModel".to_string())?,
            memory_gb: intermediate_rep
                .memory_gb
                .into_iter()
                .next()
                .ok_or_else(|| "memory_gb missing in ComputeNodeModel".to_string())?,
            num_gpus: intermediate_rep
                .num_gpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_gpus missing in ComputeNodeModel".to_string())?,
            num_nodes: intermediate_rep
                .num_nodes
                .into_iter()
                .next()
                .ok_or_else(|| "num_nodes missing in ComputeNodeModel".to_string())?,
            time_limit: intermediate_rep.time_limit.into_iter().next(),
            scheduler_config_id: intermediate_rep.scheduler_config_id.into_iter().next(),
            compute_node_type: intermediate_rep
                .compute_node_type
                .into_iter()
                .next()
                .ok_or_else(|| "compute_node_type missing in ComputeNodeModel".to_string())?,
            scheduler: intermediate_rep.scheduler.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ComputeNodeModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ComputeNodeModel>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ComputeNodeModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ComputeNodeModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ComputeNodeModel>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ComputeNodeModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ComputeNodeModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ComputeNodeModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ComputeNodeModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ComputeNodeModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ComputeNodeModel> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ComputeNodeModel as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ComputeNodeModel - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ComputeNodeSchedule {
    #[serde(rename = "max_parallel_jobs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_parallel_jobs: Option<i64>,

    #[serde(rename = "num_jobs")]
    pub num_jobs: i64,

    #[serde(rename = "scheduler_id")]
    pub scheduler_id: i64,

    #[serde(rename = "start_one_worker_per_node")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_one_worker_per_node: Option<bool>,
}

impl ComputeNodeSchedule {
    #[allow(clippy::new_without_default)]
    pub fn new(num_jobs: i64, scheduler_id: i64) -> ComputeNodeSchedule {
        ComputeNodeSchedule {
            max_parallel_jobs: None,
            num_jobs,
            scheduler_id,
            start_one_worker_per_node: Some(false),
        }
    }
}

/// Converts the ComputeNodeSchedule value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ComputeNodeSchedule {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.max_parallel_jobs.as_ref().map(|max_parallel_jobs| {
                [
                    "max_parallel_jobs".to_string(),
                    max_parallel_jobs.to_string(),
                ]
                .join(",")
            }),
            Some("num_jobs".to_string()),
            Some(self.num_jobs.to_string()),
            Some("scheduler_id".to_string()),
            Some(self.scheduler_id.to_string()),
            self.start_one_worker_per_node
                .as_ref()
                .map(|start_one_worker_per_node| {
                    [
                        "start_one_worker_per_node".to_string(),
                        start_one_worker_per_node.to_string(),
                    ]
                    .join(",")
                }),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ComputeNodeSchedule value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ComputeNodeSchedule {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub max_parallel_jobs: Vec<i64>,
            pub num_jobs: Vec<i64>,
            pub scheduler_id: Vec<i64>,
            pub start_one_worker_per_node: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ComputeNodeSchedule".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "max_parallel_jobs" => intermediate_rep.max_parallel_jobs.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_jobs" => intermediate_rep.num_jobs.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler_id" => intermediate_rep.scheduler_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "start_one_worker_per_node" => intermediate_rep.start_one_worker_per_node.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ComputeNodeSchedule".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ComputeNodeSchedule {
            max_parallel_jobs: intermediate_rep.max_parallel_jobs.into_iter().next(),
            num_jobs: intermediate_rep
                .num_jobs
                .into_iter()
                .next()
                .ok_or_else(|| "num_jobs missing in ComputeNodeSchedule".to_string())?,
            scheduler_id: intermediate_rep
                .scheduler_id
                .into_iter()
                .next()
                .ok_or_else(|| "scheduler_id missing in ComputeNodeSchedule".to_string())?,
            start_one_worker_per_node: intermediate_rep
                .start_one_worker_per_node
                .into_iter()
                .next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ComputeNodeSchedule> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ComputeNodeSchedule>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ComputeNodeSchedule>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ComputeNodeSchedule - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ComputeNodeSchedule>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ComputeNodeSchedule as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ComputeNodeSchedule - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ComputeNodeSchedule>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ComputeNodeSchedule>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ComputeNodeSchedule>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ComputeNodeSchedule> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ComputeNodeSchedule as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ComputeNodeSchedule - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ComputeNodesResources {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    #[serde(rename = "num_cpus")]
    pub num_cpus: i64,

    #[serde(rename = "memory_gb")]
    pub memory_gb: f64,

    #[serde(rename = "num_gpus")]
    pub num_gpus: i64,

    #[serde(rename = "num_nodes")]
    pub num_nodes: i64,

    #[serde(rename = "time_limit")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit: Option<String>,

    #[serde(rename = "scheduler_config_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_config_id: Option<i64>,
}

impl ComputeNodesResources {
    #[allow(clippy::new_without_default)]
    pub fn new(
        num_cpus: i64,
        memory_gb: f64,
        num_gpus: i64,
        num_nodes: i64,
    ) -> ComputeNodesResources {
        ComputeNodesResources {
            id: None,
            num_cpus,
            memory_gb,
            num_gpus,
            num_nodes,
            time_limit: None,
            scheduler_config_id: None,
        }
    }
}

/// Converts the ComputeNodesResources value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ComputeNodesResources {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("num_cpus".to_string()),
            Some(self.num_cpus.to_string()),
            Some("memory_gb".to_string()),
            Some(self.memory_gb.to_string()),
            Some("num_gpus".to_string()),
            Some(self.num_gpus.to_string()),
            Some("num_nodes".to_string()),
            Some(self.num_nodes.to_string()),
            self.time_limit
                .as_ref()
                .map(|time_limit| ["time_limit".to_string(), time_limit.to_string()].join(",")),
            self.scheduler_config_id
                .as_ref()
                .map(|scheduler_config_id| {
                    [
                        "scheduler_config_id".to_string(),
                        scheduler_config_id.to_string(),
                    ]
                    .join(",")
                }),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ComputeNodesResources value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ComputeNodesResources {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub num_cpus: Vec<i64>,
            pub memory_gb: Vec<f64>,
            pub num_gpus: Vec<i64>,
            pub num_nodes: Vec<i64>,
            pub time_limit: Vec<String>,
            pub scheduler_config_id: Vec<i64>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ComputeNodesResources".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_cpus" => intermediate_rep.num_cpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "memory_gb" => intermediate_rep.memory_gb.push(
                        <f64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_gpus" => intermediate_rep.num_gpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_nodes" => intermediate_rep.num_nodes.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "time_limit" => intermediate_rep.time_limit.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler_config_id" => intermediate_rep.scheduler_config_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ComputeNodesResources".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ComputeNodesResources {
            id: intermediate_rep.id.into_iter().next(),
            num_cpus: intermediate_rep
                .num_cpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_cpus missing in ComputeNodesResources".to_string())?,
            memory_gb: intermediate_rep
                .memory_gb
                .into_iter()
                .next()
                .ok_or_else(|| "memory_gb missing in ComputeNodesResources".to_string())?,
            num_gpus: intermediate_rep
                .num_gpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_gpus missing in ComputeNodesResources".to_string())?,
            num_nodes: intermediate_rep
                .num_nodes
                .into_iter()
                .next()
                .ok_or_else(|| "num_nodes missing in ComputeNodesResources".to_string())?,
            time_limit: intermediate_rep.time_limit.into_iter().next(),
            scheduler_config_id: intermediate_rep.scheduler_config_id.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ComputeNodesResources> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ComputeNodesResources>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ComputeNodesResources>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ComputeNodesResources - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ComputeNodesResources>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ComputeNodesResources as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ComputeNodesResources - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ComputeNodesResources>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ComputeNodesResources>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ComputeNodesResources>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ComputeNodesResources> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ComputeNodesResources as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ComputeNodesResources - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ErrorResponse {
    #[serde(rename = "error")]
    pub error: serde_json::Value,

    #[serde(rename = "errorNum")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_num: Option<i64>,

    #[serde(rename = "errorMessage")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    #[serde(rename = "code")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i64>,
}

impl ErrorResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(error: serde_json::Value) -> ErrorResponse {
        ErrorResponse {
            error,
            error_num: None,
            error_message: None,
            code: None,
        }
    }
}

/// Converts the DefaultErrorResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ErrorResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type error in query parameter serialization
            self.error_num
                .as_ref()
                .map(|error_num| ["errorNum".to_string(), error_num.to_string()].join(",")),
            self.error_message.as_ref().map(|error_message| {
                ["errorMessage".to_string(), error_message.to_string()].join(",")
            }),
            self.code
                .as_ref()
                .map(|code| ["code".to_string(), code.to_string()].join(",")),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a DefaultErrorResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ErrorResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub error: Vec<serde_json::Value>,
            pub error_num: Vec<i64>,
            pub error_message: Vec<String>,
            pub code: Vec<i64>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing DefaultErrorResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "error" => intermediate_rep.error.push(
                        <serde_json::Value as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    "errorNum" => intermediate_rep.error_num.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "errorMessage" => intermediate_rep.error_message.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "code" => intermediate_rep.code.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing DefaultErrorResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ErrorResponse {
            error: intermediate_rep
                .error
                .into_iter()
                .next()
                .ok_or_else(|| "error missing in DefaultErrorResponse".to_string())?,
            error_num: intermediate_rep.error_num.into_iter().next(),
            error_message: intermediate_rep.error_message.into_iter().next(),
            code: intermediate_rep.code.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<DefaultErrorResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ErrorResponse>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ErrorResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for DefaultErrorResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<ErrorResponse> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ErrorResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into DefaultErrorResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ErrorResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ErrorResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ErrorResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ErrorResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ErrorResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into DefaultErrorResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

/// Data model for events.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct EventModel {
    /// Database ID of the event.
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Database ID of the workflow this record is associated with.
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// Timestamp of the event in milliseconds since epoch (UTC)
    #[serde(rename = "timestamp")]
    pub timestamp: i64,

    /// User-defined data associated with the event
    #[serde(rename = "data")]
    pub data: serde_json::Value,
}

impl EventModel {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64, data: serde_json::Value) -> EventModel {
        EventModel {
            id: None,
            workflow_id,
            timestamp: Utc::now().timestamp_millis(),
            data,
        }
    }

    /// Format the timestamp as a human-readable ISO 8601 string
    pub fn timestamp_as_string(&self) -> String {
        use chrono::{DateTime, Utc};
        DateTime::from_timestamp_millis(self.timestamp)
            .map(|dt: DateTime<Utc>| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            .unwrap_or_else(|| format!("{}ms", self.timestamp))
    }
}

/// Converts the EventModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for EventModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            Some("timestamp".to_string()),
            Some(self.timestamp.to_string()),
            // Skipping non-primitive type data in query parameter serialization
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a EventModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for EventModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub timestamp: Vec<i64>,
            pub data: Vec<serde_json::Value>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing EventModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "timestamp" => intermediate_rep.timestamp.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "data" => intermediate_rep.data.push(
                        <serde_json::Value as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing EventModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(EventModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in EventModel".to_string())?,
            timestamp: intermediate_rep
                .timestamp
                .into_iter()
                .next()
                .ok_or_else(|| "timestamp missing in EventModel".to_string())?,
            data: intermediate_rep
                .data
                .into_iter()
                .next()
                .ok_or_else(|| "data missing in EventModel".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<EventModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<EventModel>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<EventModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for EventModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<EventModel> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <EventModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into EventModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<EventModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<EventModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<EventModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<EventModel> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <EventModel as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into EventModel - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

/// Data model for files needed or produced by jobs. Can be data or code.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct FileModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Database ID of the workflow this record is associated with.
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// User-defined name of the file (not necessarily the filename)
    #[serde(rename = "name")]
    pub name: String,

    /// Path to the file; can be relative to the execution directory.
    #[serde(rename = "path")]
    pub path: String,

    /// Timestamp of when the file was last modified
    #[serde(rename = "st_mtime")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st_mtime: Option<f64>,
}

impl FileModel {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64, name: String, path: String) -> FileModel {
        FileModel {
            id: None,
            workflow_id,
            name,
            path,
            st_mtime: None,
        }
    }
}

/// Converts the FileModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for FileModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            Some("name".to_string()),
            Some(self.name.to_string()),
            Some("path".to_string()),
            Some(self.path.to_string()),
            self.st_mtime
                .as_ref()
                .map(|st_mtime| ["st_mtime".to_string(), st_mtime.to_string()].join(",")),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a FileModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for FileModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub name: Vec<String>,
            pub path: Vec<String>,
            pub st_mtime: Vec<f64>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing FileModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "name" => intermediate_rep.name.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "path" => intermediate_rep.path.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "st_mtime" => intermediate_rep.st_mtime.push(
                        <f64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing FileModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(FileModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in FileModel".to_string())?,
            name: intermediate_rep
                .name
                .into_iter()
                .next()
                .ok_or_else(|| "name missing in FileModel".to_string())?,
            path: intermediate_rep
                .path
                .into_iter()
                .next()
                .ok_or_else(|| "path missing in FileModel".to_string())?,
            st_mtime: intermediate_rep.st_mtime.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<FileModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<FileModel>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<FileModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for FileModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<FileModel> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <FileModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into FileModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<FileModel>>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<FileModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<Vec<FileModel>> {
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<FileModel> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <FileModel as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into FileModel - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct FailureHandlerModel {
    /// Database ID of this record.
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Database ID of the workflow this record is associated with.
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// Name of the failure handler
    #[serde(rename = "name")]
    pub name: String,

    /// JSON array of rules specifying exit codes, recovery scripts, and max retries
    #[serde(rename = "rules")]
    pub rules: String,
}

impl FailureHandlerModel {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64, name: String, rules: String) -> FailureHandlerModel {
        FailureHandlerModel {
            id: None,
            workflow_id,
            name,
            rules,
        }
    }
}

/// Converts the FailureHandlerModel value to the Query Parameters representation (style=form, explode=false)
impl std::string::ToString for FailureHandlerModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            Some("name".to_string()),
            Some(self.name.to_string()),
            Some("rules".to_string()),
            Some(self.rules.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListFailureHandlersResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<FailureHandlerModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListFailureHandlersResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListFailureHandlersResponse {
        ListFailureHandlersResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct GetDotGraphResponse {
    #[serde(rename = "graph")]
    pub graph: String,
}

impl GetDotGraphResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(graph: String) -> GetDotGraphResponse {
        GetDotGraphResponse { graph }
    }
}

/// Converts the GetDotGraphResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for GetDotGraphResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> =
            vec![Some("graph".to_string()), Some(self.graph.to_string())];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a GetDotGraphResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for GetDotGraphResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub graph: Vec<String>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing GetDotGraphResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "graph" => intermediate_rep.graph.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing GetDotGraphResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(GetDotGraphResponse {
            graph: intermediate_rep
                .graph
                .into_iter()
                .next()
                .ok_or_else(|| "graph missing in GetDotGraphResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<GetDotGraphResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<GetDotGraphResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<GetDotGraphResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for GetDotGraphResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<GetDotGraphResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <GetDotGraphResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into GetDotGraphResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<GetDotGraphResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<GetDotGraphResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<GetDotGraphResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<GetDotGraphResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <GetDotGraphResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into GetDotGraphResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct GetReadyJobRequirementsResponse {
    #[serde(rename = "num_jobs")]
    pub num_jobs: i64,

    #[serde(rename = "num_cpus")]
    pub num_cpus: i64,

    #[serde(rename = "num_gpus")]
    pub num_gpus: i64,

    #[serde(rename = "memory_gb")]
    pub memory_gb: f64,

    #[serde(rename = "max_num_nodes")]
    pub max_num_nodes: i64,

    #[serde(rename = "max_runtime")]
    pub max_runtime: String,
}

impl GetReadyJobRequirementsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        num_jobs: i64,
        num_cpus: i64,
        num_gpus: i64,
        memory_gb: f64,
        max_num_nodes: i64,
        max_runtime: String,
    ) -> GetReadyJobRequirementsResponse {
        GetReadyJobRequirementsResponse {
            num_jobs,
            num_cpus,
            num_gpus,
            memory_gb,
            max_num_nodes,
            max_runtime,
        }
    }
}

/// Converts the GetReadyJobRequirementsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for GetReadyJobRequirementsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            Some("num_jobs".to_string()),
            Some(self.num_jobs.to_string()),
            Some("num_cpus".to_string()),
            Some(self.num_cpus.to_string()),
            Some("num_gpus".to_string()),
            Some(self.num_gpus.to_string()),
            Some("memory_gb".to_string()),
            Some(self.memory_gb.to_string()),
            Some("max_num_nodes".to_string()),
            Some(self.max_num_nodes.to_string()),
            Some("max_runtime".to_string()),
            Some(self.max_runtime.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a GetReadyJobRequirementsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for GetReadyJobRequirementsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub num_jobs: Vec<i64>,
            pub num_cpus: Vec<i64>,
            pub num_gpus: Vec<i64>,
            pub memory_gb: Vec<f64>,
            pub max_num_nodes: Vec<i64>,
            pub max_runtime: Vec<String>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing GetReadyJobRequirementsResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "num_jobs" => intermediate_rep.num_jobs.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_cpus" => intermediate_rep.num_cpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_gpus" => intermediate_rep.num_gpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "memory_gb" => intermediate_rep.memory_gb.push(
                        <f64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "max_num_nodes" => intermediate_rep.max_num_nodes.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "max_runtime" => intermediate_rep.max_runtime.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing GetReadyJobRequirementsResponse"
                                .to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(GetReadyJobRequirementsResponse {
            num_jobs: intermediate_rep
                .num_jobs
                .into_iter()
                .next()
                .ok_or_else(|| "num_jobs missing in GetReadyJobRequirementsResponse".to_string())?,
            num_cpus: intermediate_rep
                .num_cpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_cpus missing in GetReadyJobRequirementsResponse".to_string())?,
            num_gpus: intermediate_rep
                .num_gpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_gpus missing in GetReadyJobRequirementsResponse".to_string())?,
            memory_gb: intermediate_rep
                .memory_gb
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "memory_gb missing in GetReadyJobRequirementsResponse".to_string()
                })?,
            max_num_nodes: intermediate_rep
                .max_num_nodes
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "max_num_nodes missing in GetReadyJobRequirementsResponse".to_string()
                })?,
            max_runtime: intermediate_rep
                .max_runtime
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "max_runtime missing in GetReadyJobRequirementsResponse".to_string()
                })?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<GetReadyJobRequirementsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<GetReadyJobRequirementsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<GetReadyJobRequirementsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for GetReadyJobRequirementsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<GetReadyJobRequirementsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <GetReadyJobRequirementsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into GetReadyJobRequirementsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<GetReadyJobRequirementsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<GetReadyJobRequirementsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<GetReadyJobRequirementsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<GetReadyJobRequirementsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <GetReadyJobRequirementsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into GetReadyJobRequirementsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct IsCompleteResponse {
    #[serde(rename = "is_canceled")]
    pub is_canceled: bool,

    #[serde(rename = "is_complete")]
    pub is_complete: bool,

    #[serde(rename = "needs_to_run_completion_script")]
    pub needs_to_run_completion_script: bool,
}

impl IsCompleteResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        is_canceled: bool,
        is_complete: bool,
        needs_to_run_completion_script: bool,
    ) -> IsCompleteResponse {
        IsCompleteResponse {
            is_canceled,
            is_complete,
            needs_to_run_completion_script,
        }
    }
}

/// Converts the IsCompleteResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for IsCompleteResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            Some("is_canceled".to_string()),
            Some(self.is_canceled.to_string()),
            Some("is_complete".to_string()),
            Some(self.is_complete.to_string()),
            Some("needs_to_run_completion_script".to_string()),
            Some(self.needs_to_run_completion_script.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a IsCompleteResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for IsCompleteResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub is_canceled: Vec<bool>,
            pub is_complete: Vec<bool>,
            pub needs_to_run_completion_script: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing IsCompleteResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "is_canceled" => intermediate_rep.is_canceled.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "is_complete" => intermediate_rep.is_complete.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "needs_to_run_completion_script" => {
                        intermediate_rep.needs_to_run_completion_script.push(
                            <bool as std::str::FromStr>::from_str(val)
                                .map_err(|x| x.to_string())?,
                        )
                    }
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing IsCompleteResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(IsCompleteResponse {
            is_canceled: intermediate_rep
                .is_canceled
                .into_iter()
                .next()
                .ok_or_else(|| "is_canceled missing in IsCompleteResponse".to_string())?,
            is_complete: intermediate_rep
                .is_complete
                .into_iter()
                .next()
                .ok_or_else(|| "is_complete missing in IsCompleteResponse".to_string())?,
            needs_to_run_completion_script: intermediate_rep
                .needs_to_run_completion_script
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "needs_to_run_completion_script missing in IsCompleteResponse".to_string()
                })?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<IsCompleteResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<IsCompleteResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<IsCompleteResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for IsCompleteResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<IsCompleteResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <IsCompleteResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into IsCompleteResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<IsCompleteResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<IsCompleteResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<IsCompleteResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<IsCompleteResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <IsCompleteResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into IsCompleteResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct JobModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// Name of the job; no requirements on uniqueness
    #[serde(rename = "name")]
    pub name: String,

    /// CLI command to execute. Will not be executed in a shell and so must not include shell characters.
    #[serde(rename = "command")]
    pub command: String,

    /// Wrapper script for command in case the environment needs customization.
    #[serde(rename = "invocation_script")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_script: Option<String>,

    #[serde(rename = "status")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<JobStatus>,

    #[serde(rename = "schedule_compute_nodes")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_compute_nodes: Option<models::ComputeNodeSchedule>,

    /// Cancel this job if any of its blocking jobs fails.
    #[serde(rename = "cancel_on_blocking_job_failure")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_on_blocking_job_failure: Option<bool>,

    /// Informs torc that the job can be terminated gracefully before a wall-time timeout.
    #[serde(rename = "supports_termination")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_termination: Option<bool>,

    /// Database IDs of jobs that block this job
    #[serde(rename = "depends_on_job_ids")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on_job_ids: Option<Vec<i64>>,

    /// Database IDs of files that this job needs
    #[serde(rename = "input_file_ids")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_ids: Option<Vec<i64>>,

    /// Database IDs of files that this job produces
    #[serde(rename = "output_file_ids")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_ids: Option<Vec<i64>>,

    /// Database IDs of user-data objects that this job needs
    #[serde(rename = "input_user_data_ids")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_user_data_ids: Option<Vec<i64>>,

    /// Database IDs of user-data objects that this job produces
    #[serde(rename = "output_user_data_ids")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_user_data_ids: Option<Vec<i64>>,

    /// Optional database ID of resources required by this job
    #[serde(rename = "resource_requirements_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_requirements_id: Option<i64>,

    /// Optional database ID of scheduler needed by this job
    #[serde(rename = "scheduler_id")]
    pub scheduler_id: Option<i64>,

    /// Optional database ID of failure handler for this job
    #[serde(rename = "failure_handler_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_handler_id: Option<i64>,

    /// Retry attempt number (starts at 1, increments on each retry)
    #[serde(rename = "attempt_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<i64>,
}

impl JobModel {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64, name: String, command: String) -> JobModel {
        JobModel {
            id: None,
            workflow_id,
            name,
            command,
            invocation_script: None,
            status: Some(JobStatus::Uninitialized),
            schedule_compute_nodes: None,
            cancel_on_blocking_job_failure: Some(true),
            supports_termination: Some(false),
            depends_on_job_ids: None,
            input_file_ids: None,
            output_file_ids: None,
            input_user_data_ids: None,
            output_user_data_ids: None,
            resource_requirements_id: None,
            scheduler_id: None,
            failure_handler_id: None,
            attempt_id: Some(1),
        }
    }
}

/// Converts the JobModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for JobModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            Some("name".to_string()),
            Some(self.name.to_string()),
            Some("command".to_string()),
            Some(self.command.to_string()),
            self.invocation_script.as_ref().map(|invocation_script| {
                [
                    "invocation_script".to_string(),
                    invocation_script.to_string(),
                ]
                .join(",")
            }),
            // Skipping non-primitive type status in query parameter serialization
            // Skipping non-primitive type schedule_compute_nodes in query parameter serialization
            self.cancel_on_blocking_job_failure
                .as_ref()
                .map(|cancel_on_blocking_job_failure| {
                    [
                        "cancel_on_blocking_job_failure".to_string(),
                        cancel_on_blocking_job_failure.to_string(),
                    ]
                    .join(",")
                }),
            self.supports_termination
                .as_ref()
                .map(|supports_termination| {
                    [
                        "supports_termination".to_string(),
                        supports_termination.to_string(),
                    ]
                    .join(",")
                }),
            self.depends_on_job_ids.as_ref().map(|depends_on_job_ids| {
                [
                    "depends_on_job_ids".to_string(),
                    depends_on_job_ids
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                ]
                .join(",")
            }),
            self.input_file_ids.as_ref().map(|input_file_ids| {
                [
                    "input_file_ids".to_string(),
                    input_file_ids
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                ]
                .join(",")
            }),
            self.output_file_ids.as_ref().map(|output_file_ids| {
                [
                    "output_file_ids".to_string(),
                    output_file_ids
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                ]
                .join(",")
            }),
            self.input_user_data_ids
                .as_ref()
                .map(|input_user_data_ids| {
                    [
                        "input_user_data_ids".to_string(),
                        input_user_data_ids
                            .iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                    ]
                    .join(",")
                }),
            self.output_user_data_ids
                .as_ref()
                .map(|output_user_data_ids| {
                    [
                        "output_user_data_ids".to_string(),
                        output_user_data_ids
                            .iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                    ]
                    .join(",")
                }),
            self.resource_requirements_id
                .as_ref()
                .map(|resource_requirements_id| {
                    [
                        "resource_requirements_id".to_string(),
                        resource_requirements_id.to_string(),
                    ]
                    .join(",")
                }),
            self.scheduler_id.as_ref().map(|scheduler_id| {
                ["scheduler_id".to_string(), scheduler_id.to_string()].join(",")
            }),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a JobModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for JobModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub name: Vec<String>,
            pub command: Vec<String>,
            pub invocation_script: Vec<String>,
            pub status: Vec<JobStatus>,
            pub schedule_compute_nodes: Vec<models::ComputeNodeSchedule>,
            pub cancel_on_blocking_job_failure: Vec<bool>,
            pub supports_termination: Vec<bool>,
            pub depends_on_job_ids: Vec<Vec<i64>>,
            pub input_file_ids: Vec<Vec<i64>>,
            pub output_file_ids: Vec<Vec<i64>>,
            pub input_user_data_ids: Vec<Vec<i64>>,
            pub output_user_data_ids: Vec<Vec<i64>>,
            pub resource_requirements_id: Vec<i64>,
            pub scheduler_id: Vec<i64>,
            pub failure_handler_id: Vec<i64>,
            pub attempt_id: Vec<i64>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing JobModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "name" => intermediate_rep.name.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "command" => intermediate_rep.command.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "invocation_script" => intermediate_rep.invocation_script.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "status" => intermediate_rep.status.push(
                        <JobStatus as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    "schedule_compute_nodes" => intermediate_rep.schedule_compute_nodes.push(
                        <models::ComputeNodeSchedule as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    "cancel_on_blocking_job_failure" => {
                        intermediate_rep.cancel_on_blocking_job_failure.push(
                            <bool as std::str::FromStr>::from_str(val)
                                .map_err(|x| x.to_string())?,
                        )
                    }
                    "supports_termination" => intermediate_rep.supports_termination.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "depends_on_job_ids" => {
                        return std::result::Result::Err(
                            "Parsing a container in this style is not supported in JobModel"
                                .to_string(),
                        );
                    }
                    "input_file_ids" => {
                        return std::result::Result::Err(
                            "Parsing a container in this style is not supported in JobModel"
                                .to_string(),
                        );
                    }
                    "output_file_ids" => {
                        return std::result::Result::Err(
                            "Parsing a container in this style is not supported in JobModel"
                                .to_string(),
                        );
                    }
                    "input_user_data_ids" => {
                        return std::result::Result::Err(
                            "Parsing a container in this style is not supported in JobModel"
                                .to_string(),
                        );
                    }
                    "output_user_data_ids" => {
                        return std::result::Result::Err(
                            "Parsing a container in this style is not supported in JobModel"
                                .to_string(),
                        );
                    }
                    "resource_requirements_id" => intermediate_rep.resource_requirements_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler_id" => intermediate_rep.scheduler_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "failure_handler_id" => intermediate_rep.failure_handler_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "attempt_id" => intermediate_rep.attempt_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing JobModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(JobModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in JobModel".to_string())?,
            name: intermediate_rep
                .name
                .into_iter()
                .next()
                .ok_or_else(|| "name missing in JobModel".to_string())?,
            command: intermediate_rep
                .command
                .into_iter()
                .next()
                .ok_or_else(|| "command missing in JobModel".to_string())?,
            invocation_script: intermediate_rep.invocation_script.into_iter().next(),
            status: intermediate_rep.status.into_iter().next(),
            schedule_compute_nodes: intermediate_rep.schedule_compute_nodes.into_iter().next(),
            cancel_on_blocking_job_failure: intermediate_rep
                .cancel_on_blocking_job_failure
                .into_iter()
                .next(),
            supports_termination: intermediate_rep.supports_termination.into_iter().next(),
            depends_on_job_ids: intermediate_rep.depends_on_job_ids.into_iter().next(),
            input_file_ids: intermediate_rep.input_file_ids.into_iter().next(),
            output_file_ids: intermediate_rep.output_file_ids.into_iter().next(),
            input_user_data_ids: intermediate_rep.input_user_data_ids.into_iter().next(),
            output_user_data_ids: intermediate_rep.output_user_data_ids.into_iter().next(),
            resource_requirements_id: intermediate_rep.resource_requirements_id.into_iter().next(),
            scheduler_id: intermediate_rep.scheduler_id.into_iter().next(),
            failure_handler_id: intermediate_rep.failure_handler_id.into_iter().next(),
            attempt_id: intermediate_rep.attempt_id.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<JobModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<JobModel>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<JobModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for JobModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<JobModel> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <JobModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into JobModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<JobModel>>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<JobModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<Vec<JobModel>> {
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<JobModel> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <JobModel as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into JobModel - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

/// Job statuses
/// Enumeration of values.
/// Since this enum's variants do not hold data, we can easily define them as `#[repr(C)]`
/// which helps with FFI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    Hash,
    Default,
)]
#[cfg_attr(feature = "conversion", derive(frunk_enum_derive::LabelledGenericEnum))]
pub enum JobStatus {
    #[serde(rename = "uninitialized")]
    #[default]
    Uninitialized,
    #[serde(rename = "blocked")]
    Blocked,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "terminated")]
    Terminated,
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "pending_failed")]
    PendingFailed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            JobStatus::Uninitialized => write!(f, "uninitialized"),
            JobStatus::Blocked => write!(f, "blocked"),
            JobStatus::Ready => write!(f, "ready"),
            JobStatus::Pending => write!(f, "pending"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::Canceled => write!(f, "canceled"),
            JobStatus::Terminated => write!(f, "terminated"),
            JobStatus::Disabled => write!(f, "disabled"),
            JobStatus::PendingFailed => write!(f, "pending_failed"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "uninitialized" => std::result::Result::Ok(JobStatus::Uninitialized),
            "blocked" => std::result::Result::Ok(JobStatus::Blocked),
            "ready" => std::result::Result::Ok(JobStatus::Ready),
            "pending" => std::result::Result::Ok(JobStatus::Pending),
            "running" => std::result::Result::Ok(JobStatus::Running),
            "completed" => std::result::Result::Ok(JobStatus::Completed),
            "failed" => std::result::Result::Ok(JobStatus::Failed),
            "canceled" => std::result::Result::Ok(JobStatus::Canceled),
            "terminated" => std::result::Result::Ok(JobStatus::Terminated),
            "disabled" => std::result::Result::Ok(JobStatus::Disabled),
            "pending_failed" => std::result::Result::Ok(JobStatus::PendingFailed),
            _ => std::result::Result::Err(format!("Value not valid: {}", s)),
        }
    }
}

use std::collections::HashMap;
use std::sync::OnceLock;

impl JobStatus {
    /// Convert JobStatus enum to integer for database storage
    pub fn to_int(&self) -> i32 {
        match *self {
            JobStatus::Uninitialized => 0,
            JobStatus::Blocked => 1,
            JobStatus::Ready => 2,
            JobStatus::Pending => 3,
            JobStatus::Running => 4,
            JobStatus::Completed => 5,
            JobStatus::Failed => 6,
            JobStatus::Canceled => 7,
            JobStatus::Terminated => 8,
            JobStatus::Disabled => 9,
            JobStatus::PendingFailed => 10,
        }
    }

    /// Convert integer from database to JobStatus enum
    pub fn from_int(value: i32) -> std::result::Result<Self, String> {
        match value {
            0 => Ok(JobStatus::Uninitialized),
            1 => Ok(JobStatus::Blocked),
            2 => Ok(JobStatus::Ready),
            3 => Ok(JobStatus::Pending),
            4 => Ok(JobStatus::Running),
            5 => Ok(JobStatus::Completed),
            6 => Ok(JobStatus::Failed),
            7 => Ok(JobStatus::Canceled),
            8 => Ok(JobStatus::Terminated),
            9 => Ok(JobStatus::Disabled),
            10 => Ok(JobStatus::PendingFailed),
            _ => Err(format!("Invalid JobStatus integer value: {}", value)),
        }
    }

    /// Convert i64 from SQLite database to JobStatus enum
    pub fn from_i64(value: i64) -> std::result::Result<Self, String> {
        Self::from_int(value as i32)
    }
}

/// JobStatus mapping utilities for fast lookups
pub struct JobStatusMap;

impl JobStatusMap {
    /// Get the static HashMap for enum to integer mapping
    pub fn enum_to_int_map() -> &'static HashMap<JobStatus, i32> {
        static MAP: OnceLock<HashMap<JobStatus, i32>> = OnceLock::new();
        MAP.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert(JobStatus::Uninitialized, 0);
            map.insert(JobStatus::Blocked, 1);
            map.insert(JobStatus::Ready, 2);
            map.insert(JobStatus::Pending, 3);
            map.insert(JobStatus::Running, 4);
            map.insert(JobStatus::Completed, 5);
            map.insert(JobStatus::Failed, 6);
            map.insert(JobStatus::Canceled, 7);
            map.insert(JobStatus::Terminated, 8);
            map.insert(JobStatus::Disabled, 9);
            map.insert(JobStatus::PendingFailed, 10);
            map
        })
    }

    /// Get the static HashMap for integer to enum mapping
    pub fn int_to_enum_map() -> &'static HashMap<i32, JobStatus> {
        static MAP: OnceLock<HashMap<i32, JobStatus>> = OnceLock::new();
        MAP.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert(0, JobStatus::Uninitialized);
            map.insert(1, JobStatus::Blocked);
            map.insert(2, JobStatus::Ready);
            map.insert(3, JobStatus::Pending);
            map.insert(4, JobStatus::Running);
            map.insert(5, JobStatus::Completed);
            map.insert(6, JobStatus::Failed);
            map.insert(7, JobStatus::Canceled);
            map.insert(8, JobStatus::Terminated);
            map.insert(9, JobStatus::Disabled);
            map.insert(10, JobStatus::PendingFailed);
            map
        })
    }

    /// Convert enum to integer using HashMap lookup
    pub fn to_int(status: &JobStatus) -> i32 {
        *Self::enum_to_int_map().get(status).unwrap_or(&-1)
    }

    /// Convert integer to enum using HashMap lookup
    pub fn from_int(value: i32) -> Option<JobStatus> {
        Self::int_to_enum_map().get(&value).copied()
    }

    /// Convert i64 to enum using HashMap lookup (for SQLite compatibility)
    pub fn from_i64(value: i64) -> Option<JobStatus> {
        Self::from_int(value as i32)
    }
}

// Methods for converting between header::IntoHeaderValue<JobStatus> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<JobStatus>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<JobStatus>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for JobStatus - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<JobStatus> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <JobStatus as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into JobStatus - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<JobStatus>>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<JobStatus>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<Vec<JobStatus>> {
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<JobStatus> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <JobStatus as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into JobStatus - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

impl JobStatus {
    /// Returns true if the job status indicates the job has finished executing
    /// and reached a terminal state that can be set via complete_job API.
    /// This includes: Completed, Failed, Canceled, Terminated, PendingFailed
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed
                | JobStatus::Failed
                | JobStatus::Canceled
                | JobStatus::Terminated
                | JobStatus::PendingFailed
        )
    }

    /// Returns true if the job status indicates the workflow can progress.
    /// PendingFailed is NOT considered complete for workflow progression purposes
    /// because it's awaiting AI classification.
    /// Complete statuses: Completed, Failed, Canceled, Terminated
    pub fn is_complete(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Canceled | JobStatus::Terminated
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status_is_complete() {
        // Test complete statuses (workflow can progress)
        assert!(JobStatus::Completed.is_complete());
        assert!(JobStatus::Failed.is_complete());
        assert!(JobStatus::Canceled.is_complete());
        assert!(JobStatus::Terminated.is_complete());

        // PendingFailed is NOT complete (workflow cannot progress)
        assert!(!JobStatus::PendingFailed.is_complete());

        // Test incomplete statuses
        assert!(!JobStatus::Uninitialized.is_complete());
        assert!(!JobStatus::Blocked.is_complete());
        assert!(!JobStatus::Ready.is_complete());
        assert!(!JobStatus::Running.is_complete());
        assert!(!JobStatus::Pending.is_complete());
        assert!(!JobStatus::Disabled.is_complete());
    }

    #[test]
    fn test_job_status_is_terminal() {
        // Test terminal statuses (finished executing)
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::Canceled.is_terminal());
        assert!(JobStatus::Terminated.is_terminal());
        assert!(JobStatus::PendingFailed.is_terminal());

        // Test non-terminal statuses (still executing or not started)
        assert!(!JobStatus::Uninitialized.is_terminal());
        assert!(!JobStatus::Blocked.is_terminal());
        assert!(!JobStatus::Ready.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
        assert!(!JobStatus::Pending.is_terminal());
        assert!(!JobStatus::Disabled.is_terminal());
    }

    #[test]
    fn test_job_status_integer_mapping() {
        // Test all enum variants map to correct integers
        assert_eq!(JobStatus::Uninitialized.to_int(), 0);
        assert_eq!(JobStatus::Blocked.to_int(), 1);
        assert_eq!(JobStatus::Ready.to_int(), 2);
        assert_eq!(JobStatus::Pending.to_int(), 3);
        assert_eq!(JobStatus::Running.to_int(), 4);
        assert_eq!(JobStatus::Completed.to_int(), 5);
        assert_eq!(JobStatus::Failed.to_int(), 6);
        assert_eq!(JobStatus::Canceled.to_int(), 7);
        assert_eq!(JobStatus::Terminated.to_int(), 8);
        assert_eq!(JobStatus::Disabled.to_int(), 9);
        assert_eq!(JobStatus::PendingFailed.to_int(), 10);
    }

    #[test]
    fn test_job_status_from_integer() {
        // Test all integers map back to correct enum variants
        assert_eq!(JobStatus::from_int(0).unwrap(), JobStatus::Uninitialized);
        assert_eq!(JobStatus::from_int(1).unwrap(), JobStatus::Blocked);
        assert_eq!(JobStatus::from_int(2).unwrap(), JobStatus::Ready);
        assert_eq!(JobStatus::from_int(3).unwrap(), JobStatus::Pending);
        assert_eq!(JobStatus::from_int(4).unwrap(), JobStatus::Running);
        assert_eq!(JobStatus::from_int(5).unwrap(), JobStatus::Completed);
        assert_eq!(JobStatus::from_int(6).unwrap(), JobStatus::Failed);
        assert_eq!(JobStatus::from_int(7).unwrap(), JobStatus::Canceled);
        assert_eq!(JobStatus::from_int(8).unwrap(), JobStatus::Terminated);
        assert_eq!(JobStatus::from_int(9).unwrap(), JobStatus::Disabled);
        assert_eq!(JobStatus::from_int(10).unwrap(), JobStatus::PendingFailed);

        // Test invalid integer
        assert!(JobStatus::from_int(11).is_err());
        assert!(JobStatus::from_int(-1).is_err());
    }

    #[test]
    fn test_job_status_hashmap_mapping() {
        // Test HashMap-based conversions
        assert_eq!(JobStatusMap::to_int(&JobStatus::Completed), 5);
        assert_eq!(JobStatusMap::from_int(5).unwrap(), JobStatus::Completed);
        assert_eq!(JobStatusMap::to_int(&JobStatus::Ready), 2);
        assert_eq!(JobStatusMap::from_int(2).unwrap(), JobStatus::Ready);

        // Test invalid lookup
        assert!(JobStatusMap::from_int(99).is_none());
    }

    #[test]
    fn test_job_status_roundtrip() {
        // Test roundtrip conversion for all variants
        let variants = [
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
        ];

        for variant in &variants {
            let int_val = variant.to_int();
            let back_to_enum = JobStatus::from_int(int_val).unwrap();
            assert_eq!(*variant, back_to_enum);
        }
    }
}

/// Data model for a batch of jobs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct JobsModel {
    /// Jobs in the batch
    #[serde(rename = "jobs")]
    pub jobs: Vec<models::JobModel>,
}

impl JobsModel {
    #[allow(clippy::new_without_default)]
    pub fn new(jobs: Vec<models::JobModel>) -> JobsModel {
        JobsModel { jobs }
    }
}

/// Converts the JobsModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for JobsModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type jobs in query parameter serialization
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a JobsModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for JobsModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub jobs: Vec<Vec<models::JobModel>>,
        }

        let intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "jobs" => {
                        return std::result::Result::Err(
                            "Parsing a container in this style is not supported in JobsModel"
                                .to_string(),
                        );
                    }
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing JobsModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(JobsModel {
            jobs: intermediate_rep
                .jobs
                .into_iter()
                .next()
                .ok_or_else(|| "jobs missing in JobsModel".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<JobsModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<JobsModel>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<JobsModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for JobsModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<JobsModel> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <JobsModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into JobsModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<JobsModel>>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<JobsModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<Vec<JobsModel>> {
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<JobsModel> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <JobsModel as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into JobsModel - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListComputeNodesResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::ComputeNodeModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListComputeNodesResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListComputeNodesResponse {
        ListComputeNodesResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListComputeNodesResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListComputeNodesResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListComputeNodesResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListComputeNodesResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::ComputeNodeModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListComputeNodesResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err("Parsing a container in this style is not supported in ListComputeNodesResponse".to_string()),
                    "offset" => intermediate_rep.offset.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "max_limit" => intermediate_rep.max_limit.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "count" => intermediate_rep.count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "total_count" => intermediate_rep.total_count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "has_more" => intermediate_rep.has_more.push(<bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListComputeNodesResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListComputeNodesResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListComputeNodesResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListComputeNodesResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListComputeNodesResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListComputeNodesResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListComputeNodesResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListComputeNodesResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListComputeNodesResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListComputeNodesResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListComputeNodesResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListComputeNodesResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListComputeNodesResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListComputeNodesResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListComputeNodesResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListComputeNodesResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListComputeNodesResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListComputeNodesResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListComputeNodesResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListComputeNodesResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListEventsResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::EventModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListEventsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListEventsResponse {
        ListEventsResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListEventsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListEventsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListEventsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListEventsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::EventModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListEventsResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err(
                        "Parsing a container in this style is not supported in ListEventsResponse"
                            .to_string(),
                    ),
                    "offset" => intermediate_rep.offset.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "max_limit" => intermediate_rep.max_limit.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "count" => intermediate_rep.count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "total_count" => intermediate_rep.total_count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "has_more" => intermediate_rep.has_more.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ListEventsResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListEventsResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListEventsResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListEventsResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListEventsResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListEventsResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListEventsResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListEventsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListEventsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListEventsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListEventsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListEventsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListEventsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListEventsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListEventsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListEventsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListEventsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListEventsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListEventsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListEventsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListFilesResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::FileModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListFilesResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListFilesResponse {
        ListFilesResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListFilesResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListFilesResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListFilesResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListFilesResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::FileModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListFilesResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err(
                        "Parsing a container in this style is not supported in ListFilesResponse"
                            .to_string(),
                    ),
                    "offset" => intermediate_rep.offset.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "max_limit" => intermediate_rep.max_limit.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "count" => intermediate_rep.count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "total_count" => intermediate_rep.total_count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "has_more" => intermediate_rep.has_more.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ListFilesResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListFilesResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListFilesResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListFilesResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListFilesResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListFilesResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListFilesResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListFilesResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListFilesResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListFilesResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListFilesResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListFilesResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListFilesResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListFilesResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListFilesResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListFilesResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListFilesResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListFilesResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListFilesResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListFilesResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListJobsResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::JobModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListJobsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListJobsResponse {
        ListJobsResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListJobsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListJobsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListJobsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListJobsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::JobModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListJobsResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err(
                        "Parsing a container in this style is not supported in ListJobsResponse"
                            .to_string(),
                    ),
                    "offset" => intermediate_rep.offset.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "max_limit" => intermediate_rep.max_limit.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "count" => intermediate_rep.count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "total_count" => intermediate_rep.total_count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "has_more" => intermediate_rep.has_more.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ListJobsResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListJobsResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListJobsResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListJobsResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListJobsResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListJobsResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListJobsResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListJobsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListJobsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListJobsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListJobsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListJobsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListJobsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListJobsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListJobsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListJobsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListJobsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListJobsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListJobsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListJobsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListLocalSchedulersResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::LocalSchedulerModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListLocalSchedulersResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListLocalSchedulersResponse {
        ListLocalSchedulersResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListLocalSchedulersResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListLocalSchedulersResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListLocalSchedulersResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListLocalSchedulersResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::LocalSchedulerModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListLocalSchedulersResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err("Parsing a container in this style is not supported in ListLocalSchedulersResponse".to_string()),
                    "offset" => intermediate_rep.offset.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "max_limit" => intermediate_rep.max_limit.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "count" => intermediate_rep.count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "total_count" => intermediate_rep.total_count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "has_more" => intermediate_rep.has_more.push(<bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListLocalSchedulersResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListLocalSchedulersResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListLocalSchedulersResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListLocalSchedulersResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListLocalSchedulersResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListLocalSchedulersResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListLocalSchedulersResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListLocalSchedulersResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListLocalSchedulersResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListLocalSchedulersResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListLocalSchedulersResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListLocalSchedulersResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListLocalSchedulersResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListLocalSchedulersResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListLocalSchedulersResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListLocalSchedulersResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListLocalSchedulersResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListLocalSchedulersResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListLocalSchedulersResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListLocalSchedulersResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListMissingUserDataResponse {
    #[serde(rename = "user_data")]
    pub user_data: Vec<i64>,
}

impl ListMissingUserDataResponse {
    #[allow(clippy::new_without_default)]
    pub fn new() -> ListMissingUserDataResponse {
        ListMissingUserDataResponse {
            user_data: Vec::new(),
        }
    }
}

/// Converts the ListMissingUserDataResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListMissingUserDataResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![Some(
            [
                "user_data".to_string(),
                self.user_data
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ]
            .join("="),
        )];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListMissingUserDataResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListMissingUserDataResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub user_data: Vec<Vec<i64>>,
        }

        let intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "user_data" => return std::result::Result::Err("Parsing a container in this style is not supported in ListMissingUserDataResponse".to_string()),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListMissingUserDataResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListMissingUserDataResponse {
            user_data: intermediate_rep
                .user_data
                .into_iter()
                .next()
                .unwrap_or_default(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListMissingUserDataResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListMissingUserDataResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListMissingUserDataResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListMissingUserDataResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListMissingUserDataResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListMissingUserDataResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListMissingUserDataResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListMissingUserDataResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListMissingUserDataResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListMissingUserDataResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListMissingUserDataResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListMissingUserDataResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListMissingUserDataResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListRequiredExistingFilesResponse {
    #[serde(rename = "files")]
    pub files: Vec<i64>,
}

impl ListRequiredExistingFilesResponse {
    #[allow(clippy::new_without_default)]
    pub fn new() -> ListRequiredExistingFilesResponse {
        ListRequiredExistingFilesResponse { files: Vec::new() }
    }
}

/// Converts the ListRequiredExistingFilesResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListRequiredExistingFilesResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![Some(
            [
                "files".to_string(),
                self.files
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ]
            .join("="),
        )];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListRequiredExistingFilesResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListRequiredExistingFilesResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub files: Vec<Vec<i64>>,
        }

        let intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "files" => return std::result::Result::Err("Parsing a container in this style is not supported in ListRequiredExistingFilesResponse".to_string()),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListRequiredExistingFilesResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListRequiredExistingFilesResponse {
            files: intermediate_rep
                .files
                .into_iter()
                .next()
                .unwrap_or_default(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListRequiredExistingFilesResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListRequiredExistingFilesResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListRequiredExistingFilesResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListRequiredExistingFilesResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListRequiredExistingFilesResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListRequiredExistingFilesResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListRequiredExistingFilesResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListRequiredExistingFilesResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListRequiredExistingFilesResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListRequiredExistingFilesResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListRequiredExistingFilesResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListRequiredExistingFilesResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListRequiredExistingFilesResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListResourceRequirementsResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::ResourceRequirementsModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListResourceRequirementsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListResourceRequirementsResponse {
        ListResourceRequirementsResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListResourceRequirementsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListResourceRequirementsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListResourceRequirementsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListResourceRequirementsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::ResourceRequirementsModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListResourceRequirementsResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err("Parsing a container in this style is not supported in ListResourceRequirementsResponse".to_string()),
                    "offset" => intermediate_rep.offset.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "max_limit" => intermediate_rep.max_limit.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "count" => intermediate_rep.count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "total_count" => intermediate_rep.total_count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "has_more" => intermediate_rep.has_more.push(<bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListResourceRequirementsResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListResourceRequirementsResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListResourceRequirementsResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "max_limit missing in ListResourceRequirementsResponse".to_string()
                })?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListResourceRequirementsResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "total_count missing in ListResourceRequirementsResponse".to_string()
                })?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "has_more missing in ListResourceRequirementsResponse".to_string()
                })?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListResourceRequirementsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListResourceRequirementsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListResourceRequirementsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListResourceRequirementsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListResourceRequirementsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListResourceRequirementsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListResourceRequirementsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListResourceRequirementsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListResourceRequirementsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListResourceRequirementsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListResourceRequirementsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListResourceRequirementsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListResourceRequirementsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListResultsResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::ResultModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListResultsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListResultsResponse {
        ListResultsResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListResultsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListResultsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListResultsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListResultsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::ResultModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListResultsResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err(
                        "Parsing a container in this style is not supported in ListResultsResponse"
                            .to_string(),
                    ),
                    "offset" => intermediate_rep.offset.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "max_limit" => intermediate_rep.max_limit.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "count" => intermediate_rep.count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "total_count" => intermediate_rep.total_count.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "has_more" => intermediate_rep.has_more.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ListResultsResponse".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListResultsResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListResultsResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListResultsResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListResultsResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListResultsResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListResultsResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListResultsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListResultsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListResultsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListResultsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListResultsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListResultsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListResultsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListResultsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListResultsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListResultsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListResultsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListResultsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListResultsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListScheduledComputeNodesResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::ScheduledComputeNodesModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListScheduledComputeNodesResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListScheduledComputeNodesResponse {
        ListScheduledComputeNodesResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListScheduledComputeNodesResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListScheduledComputeNodesResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListScheduledComputeNodesResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListScheduledComputeNodesResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::ScheduledComputeNodesModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListScheduledComputeNodesResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err("Parsing a container in this style is not supported in ListScheduledComputeNodesResponse".to_string()),
                    "offset" => intermediate_rep.offset.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "max_limit" => intermediate_rep.max_limit.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "count" => intermediate_rep.count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "total_count" => intermediate_rep.total_count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "has_more" => intermediate_rep.has_more.push(<bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListScheduledComputeNodesResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListScheduledComputeNodesResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListScheduledComputeNodesResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "max_limit missing in ListScheduledComputeNodesResponse".to_string()
                })?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListScheduledComputeNodesResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "total_count missing in ListScheduledComputeNodesResponse".to_string()
                })?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "has_more missing in ListScheduledComputeNodesResponse".to_string()
                })?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListScheduledComputeNodesResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListScheduledComputeNodesResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListScheduledComputeNodesResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListScheduledComputeNodesResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListScheduledComputeNodesResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListScheduledComputeNodesResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListScheduledComputeNodesResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListScheduledComputeNodesResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListScheduledComputeNodesResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListScheduledComputeNodesResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListScheduledComputeNodesResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListScheduledComputeNodesResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListScheduledComputeNodesResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListSlurmSchedulersResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::SlurmSchedulerModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListSlurmSchedulersResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListSlurmSchedulersResponse {
        ListSlurmSchedulersResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListSlurmSchedulersResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListSlurmSchedulersResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListSlurmSchedulersResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListSlurmSchedulersResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::SlurmSchedulerModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListSlurmSchedulersResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err("Parsing a container in this style is not supported in ListSlurmSchedulersResponse".to_string()),
                    "offset" => intermediate_rep.offset.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "max_limit" => intermediate_rep.max_limit.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "count" => intermediate_rep.count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "total_count" => intermediate_rep.total_count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "has_more" => intermediate_rep.has_more.push(<bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListSlurmSchedulersResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListSlurmSchedulersResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListSlurmSchedulersResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListSlurmSchedulersResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListSlurmSchedulersResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListSlurmSchedulersResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListSlurmSchedulersResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListSlurmSchedulersResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListSlurmSchedulersResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListSlurmSchedulersResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListSlurmSchedulersResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListSlurmSchedulersResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListSlurmSchedulersResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListSlurmSchedulersResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListSlurmSchedulersResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListSlurmSchedulersResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListSlurmSchedulersResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListSlurmSchedulersResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListSlurmSchedulersResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListSlurmSchedulersResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListUserDataResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::UserDataModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListUserDataResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListUserDataResponse {
        ListUserDataResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListUserDataResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListUserDataResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListUserDataResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListUserDataResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::UserDataModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListUserDataResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err("Parsing a container in this style is not supported in ListUserDataResponse".to_string()),
                    "offset" => intermediate_rep.offset.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "max_limit" => intermediate_rep.max_limit.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "count" => intermediate_rep.count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "total_count" => intermediate_rep.total_count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "has_more" => intermediate_rep.has_more.push(<bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListUserDataResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListUserDataResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListUserDataResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListUserDataResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListUserDataResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListUserDataResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListUserDataResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListUserDataResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListUserDataResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListUserDataResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListUserDataResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListUserDataResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListUserDataResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListUserDataResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListUserDataResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListUserDataResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListUserDataResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListUserDataResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListUserDataResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListUserDataResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListWorkflowsResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::WorkflowModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListWorkflowsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListWorkflowsResponse {
        ListWorkflowsResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Converts the ListWorkflowsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ListWorkflowsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type items in query parameter serialization
            Some("offset".to_string()),
            Some(self.offset.to_string()),
            Some("max_limit".to_string()),
            Some(self.max_limit.to_string()),
            Some("count".to_string()),
            Some(self.count.to_string()),
            Some("total_count".to_string()),
            Some(self.total_count.to_string()),
            Some("has_more".to_string()),
            Some(self.has_more.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ListWorkflowsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ListWorkflowsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub items: Vec<Vec<models::WorkflowModel>>,
            pub offset: Vec<i64>,
            pub max_limit: Vec<i64>,
            pub count: Vec<i64>,
            pub total_count: Vec<i64>,
            pub has_more: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ListWorkflowsResponse".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "items" => return std::result::Result::Err("Parsing a container in this style is not supported in ListWorkflowsResponse".to_string()),
                    "offset" => intermediate_rep.offset.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "max_limit" => intermediate_rep.max_limit.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "count" => intermediate_rep.count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "total_count" => intermediate_rep.total_count.push(<i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    "has_more" => intermediate_rep.has_more.push(<bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ListWorkflowsResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ListWorkflowsResponse {
            items: intermediate_rep.items.into_iter().next(),
            offset: intermediate_rep
                .offset
                .into_iter()
                .next()
                .ok_or_else(|| "offset missing in ListWorkflowsResponse".to_string())?,
            max_limit: intermediate_rep
                .max_limit
                .into_iter()
                .next()
                .ok_or_else(|| "max_limit missing in ListWorkflowsResponse".to_string())?,
            count: intermediate_rep
                .count
                .into_iter()
                .next()
                .ok_or_else(|| "count missing in ListWorkflowsResponse".to_string())?,
            total_count: intermediate_rep
                .total_count
                .into_iter()
                .next()
                .ok_or_else(|| "total_count missing in ListWorkflowsResponse".to_string())?,
            has_more: intermediate_rep
                .has_more
                .into_iter()
                .next()
                .ok_or_else(|| "has_more missing in ListWorkflowsResponse".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ListWorkflowsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ListWorkflowsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ListWorkflowsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ListWorkflowsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ListWorkflowsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ListWorkflowsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ListWorkflowsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ListWorkflowsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ListWorkflowsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ListWorkflowsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ListWorkflowsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ListWorkflowsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ListWorkflowsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct LocalSchedulerModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    #[serde(rename = "name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(rename = "memory")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    #[serde(rename = "num_cpus")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_cpus: Option<i64>,
}

impl LocalSchedulerModel {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64) -> LocalSchedulerModel {
        LocalSchedulerModel {
            id: None,
            workflow_id,
            name: Some("default".to_string()),
            memory: None,
            num_cpus: None,
        }
    }
}

/// Converts the LocalSchedulerModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for LocalSchedulerModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            self.name
                .as_ref()
                .map(|name| ["name".to_string(), name.to_string()].join(",")),
            self.memory
                .as_ref()
                .map(|memory| ["memory".to_string(), memory.to_string()].join(",")),
            self.num_cpus
                .as_ref()
                .map(|num_cpus| ["num_cpus".to_string(), num_cpus.to_string()].join(",")),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a LocalSchedulerModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for LocalSchedulerModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub name: Vec<String>,
            pub memory: Vec<String>,
            pub num_cpus: Vec<i64>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing LocalSchedulerModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "name" => intermediate_rep.name.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "memory" => intermediate_rep.memory.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_cpus" => intermediate_rep.num_cpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing LocalSchedulerModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(LocalSchedulerModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in LocalSchedulerModel".to_string())?,
            name: intermediate_rep.name.into_iter().next(),
            memory: intermediate_rep.memory.into_iter().next(),
            num_cpus: intermediate_rep.num_cpus.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<LocalSchedulerModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<LocalSchedulerModel>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<LocalSchedulerModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for LocalSchedulerModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<LocalSchedulerModel>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <LocalSchedulerModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into LocalSchedulerModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<LocalSchedulerModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<LocalSchedulerModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<LocalSchedulerModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<LocalSchedulerModel> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <LocalSchedulerModel as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into LocalSchedulerModel - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ClaimJobsBasedOnResources {
    #[serde(rename = "jobs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<models::JobModel>>,

    #[serde(rename = "reason")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ClaimJobsBasedOnResources {
    #[allow(clippy::new_without_default)]
    pub fn new() -> ClaimJobsBasedOnResources {
        ClaimJobsBasedOnResources {
            jobs: None,
            reason: None,
        }
    }
}

/// Converts the ClaimJobsBasedOnResources value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ClaimJobsBasedOnResources {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type jobs in query parameter serialization
            self.reason
                .as_ref()
                .map(|reason| ["reason".to_string(), reason.to_string()].join(",")),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ClaimJobsBasedOnResources value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ClaimJobsBasedOnResources {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub jobs: Vec<Vec<models::JobModel>>,
            pub reason: Vec<String>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ClaimJobsBasedOnResources".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "jobs" => return std::result::Result::Err("Parsing a container in this style is not supported in ClaimJobsBasedOnResources".to_string()),
                    "reason" => intermediate_rep.reason.push(<String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?),
                    _ => return std::result::Result::Err("Unexpected key while parsing ClaimJobsBasedOnResources".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ClaimJobsBasedOnResources {
            jobs: intermediate_rep.jobs.into_iter().next(),
            reason: intermediate_rep.reason.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ClaimJobsBasedOnResources> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ClaimJobsBasedOnResources>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ClaimJobsBasedOnResources>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ClaimJobsBasedOnResources - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ClaimJobsBasedOnResources>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ClaimJobsBasedOnResources as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ClaimJobsBasedOnResources - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ClaimJobsBasedOnResources>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ClaimJobsBasedOnResources>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ClaimJobsBasedOnResources>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ClaimJobsBasedOnResources> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ClaimJobsBasedOnResources as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ClaimJobsBasedOnResources - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

/// Inform the server to use this sort method when processing the claim_jobs_based_on_resources command.
/// Enumeration of values.
/// Since this enum's variants do not hold data, we can easily define them as `#[repr(C)]`
/// which helps with FFI.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    Hash,
    Default,
)]
#[cfg_attr(feature = "conversion", derive(frunk_enum_derive::LabelledGenericEnum))]
pub enum ClaimJobsSortMethod {
    #[serde(rename = "gpus_runtime_memory")]
    GpusRuntimeMemory,
    #[serde(rename = "gpus_memory_runtime")]
    GpusMemoryRuntime,
    #[serde(rename = "none")]
    #[default]
    None,
}

impl std::fmt::Display for ClaimJobsSortMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ClaimJobsSortMethod::GpusRuntimeMemory => write!(f, "gpus_runtime_memory"),
            ClaimJobsSortMethod::GpusMemoryRuntime => write!(f, "gpus_memory_runtime"),
            ClaimJobsSortMethod::None => write!(f, "none"),
        }
    }
}

impl std::str::FromStr for ClaimJobsSortMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "gpus_runtime_memory" => {
                std::result::Result::Ok(ClaimJobsSortMethod::GpusRuntimeMemory)
            }
            "gpus_memory_runtime" => {
                std::result::Result::Ok(ClaimJobsSortMethod::GpusMemoryRuntime)
            }
            "none" => std::result::Result::Ok(ClaimJobsSortMethod::None),
            _ => std::result::Result::Err(format!("Value not valid: {}", s)),
        }
    }
}

// Methods for converting between header::IntoHeaderValue<ClaimJobsSortMethod> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ClaimJobsSortMethod>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ClaimJobsSortMethod>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ClaimJobsSortMethod - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ClaimJobsSortMethod>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ClaimJobsSortMethod as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ClaimJobsSortMethod - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ClaimJobsSortMethod>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ClaimJobsSortMethod>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ClaimJobsSortMethod>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ClaimJobsSortMethod> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ClaimJobsSortMethod as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ClaimJobsSortMethod - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ClaimNextJobsResponse {
    #[serde(rename = "jobs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<models::JobModel>>,
}

impl ClaimNextJobsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new() -> ClaimNextJobsResponse {
        ClaimNextJobsResponse { jobs: None }
    }
}

/// Converts the ClaimNextJobsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ClaimNextJobsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            // Skipping non-primitive type jobs in query parameter serialization
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ClaimNextJobsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ClaimNextJobsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub jobs: Vec<Vec<models::JobModel>>,
        }

        let intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "jobs" => return std::result::Result::Err("Parsing a container in this style is not supported in ClaimNextJobsResponse".to_string()),
                    _ => return std::result::Result::Err("Unexpected key while parsing ClaimNextJobsResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ClaimNextJobsResponse {
            jobs: intermediate_rep.jobs.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ClaimNextJobsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ClaimNextJobsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ClaimNextJobsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ClaimNextJobsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ClaimNextJobsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ClaimNextJobsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ClaimNextJobsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ClaimNextJobsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ClaimNextJobsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ClaimNextJobsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ClaimNextJobsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ClaimNextJobsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ClaimNextJobsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ProcessChangedJobInputsResponse {
    #[serde(rename = "reinitialized_jobs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reinitialized_jobs: Option<Vec<String>>,
}

impl ProcessChangedJobInputsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new() -> ProcessChangedJobInputsResponse {
        ProcessChangedJobInputsResponse {
            reinitialized_jobs: None,
        }
    }
}

/// Converts the ProcessChangedJobInputsResponse value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ProcessChangedJobInputsResponse {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> =
            vec![self.reinitialized_jobs.as_ref().map(|reinitialized_jobs| {
                [
                    "reinitialized_jobs".to_string(),
                    reinitialized_jobs
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                ]
                .join(",")
            })];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ProcessChangedJobInputsResponse value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ProcessChangedJobInputsResponse {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub reinitialized_jobs: Vec<Vec<String>>,
        }

        let intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "reinitialized_jobs" => return std::result::Result::Err("Parsing a container in this style is not supported in ProcessChangedJobInputsResponse".to_string()),
                    _ => return std::result::Result::Err("Unexpected key while parsing ProcessChangedJobInputsResponse".to_string())
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ProcessChangedJobInputsResponse {
            reinitialized_jobs: intermediate_rep.reinitialized_jobs.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ProcessChangedJobInputsResponse> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ProcessChangedJobInputsResponse>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ProcessChangedJobInputsResponse>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ProcessChangedJobInputsResponse - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ProcessChangedJobInputsResponse>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ProcessChangedJobInputsResponse as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ProcessChangedJobInputsResponse - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ProcessChangedJobInputsResponse>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ProcessChangedJobInputsResponse>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ProcessChangedJobInputsResponse>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ProcessChangedJobInputsResponse> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ProcessChangedJobInputsResponse as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ProcessChangedJobInputsResponse - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ResourceRequirementsModel {
    /// Database ID of this record.
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Database ID of the workflow this record is associated with.
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// Name of the resource requirements
    #[serde(rename = "name")]
    pub name: String,

    /// Number of CPUs required by a job
    #[serde(rename = "num_cpus")]
    pub num_cpus: i64,

    /// Number of GPUs required by a job
    #[serde(rename = "num_gpus")]
    pub num_gpus: i64,

    /// Number of nodes required by a job
    #[serde(rename = "num_nodes")]
    pub num_nodes: i64,

    /// Amount of memory required by a job, e.g., 20g
    #[serde(rename = "memory")]
    pub memory: String,

    /// Maximum runtime for a job
    #[serde(rename = "runtime")]
    pub runtime: String,
}

impl ResourceRequirementsModel {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64, name: String) -> ResourceRequirementsModel {
        ResourceRequirementsModel {
            id: None,
            workflow_id,
            name,
            num_cpus: 1,
            num_gpus: 0,
            num_nodes: 1,
            memory: "1m".to_string(),
            runtime: "P0DT1M".to_string(),
        }
    }
}

/// Converts the ResourceRequirementsModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ResourceRequirementsModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            Some(self.name.to_string()),
            Some(self.num_cpus.to_string()),
            Some(self.num_gpus.to_string()),
            Some(self.num_nodes.to_string()),
            Some(self.memory.to_string()),
            Some(self.runtime.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ResourceRequirementsModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ResourceRequirementsModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub name: Vec<String>,
            pub num_cpus: Vec<i64>,
            pub num_gpus: Vec<i64>,
            pub num_nodes: Vec<i64>,
            pub memory: Vec<String>,
            pub runtime: Vec<String>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ResourceRequirementsModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "name" => intermediate_rep.name.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_cpus" => intermediate_rep.num_cpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_gpus" => intermediate_rep.num_gpus.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "num_nodes" => intermediate_rep.num_nodes.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "memory" => intermediate_rep.memory.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "runtime" => intermediate_rep.runtime.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ResourceRequirementsModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ResourceRequirementsModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in ResourceRequirementsModel".to_string())?,
            name: intermediate_rep
                .name
                .into_iter()
                .next()
                .ok_or_else(|| "name missing in ResourceRequirementsModel".to_string())?,
            num_cpus: intermediate_rep
                .num_cpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_cpus missing in ResourceRequirementsModel".to_string())?,
            num_gpus: intermediate_rep
                .num_gpus
                .into_iter()
                .next()
                .ok_or_else(|| "num_gpus missing in ResourceRequirementsModel".to_string())?,
            num_nodes: intermediate_rep
                .num_nodes
                .into_iter()
                .next()
                .ok_or_else(|| "num_nodes missing in ResourceRequirementsModel".to_string())?,
            memory: intermediate_rep
                .memory
                .into_iter()
                .next()
                .ok_or_else(|| "memory missing in ResourceRequirementsModel".to_string())?,
            runtime: intermediate_rep
                .runtime
                .into_iter()
                .next()
                .ok_or_else(|| "runtime missing in ResourceRequirementsModel".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ResourceRequirementsModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ResourceRequirementsModel>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ResourceRequirementsModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ResourceRequirementsModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ResourceRequirementsModel>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ResourceRequirementsModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ResourceRequirementsModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ResourceRequirementsModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ResourceRequirementsModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ResourceRequirementsModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ResourceRequirementsModel> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ResourceRequirementsModel as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ResourceRequirementsModel - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ResultModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Database ID for the job tied to this result
    #[serde(rename = "job_id")]
    pub job_id: i64,

    /// Database ID for the workflow tied to this result
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// ID of the workflow run. Incremements on every start and restart.
    #[serde(rename = "run_id")]
    pub run_id: i64,

    /// Retry attempt number for this result (starts at 1, increments on each retry)
    #[serde(rename = "attempt_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<i64>,

    /// Database ID for the compute node that ran this job
    #[serde(rename = "compute_node_id")]
    pub compute_node_id: i64,

    /// Code returned by the job. Zero is success; non-zero is a failure.
    #[serde(rename = "return_code")]
    pub return_code: i64,

    /// Job execution time in minutes
    #[serde(rename = "exec_time_minutes")]
    pub exec_time_minutes: f64,

    /// Timestamp of when the job completed.
    #[serde(rename = "completion_time")]
    pub completion_time: String,

    /// Peak memory usage in bytes
    #[serde(rename = "peak_memory_bytes")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_memory_bytes: Option<i64>,

    /// Average memory usage in bytes
    #[serde(rename = "avg_memory_bytes")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_memory_bytes: Option<i64>,

    /// Peak CPU usage as percentage (can exceed 100% for multi-core)
    #[serde(rename = "peak_cpu_percent")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_cpu_percent: Option<f64>,

    /// Average CPU usage as percentage (can exceed 100% for multi-core)
    #[serde(rename = "avg_cpu_percent")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_cpu_percent: Option<f64>,

    #[serde(rename = "status")]
    pub status: JobStatus,
}

impl ResultModel {
    #[allow(clippy::new_without_default)]
    pub fn new(
        job_id: i64,
        workflow_id: i64,
        run_id: i64,
        attempt_id: i64,
        compute_node_id: i64,
        return_code: i64,
        exec_time_minutes: f64,
        completion_time: String,
        status: JobStatus,
    ) -> ResultModel {
        ResultModel {
            id: None,
            job_id,
            workflow_id,
            run_id,
            attempt_id: Some(attempt_id),
            compute_node_id,
            return_code,
            exec_time_minutes,
            completion_time,
            peak_memory_bytes: None,
            avg_memory_bytes: None,
            peak_cpu_percent: None,
            avg_cpu_percent: None,
            status,
        }
    }
}

/// Converts the ResultModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ResultModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("job_id".to_string()),
            Some(self.job_id.to_string()),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            Some("run_id".to_string()),
            Some(self.run_id.to_string()),
            Some("return_code".to_string()),
            Some(self.return_code.to_string()),
            Some("exec_time_minutes".to_string()),
            Some(self.exec_time_minutes.to_string()),
            Some("completion_time".to_string()),
            Some(self.completion_time.to_string()),
            // Skipping non-primitive type status in query parameter serialization
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ResultModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ResultModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub job_id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub run_id: Vec<i64>,
            pub attempt_id: Vec<i64>,
            pub compute_node_id: Vec<i64>,
            pub return_code: Vec<i64>,
            pub exec_time_minutes: Vec<f64>,
            pub completion_time: Vec<String>,
            pub peak_memory_bytes: Vec<i64>,
            pub avg_memory_bytes: Vec<i64>,
            pub peak_cpu_percent: Vec<f64>,
            pub avg_cpu_percent: Vec<f64>,
            pub status: Vec<JobStatus>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ResultModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "job_id" => intermediate_rep.job_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "run_id" => intermediate_rep.run_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "attempt_id" => intermediate_rep.attempt_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "compute_node_id" => intermediate_rep.compute_node_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "return_code" => intermediate_rep.return_code.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "exec_time_minutes" => intermediate_rep.exec_time_minutes.push(
                        <f64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "completion_time" => intermediate_rep.completion_time.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "status" => intermediate_rep.status.push(
                        <JobStatus as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ResultModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ResultModel {
            id: intermediate_rep.id.into_iter().next(),
            job_id: intermediate_rep
                .job_id
                .into_iter()
                .next()
                .ok_or_else(|| "job_id missing in ResultModel".to_string())?,
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in ResultModel".to_string())?,
            run_id: intermediate_rep
                .run_id
                .into_iter()
                .next()
                .ok_or_else(|| "run_id missing in ResultModel".to_string())?,
            attempt_id: intermediate_rep.attempt_id.into_iter().next(),
            compute_node_id: intermediate_rep
                .compute_node_id
                .into_iter()
                .next()
                .ok_or_else(|| "compute_node_id missing in ResultModel".to_string())?,
            return_code: intermediate_rep
                .return_code
                .into_iter()
                .next()
                .ok_or_else(|| "return_code missing in ResultModel".to_string())?,
            exec_time_minutes: intermediate_rep
                .exec_time_minutes
                .into_iter()
                .next()
                .ok_or_else(|| "exec_time_minutes missing in ResultModel".to_string())?,
            completion_time: intermediate_rep
                .completion_time
                .into_iter()
                .next()
                .ok_or_else(|| "completion_time missing in ResultModel".to_string())?,
            peak_memory_bytes: intermediate_rep.peak_memory_bytes.into_iter().next(),
            avg_memory_bytes: intermediate_rep.avg_memory_bytes.into_iter().next(),
            peak_cpu_percent: intermediate_rep.peak_cpu_percent.into_iter().next(),
            avg_cpu_percent: intermediate_rep.avg_cpu_percent.into_iter().next(),
            status: intermediate_rep
                .status
                .into_iter()
                .next()
                .ok_or_else(|| "status missing in ResultModel".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ResultModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ResultModel>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ResultModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ResultModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<ResultModel> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ResultModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ResultModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ResultModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ResultModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ResultModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<ResultModel> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <ResultModel as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into ResultModel - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ScheduledComputeNodesModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    #[serde(rename = "scheduler_id")]
    #[serde()]
    pub scheduler_id: i64,

    #[serde(rename = "scheduler_config_id")]
    pub scheduler_config_id: i64,

    #[serde(rename = "scheduler_type")]
    pub scheduler_type: String,

    #[serde(rename = "status")]
    pub status: String,
}

impl ScheduledComputeNodesModel {
    #[allow(clippy::new_without_default)]
    pub fn new(
        workflow_id: i64,
        scheduler_id: i64,
        scheduler_config_id: i64,
        scheduler_type: String,
        status: String,
    ) -> ScheduledComputeNodesModel {
        ScheduledComputeNodesModel {
            id: None,
            workflow_id,
            scheduler_id,
            scheduler_config_id,
            scheduler_type,
            status,
        }
    }
}

/// Converts the ScheduledComputeNodesModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for ScheduledComputeNodesModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            Some("scheduler_id".to_string()),
            Some(self.scheduler_id.to_string()),
            Some("scheduler_config_id".to_string()),
            Some(self.scheduler_config_id.to_string()),
            Some("scheduler_type".to_string()),
            Some(self.scheduler_type.to_string()),
            Some("status".to_string()),
            Some(self.status.to_string()),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a ScheduledComputeNodesModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for ScheduledComputeNodesModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub scheduler_id: Vec<i64>,
            pub scheduler_config_id: Vec<i64>,
            pub scheduler_type: Vec<String>,
            pub status: Vec<String>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing ScheduledComputeNodesModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler_id" => intermediate_rep.scheduler_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler_config_id" => intermediate_rep.scheduler_config_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "scheduler_type" => intermediate_rep.scheduler_type.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "status" => intermediate_rep.status.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing ScheduledComputeNodesModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(ScheduledComputeNodesModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in ScheduledComputeNodesModel".to_string())?,
            scheduler_id: intermediate_rep
                .scheduler_id
                .into_iter()
                .next()
                .ok_or_else(|| "scheduler_id missing in ScheduledComputeNodesModel".to_string())?,
            scheduler_config_id: intermediate_rep
                .scheduler_config_id
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "scheduler_config_id missing in ScheduledComputeNodesModel".to_string()
                })?,
            scheduler_type: intermediate_rep
                .scheduler_type
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "scheduler_type missing in ScheduledComputeNodesModel".to_string()
                })?,
            status: intermediate_rep
                .status
                .into_iter()
                .next()
                .ok_or_else(|| "status missing in ScheduledComputeNodesModel".to_string())?,
        })
    }
}

// Methods for converting between header::IntoHeaderValue<ScheduledComputeNodesModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<ScheduledComputeNodesModel>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<ScheduledComputeNodesModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for ScheduledComputeNodesModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<ScheduledComputeNodesModel>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <ScheduledComputeNodesModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into ScheduledComputeNodesModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<ScheduledComputeNodesModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<ScheduledComputeNodesModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<ScheduledComputeNodesModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<ScheduledComputeNodesModel> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <ScheduledComputeNodesModel as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into ScheduledComputeNodesModel - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

/// Data model for Slurm scheduler
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct SlurmSchedulerModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Database ID of the workflow this scheduler is associated with.
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// Name of the scheduler
    #[serde(rename = "name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Slurm account ID
    #[serde(rename = "account")]
    pub account: String,

    /// Generic resource requirement
    #[serde(rename = "gres")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gres: Option<String>,

    /// Compute node memory requirement
    #[serde(rename = "mem")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem: Option<String>,

    /// Number of nodes for the Slurm allocation
    #[serde(rename = "nodes")]
    pub nodes: i64,

    /// Number of tasks to invoke on each node
    #[serde(rename = "ntasks_per_node")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ntasks_per_node: Option<i64>,

    /// Compute node partition; likely not necessary because Slurm should optimize it.
    #[serde(rename = "partition")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,

    /// Priority of Slurm job
    #[serde(rename = "qos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qos: Option<String>,

    /// Compute node local storage size requirement
    #[serde(rename = "tmp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmp: Option<String>,

    /// Slurm runtime requirement, e.g., 04:00:00
    #[serde(rename = "walltime")]
    pub walltime: String,

    /// Extra Slurm parameters that torc will append to the sbatch command
    #[serde(rename = "extra")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

impl SlurmSchedulerModel {
    #[allow(clippy::new_without_default)]
    pub fn new(
        workflow_id: i64,
        account: String,
        nodes: i64,
        walltime: String,
    ) -> SlurmSchedulerModel {
        SlurmSchedulerModel {
            id: None,
            workflow_id,
            name: None,
            account,
            gres: None,
            mem: None,
            nodes,
            ntasks_per_node: None,
            partition: None,
            qos: Some("normal".to_string()),
            tmp: None,
            walltime,
            extra: None,
        }
    }
}

/// Converts the SlurmSchedulerModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for SlurmSchedulerModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            self.name
                .as_ref()
                .map(|name| ["name".to_string(), name.to_string()].join(",")),
            Some("account".to_string()),
            Some(self.account.to_string()),
            self.gres
                .as_ref()
                .map(|gres| ["gres".to_string(), gres.to_string()].join(",")),
            self.mem
                .as_ref()
                .map(|mem| ["mem".to_string(), mem.to_string()].join(",")),
            Some("nodes".to_string()),
            Some(self.nodes.to_string()),
            self.ntasks_per_node.as_ref().map(|ntasks_per_node| {
                ["ntasks_per_node".to_string(), ntasks_per_node.to_string()].join(",")
            }),
            self.partition
                .as_ref()
                .map(|partition| ["partition".to_string(), partition.to_string()].join(",")),
            self.qos
                .as_ref()
                .map(|qos| ["qos".to_string(), qos.to_string()].join(",")),
            self.tmp
                .as_ref()
                .map(|tmp| ["tmp".to_string(), tmp.to_string()].join(",")),
            Some(["walltime".to_string(), self.walltime.to_string()].join(",")),
            self.extra
                .as_ref()
                .map(|extra| ["extra".to_string(), extra.to_string()].join(",")),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a SlurmSchedulerModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for SlurmSchedulerModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub name: Vec<String>,
            pub account: Vec<String>,
            pub gres: Vec<String>,
            pub mem: Vec<String>,
            pub nodes: Vec<i64>,
            pub ntasks_per_node: Vec<i64>,
            pub partition: Vec<String>,
            pub qos: Vec<String>,
            pub tmp: Vec<String>,
            pub walltime: Vec<String>,
            pub extra: Vec<String>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing SlurmSchedulerModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "name" => intermediate_rep.name.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "account" => intermediate_rep.account.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "gres" => intermediate_rep.gres.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "mem" => intermediate_rep.mem.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "nodes" => intermediate_rep.nodes.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "ntasks_per_node" => intermediate_rep.ntasks_per_node.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "partition" => intermediate_rep.partition.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "qos" => intermediate_rep.qos.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "tmp" => intermediate_rep.tmp.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "walltime" => intermediate_rep.walltime.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "extra" => intermediate_rep.extra.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing SlurmSchedulerModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(SlurmSchedulerModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in SlurmSchedulerModel".to_string())?,
            name: intermediate_rep.name.into_iter().next(),
            account: intermediate_rep
                .account
                .into_iter()
                .next()
                .ok_or_else(|| "account missing in SlurmSchedulerModel".to_string())?,
            gres: intermediate_rep.gres.into_iter().next(),
            mem: intermediate_rep.mem.into_iter().next(),
            nodes: intermediate_rep
                .nodes
                .into_iter()
                .next()
                .ok_or_else(|| "nodes missing in SlurmSchedulerModel".to_string())?,
            ntasks_per_node: intermediate_rep.ntasks_per_node.into_iter().next(),
            partition: intermediate_rep.partition.into_iter().next(),
            qos: intermediate_rep.qos.into_iter().next(),
            tmp: intermediate_rep.tmp.into_iter().next(),
            walltime: intermediate_rep
                .walltime
                .into_iter()
                .next()
                .ok_or_else(|| "walltime missing in SlurmSchedulerModel".to_string())?,
            extra: intermediate_rep.extra.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<SlurmSchedulerModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<SlurmSchedulerModel>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<SlurmSchedulerModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for SlurmSchedulerModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<SlurmSchedulerModel>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <SlurmSchedulerModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into SlurmSchedulerModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<SlurmSchedulerModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<SlurmSchedulerModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<SlurmSchedulerModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<SlurmSchedulerModel> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <SlurmSchedulerModel as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into SlurmSchedulerModel - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct UserDataModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Database ID of the workflow this record is associated with.
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// The data will only exist for the duration of one run. Torc will clear it before starting new runs.
    #[serde(rename = "is_ephemeral")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_ephemeral: Option<bool>,

    /// Name of the data object
    #[serde(rename = "name")]
    pub name: String,

    /// User-defined data
    #[serde(rename = "data")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl UserDataModel {
    pub fn new(workflow_id: i64, name: String) -> UserDataModel {
        UserDataModel {
            id: None,
            workflow_id,
            is_ephemeral: Some(false),
            name,
            data: None,
        }
    }
}

/// Converts the UserDataModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for UserDataModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("workflow_id".to_string()),
            Some(self.workflow_id.to_string()),
            self.is_ephemeral.as_ref().map(|is_ephemeral| {
                ["is_ephemeral".to_string(), is_ephemeral.to_string()].join(",")
            }),
            Some(["name".to_string(), self.name.to_string()].join(",")),
            // Skipping non-primitive type data in query parameter serialization
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a UserDataModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for UserDataModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub workflow_id: Vec<i64>,
            pub is_ephemeral: Vec<bool>,
            pub name: Vec<String>,
            pub data: Vec<serde_json::Value>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing UserDataModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "workflow_id" => intermediate_rep.workflow_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "is_ephemeral" => intermediate_rep.is_ephemeral.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "name" => intermediate_rep.name.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "data" => intermediate_rep.data.push(
                        <serde_json::Value as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing UserDataModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(UserDataModel {
            id: intermediate_rep.id.into_iter().next(),
            workflow_id: intermediate_rep
                .workflow_id
                .into_iter()
                .next()
                .ok_or_else(|| "workflow_id missing in UserDataModel".to_string())?,
            is_ephemeral: intermediate_rep.is_ephemeral.into_iter().next(),
            name: intermediate_rep
                .name
                .into_iter()
                .next()
                .ok_or_else(|| "name missing in UserDataModel".to_string())?,
            data: intermediate_rep.data.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<UserDataModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<UserDataModel>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<UserDataModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for UserDataModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<UserDataModel> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <UserDataModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into UserDataModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<UserDataModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<UserDataModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<UserDataModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<UserDataModel> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <UserDataModel as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into UserDataModel - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct WorkflowModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Name of the workflow
    #[serde(rename = "name")]
    pub name: String,

    /// User that created the workflow
    #[serde(rename = "user")]
    pub user: String,

    /// Description of the workflow
    #[serde(rename = "description")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Timestamp of workflow creation
    #[serde(rename = "timestamp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,

    /// Inform all compute nodes to shut down this number of seconds before the expiration time. This allows torc to send SIGTERM to all job processes and set all statuses to terminated. Increase the time in cases where the job processes handle SIGTERM and need more time to gracefully shut down. Set the value to 0 to maximize the time given to jobs. If not set, take the database's default value of 60 seconds.
    #[serde(rename = "compute_node_expiration_buffer_seconds")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_expiration_buffer_seconds: Option<i64>,

    /// Inform all compute nodes to wait for new jobs for this time period before exiting. Does not apply if the workflow is complete.
    #[serde(rename = "compute_node_wait_for_new_jobs_seconds")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_wait_for_new_jobs_seconds: Option<i64>,

    /// Inform all compute nodes to ignore workflow completions and hold onto allocations indefinitely. Useful for debugging failed jobs and possibly dynamic workflows where jobs get added after starting.
    #[serde(rename = "compute_node_ignore_workflow_completion")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_ignore_workflow_completion: Option<bool>,

    /// Inform all compute nodes to wait this number of minutes if the database becomes unresponsive.
    #[serde(rename = "compute_node_wait_for_healthy_database_minutes")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_wait_for_healthy_database_minutes: Option<i64>,

    /// Minimum remaining walltime (in seconds) required before a compute node will request new jobs.
    /// If the remaining time is less than this value, the compute node will stop requesting new jobs
    /// and wait for running jobs to complete. This prevents starting jobs that won't have enough time
    /// to complete. Default is 300 seconds (5 minutes).
    #[serde(rename = "compute_node_min_time_for_new_jobs_seconds")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_min_time_for_new_jobs_seconds: Option<i64>,

    #[serde(rename = "jobs_sort_method")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs_sort_method: Option<models::ClaimJobsSortMethod>,

    /// Resource monitoring configuration as JSON string
    #[serde(rename = "resource_monitor_config")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_monitor_config: Option<String>,

    /// Default Slurm parameters to apply to all schedulers as JSON string
    #[serde(rename = "slurm_defaults")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_defaults: Option<String>,

    /// Use PendingFailed status for failed jobs (enables AI-assisted recovery)
    #[serde(rename = "use_pending_failed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_pending_failed: Option<bool>,

    /// Project name or identifier for grouping workflows
    #[serde(rename = "project")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,

    /// Arbitrary metadata as JSON string
    #[serde(rename = "metadata")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,

    #[serde(rename = "status_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_id: Option<i64>,
}

impl WorkflowModel {
    #[allow(clippy::new_without_default)]
    pub fn new(name: String, user: String) -> WorkflowModel {
        WorkflowModel {
            id: None,
            name,
            user,
            description: None,
            timestamp: None,
            compute_node_expiration_buffer_seconds: Some(180),
            compute_node_wait_for_new_jobs_seconds: Some(0),
            compute_node_ignore_workflow_completion: Some(false),
            compute_node_wait_for_healthy_database_minutes: Some(20),
            compute_node_min_time_for_new_jobs_seconds: Some(300),
            jobs_sort_method: None,
            resource_monitor_config: None,
            slurm_defaults: None,
            use_pending_failed: Some(false),
            project: None,
            metadata: None,
            status_id: None,
        }
    }
}

/// Converts the WorkflowModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for WorkflowModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("name".to_string()),
            Some(self.name.to_string()),
            Some("user".to_string()),
            Some(self.user.to_string()),
            self.description
                .as_ref()
                .map(|description| ["description".to_string(), description.to_string()].join(",")),
            self.timestamp
                .as_ref()
                .map(|timestamp| ["timestamp".to_string(), timestamp.to_string()].join(",")),
            self.compute_node_expiration_buffer_seconds.as_ref().map(
                |compute_node_expiration_buffer_seconds| {
                    [
                        "compute_node_expiration_buffer_seconds".to_string(),
                        compute_node_expiration_buffer_seconds.to_string(),
                    ]
                    .join(",")
                },
            ),
            self.compute_node_wait_for_new_jobs_seconds.as_ref().map(
                |compute_node_wait_for_new_jobs_seconds| {
                    [
                        "compute_node_wait_for_new_jobs_seconds".to_string(),
                        compute_node_wait_for_new_jobs_seconds.to_string(),
                    ]
                    .join(",")
                },
            ),
            self.compute_node_ignore_workflow_completion.as_ref().map(
                |compute_node_ignore_workflow_completion| {
                    [
                        "compute_node_ignore_workflow_completion".to_string(),
                        compute_node_ignore_workflow_completion.to_string(),
                    ]
                    .join(",")
                },
            ),
            self.compute_node_wait_for_healthy_database_minutes
                .as_ref()
                .map(|compute_node_wait_for_healthy_database_minutes| {
                    [
                        "compute_node_wait_for_healthy_database_minutes".to_string(),
                        compute_node_wait_for_healthy_database_minutes.to_string(),
                    ]
                    .join(",")
                }),
            self.compute_node_min_time_for_new_jobs_seconds
                .as_ref()
                .map(|compute_node_min_time_for_new_jobs_seconds| {
                    [
                        "compute_node_min_time_for_new_jobs_seconds".to_string(),
                        compute_node_min_time_for_new_jobs_seconds.to_string(),
                    ]
                    .join(",")
                }),
            // Skipping non-primitive type jobs_sort_method in query parameter serialization
            self.use_pending_failed.as_ref().map(|use_pending_failed| {
                [
                    "use_pending_failed".to_string(),
                    use_pending_failed.to_string(),
                ]
                .join(",")
            }),
            self.status_id
                .as_ref()
                .map(|status_id| ["status_id".to_string(), status_id.to_string()].join(",")),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a WorkflowModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for WorkflowModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub name: Vec<String>,
            pub user: Vec<String>,
            pub description: Vec<String>,
            pub timestamp: Vec<String>,
            pub compute_node_expiration_buffer_seconds: Vec<i64>,
            pub compute_node_wait_for_new_jobs_seconds: Vec<i64>,
            pub compute_node_ignore_workflow_completion: Vec<bool>,
            pub compute_node_wait_for_healthy_database_minutes: Vec<i64>,
            pub compute_node_min_time_for_new_jobs_seconds: Vec<i64>,
            pub jobs_sort_method: Vec<models::ClaimJobsSortMethod>,
            pub resource_monitor_config: Vec<String>,
            pub slurm_defaults: Vec<String>,
            pub use_pending_failed: Vec<bool>,
            pub project: Vec<String>,
            pub metadata: Vec<String>,
            pub status_id: Vec<i64>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing WorkflowModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "name" => intermediate_rep.name.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "user" => intermediate_rep.user.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "description" => intermediate_rep.description.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "timestamp" => intermediate_rep.timestamp.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "compute_node_expiration_buffer_seconds" => intermediate_rep
                        .compute_node_expiration_buffer_seconds
                        .push(
                            <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                        ),
                    "compute_node_wait_for_new_jobs_seconds" => intermediate_rep
                        .compute_node_wait_for_new_jobs_seconds
                        .push(
                            <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                        ),
                    "compute_node_ignore_workflow_completion" => intermediate_rep
                        .compute_node_ignore_workflow_completion
                        .push(
                            <bool as std::str::FromStr>::from_str(val)
                                .map_err(|x| x.to_string())?,
                        ),
                    "compute_node_wait_for_healthy_database_minutes" => intermediate_rep
                        .compute_node_wait_for_healthy_database_minutes
                        .push(
                            <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                        ),
                    "compute_node_min_time_for_new_jobs_seconds" => intermediate_rep
                        .compute_node_min_time_for_new_jobs_seconds
                        .push(
                            <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                        ),
                    "jobs_sort_method" => intermediate_rep.jobs_sort_method.push(
                        <models::ClaimJobsSortMethod as std::str::FromStr>::from_str(val)
                            .map_err(|x| x.to_string())?,
                    ),
                    "status_id" => intermediate_rep.status_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "use_pending_failed" => intermediate_rep.use_pending_failed.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "project" => intermediate_rep.project.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "metadata" => intermediate_rep.metadata.push(
                        <String as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing WorkflowModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(WorkflowModel {
            id: intermediate_rep.id.into_iter().next(),
            name: intermediate_rep
                .name
                .into_iter()
                .next()
                .ok_or_else(|| "name missing in WorkflowModel".to_string())?,
            user: intermediate_rep
                .user
                .into_iter()
                .next()
                .ok_or_else(|| "user missing in WorkflowModel".to_string())?,
            description: intermediate_rep.description.into_iter().next(),
            timestamp: intermediate_rep.timestamp.into_iter().next(),
            compute_node_expiration_buffer_seconds: intermediate_rep
                .compute_node_expiration_buffer_seconds
                .into_iter()
                .next(),
            compute_node_wait_for_new_jobs_seconds: intermediate_rep
                .compute_node_wait_for_new_jobs_seconds
                .into_iter()
                .next(),
            compute_node_ignore_workflow_completion: intermediate_rep
                .compute_node_ignore_workflow_completion
                .into_iter()
                .next(),
            compute_node_wait_for_healthy_database_minutes: intermediate_rep
                .compute_node_wait_for_healthy_database_minutes
                .into_iter()
                .next(),
            compute_node_min_time_for_new_jobs_seconds: intermediate_rep
                .compute_node_min_time_for_new_jobs_seconds
                .into_iter()
                .next(),
            jobs_sort_method: intermediate_rep.jobs_sort_method.into_iter().next(),
            resource_monitor_config: intermediate_rep.resource_monitor_config.into_iter().next(),
            slurm_defaults: intermediate_rep.slurm_defaults.into_iter().next(),
            use_pending_failed: intermediate_rep.use_pending_failed.into_iter().next(),
            project: intermediate_rep.project.into_iter().next(),
            metadata: intermediate_rep.metadata.into_iter().next(),
            status_id: intermediate_rep.status_id.into_iter().next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<WorkflowModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<WorkflowModel>> for hyper::header::HeaderValue {
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<WorkflowModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for WorkflowModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue> for header::IntoHeaderValue<WorkflowModel> {
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <WorkflowModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into WorkflowModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<WorkflowModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<WorkflowModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<WorkflowModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values: std::vec::Vec<WorkflowModel> = hdr_values
                    .split(',')
                    .filter_map(|hdr_value| match hdr_value.trim() {
                        "" => std::option::Option::None,
                        hdr_value => std::option::Option::Some({
                            match <WorkflowModel as std::str::FromStr>::from_str(hdr_value) {
                                std::result::Result::Ok(value) => std::result::Result::Ok(value),
                                std::result::Result::Err(err) => std::result::Result::Err(format!(
                                    "Unable to convert header value '{}' into WorkflowModel - {}",
                                    hdr_value, err
                                )),
                            }
                        }),
                    })
                    .collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

/// Data model for a workflow
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct WorkflowStatusModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Flag indicating whether the workflow has been canceled.
    #[serde(rename = "is_canceled")]
    pub is_canceled: bool,

    /// Flag indicating whether the workflow has been archived.
    #[serde(rename = "is_archived")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_archived: Option<bool>,

    #[serde(rename = "run_id")]
    pub run_id: i64,

    #[serde(rename = "has_detected_need_to_run_completion_script")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_detected_need_to_run_completion_script: Option<bool>,
}

impl WorkflowStatusModel {
    #[allow(clippy::new_without_default)]
    pub fn new(is_canceled: bool, run_id: i64) -> WorkflowStatusModel {
        WorkflowStatusModel {
            id: None,
            is_canceled,
            is_archived: Some(false),
            run_id,
            has_detected_need_to_run_completion_script: Some(false),
        }
    }
}

/// Converts the WorkflowStatusModel value to the Query Parameters representation (style=form, explode=false)
/// specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde serializer
impl std::string::ToString for WorkflowStatusModel {
    fn to_string(&self) -> String {
        let params: Vec<Option<String>> = vec![
            self.id
                .as_ref()
                .map(|id| ["id".to_string(), id.to_string()].join(",")),
            Some("is_canceled".to_string()),
            Some(self.is_canceled.to_string()),
            self.is_archived
                .as_ref()
                .map(|is_archived| ["is_archived".to_string(), is_archived.to_string()].join(",")),
            Some("run_id".to_string()),
            Some(self.run_id.to_string()),
            self.has_detected_need_to_run_completion_script
                .as_ref()
                .map(|has_detected_need_to_run_completion_script| {
                    [
                        "has_detected_need_to_run_completion_script".to_string(),
                        has_detected_need_to_run_completion_script.to_string(),
                    ]
                    .join(",")
                }),
        ];

        params.into_iter().flatten().collect::<Vec<_>>().join(",")
    }
}

/// Converts Query Parameters representation (style=form, explode=false) to a WorkflowStatusModel value
/// as specified in https://swagger.io/docs/specification/serialization/
/// Should be implemented in a serde deserializer
impl std::str::FromStr for WorkflowStatusModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        /// An intermediate representation of the struct to use for parsing.
        #[derive(Default)]
        #[allow(dead_code)]
        struct IntermediateRep {
            pub id: Vec<i64>,
            pub is_canceled: Vec<bool>,
            pub is_archived: Vec<bool>,
            pub run_id: Vec<i64>,
            pub has_detected_need_to_run_completion_script: Vec<bool>,
        }

        let mut intermediate_rep = IntermediateRep::default();

        // Parse into intermediate representation
        let mut string_iter = s.split(',');
        let mut key_result = string_iter.next();

        while key_result.is_some() {
            let val = match string_iter.next() {
                Some(x) => x,
                None => {
                    return std::result::Result::Err(
                        "Missing value while parsing WorkflowStatusModel".to_string(),
                    );
                }
            };

            if let Some(key) = key_result {
                #[allow(clippy::match_single_binding)]
                match key {
                    "id" => intermediate_rep.id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "is_canceled" => intermediate_rep.is_canceled.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "is_archived" => intermediate_rep.is_archived.push(
                        <bool as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "run_id" => intermediate_rep.run_id.push(
                        <i64 as std::str::FromStr>::from_str(val).map_err(|x| x.to_string())?,
                    ),
                    "has_detected_need_to_run_completion_script" => intermediate_rep
                        .has_detected_need_to_run_completion_script
                        .push(
                            <bool as std::str::FromStr>::from_str(val)
                                .map_err(|x| x.to_string())?,
                        ),
                    _ => {
                        return std::result::Result::Err(
                            "Unexpected key while parsing WorkflowStatusModel".to_string(),
                        );
                    }
                }
            }

            // Get the next key
            key_result = string_iter.next();
        }

        // Use the intermediate representation to return the struct
        std::result::Result::Ok(WorkflowStatusModel {
            id: intermediate_rep.id.into_iter().next(),
            is_canceled: intermediate_rep
                .is_canceled
                .into_iter()
                .next()
                .ok_or_else(|| "is_canceled missing in WorkflowStatusModel".to_string())?,
            is_archived: intermediate_rep.is_archived.into_iter().next(),
            run_id: intermediate_rep
                .run_id
                .into_iter()
                .next()
                .ok_or_else(|| "run_id missing in WorkflowStatusModel".to_string())?,
            has_detected_need_to_run_completion_script: intermediate_rep
                .has_detected_need_to_run_completion_script
                .into_iter()
                .next(),
        })
    }
}

// Methods for converting between header::IntoHeaderValue<WorkflowStatusModel> and hyper::header::HeaderValue

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<WorkflowStatusModel>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_value: header::IntoHeaderValue<WorkflowStatusModel>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_value = hdr_value.to_string();
        match hyper::header::HeaderValue::from_str(&hdr_value) {
            std::result::Result::Ok(value) => std::result::Result::Ok(value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Invalid header value for WorkflowStatusModel - value: {} is invalid {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<WorkflowStatusModel>
{
    type Error = String;

    fn try_from(hdr_value: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_value.to_str() {
            std::result::Result::Ok(value) => {
                match <WorkflowStatusModel as std::str::FromStr>::from_str(value) {
                    std::result::Result::Ok(value) => {
                        std::result::Result::Ok(header::IntoHeaderValue(value))
                    }
                    std::result::Result::Err(err) => std::result::Result::Err(format!(
                        "Unable to convert header value '{}' into WorkflowStatusModel - {}",
                        value, err
                    )),
                }
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert header: {:?} to string: {}",
                hdr_value, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<header::IntoHeaderValue<Vec<WorkflowStatusModel>>>
    for hyper::header::HeaderValue
{
    type Error = String;

    fn try_from(
        hdr_values: header::IntoHeaderValue<Vec<WorkflowStatusModel>>,
    ) -> std::result::Result<Self, Self::Error> {
        let hdr_values: Vec<String> = hdr_values
            .0
            .into_iter()
            .map(|hdr_value| hdr_value.to_string())
            .collect();

        match hyper::header::HeaderValue::from_str(&hdr_values.join(", ")) {
            std::result::Result::Ok(hdr_value) => std::result::Result::Ok(hdr_value),
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to convert {:?} into a header - {}",
                hdr_values, e
            )),
        }
    }
}

#[cfg(feature = "server")]
impl std::convert::TryFrom<hyper::header::HeaderValue>
    for header::IntoHeaderValue<Vec<WorkflowStatusModel>>
{
    type Error = String;

    fn try_from(hdr_values: hyper::header::HeaderValue) -> std::result::Result<Self, Self::Error> {
        match hdr_values.to_str() {
            std::result::Result::Ok(hdr_values) => {
                let hdr_values : std::vec::Vec<WorkflowStatusModel> = hdr_values
                .split(',')
                .filter_map(|hdr_value| match hdr_value.trim() {
                    "" => std::option::Option::None,
                    hdr_value => std::option::Option::Some({
                        match <WorkflowStatusModel as std::str::FromStr>::from_str(hdr_value) {
                            std::result::Result::Ok(value) => std::result::Result::Ok(value),
                            std::result::Result::Err(err) => std::result::Result::Err(
                                format!("Unable to convert header value '{}' into WorkflowStatusModel - {}",
                                    hdr_value, err))
                        }
                    })
                }).collect::<std::result::Result<std::vec::Vec<_>, String>>()?;

                std::result::Result::Ok(header::IntoHeaderValue(hdr_values))
            }
            std::result::Result::Err(e) => std::result::Result::Err(format!(
                "Unable to parse header: {:?} as a string - {}",
                hdr_values, e
            )),
        }
    }
}

// JobDependencyModel - Represents a blocking relationship between two jobs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct JobDependencyModel {
    /// The job that is blocked
    #[serde(rename = "job_id")]
    pub job_id: i64,

    /// The name of the job that is blocked
    #[serde(rename = "job_name")]
    pub job_name: String,

    /// The job that must complete first
    #[serde(rename = "depends_on_job_id")]
    pub depends_on_job_id: i64,

    /// The name of the job that must complete first
    #[serde(rename = "depends_on_job_name")]
    pub depends_on_job_name: String,

    /// The workflow containing both jobs
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,
}

impl JobDependencyModel {
    pub fn new(
        job_id: i64,
        job_name: String,
        depends_on_job_id: i64,
        depends_on_job_name: String,
        workflow_id: i64,
    ) -> JobDependencyModel {
        JobDependencyModel {
            job_id,
            job_name,
            depends_on_job_id,
            depends_on_job_name,
            workflow_id,
        }
    }
}

// ListJobDependenciesResponse - Response for listing job dependencies
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListJobDependenciesResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::JobDependencyModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListJobDependenciesResponse {
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListJobDependenciesResponse {
        ListJobDependenciesResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

// JobFileRelationshipModel - Represents a job-file relationship showing producer and consumer jobs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct JobFileRelationshipModel {
    /// The file ID
    #[serde(rename = "file_id")]
    pub file_id: i64,

    /// The name of the file
    #[serde(rename = "file_name")]
    pub file_name: String,

    /// The path of the file
    #[serde(rename = "file_path")]
    pub file_path: String,

    /// The job that produces this file (null for workflow inputs)
    #[serde(rename = "producer_job_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_id: Option<i64>,

    /// The name of the job that produces this file
    #[serde(rename = "producer_job_name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_name: Option<String>,

    /// The job that consumes this file (null for workflow outputs)
    #[serde(rename = "consumer_job_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_id: Option<i64>,

    /// The name of the job that consumes this file
    #[serde(rename = "consumer_job_name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_name: Option<String>,

    /// The workflow containing the file and jobs
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,
}

impl JobFileRelationshipModel {
    pub fn new(
        file_id: i64,
        file_name: String,
        file_path: String,
        workflow_id: i64,
    ) -> JobFileRelationshipModel {
        JobFileRelationshipModel {
            file_id,
            file_name,
            file_path,
            producer_job_id: None,
            producer_job_name: None,
            consumer_job_id: None,
            consumer_job_name: None,
            workflow_id,
        }
    }
}

// ListJobFileRelationshipsResponse - Response for listing job-file relationships
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListJobFileRelationshipsResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::JobFileRelationshipModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListJobFileRelationshipsResponse {
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListJobFileRelationshipsResponse {
        ListJobFileRelationshipsResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

// JobUserDataRelationshipModel - Represents a job-user_data relationship showing producer and consumer jobs
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct JobUserDataRelationshipModel {
    /// The user_data ID
    #[serde(rename = "user_data_id")]
    pub user_data_id: i64,

    /// The name of the user_data
    #[serde(rename = "user_data_name")]
    pub user_data_name: String,

    /// The job that produces this user_data (null for workflow inputs)
    #[serde(rename = "producer_job_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_id: Option<i64>,

    /// The name of the job that produces this user_data
    #[serde(rename = "producer_job_name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_name: Option<String>,

    /// The job that consumes this user_data (null for workflow outputs)
    #[serde(rename = "consumer_job_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_id: Option<i64>,

    /// The name of the job that consumes this user_data
    #[serde(rename = "consumer_job_name")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_name: Option<String>,

    /// The workflow containing the user_data and jobs
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,
}

impl JobUserDataRelationshipModel {
    pub fn new(
        user_data_id: i64,
        user_data_name: String,
        workflow_id: i64,
    ) -> JobUserDataRelationshipModel {
        JobUserDataRelationshipModel {
            user_data_id,
            user_data_name,
            producer_job_id: None,
            producer_job_name: None,
            consumer_job_id: None,
            consumer_job_name: None,
            workflow_id,
        }
    }
}

// ListJobUserDataRelationshipsResponse - Response for listing job-user_data relationships
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListJobUserDataRelationshipsResponse {
    #[serde(rename = "items")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<models::JobUserDataRelationshipModel>>,

    #[serde(rename = "offset")]
    pub offset: i64,

    #[serde(rename = "max_limit")]
    pub max_limit: i64,

    #[serde(rename = "count")]
    pub count: i64,

    #[serde(rename = "total_count")]
    pub total_count: i64,

    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListJobUserDataRelationshipsResponse {
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListJobUserDataRelationshipsResponse {
        ListJobUserDataRelationshipsResponse {
            items: None,
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

/// Represents a workflow action in the database
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct WorkflowActionModel {
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// Type of trigger (on_workflow_start, on_workflow_complete, on_jobs_ready, on_jobs_complete)
    #[serde(rename = "trigger_type")]
    pub trigger_type: String,

    /// Type of action (run_commands, schedule_nodes)
    #[serde(rename = "action_type")]
    pub action_type: String,

    /// JSON configuration for the action
    #[serde(rename = "action_config")]
    pub action_config: serde_json::Value,

    /// Array of job IDs that this action applies to
    #[serde(rename = "job_ids")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_ids: Option<Vec<i64>>,

    /// Number of times this action has been triggered (counter)
    #[serde(rename = "trigger_count")]
    #[serde(default)]
    pub trigger_count: i64,

    /// Number of triggers required before action is ready for execution
    #[serde(rename = "required_triggers")]
    #[serde(default = "default_required_triggers")]
    pub required_triggers: i64,

    /// Whether the action has been executed
    #[serde(rename = "executed")]
    #[serde(default)]
    pub executed: bool,

    /// Timestamp when the action was executed
    #[serde(rename = "executed_at")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<String>,

    /// ID of the compute node that executed the action
    #[serde(rename = "executed_by")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_by: Option<i64>,

    /// Whether the action persists and can be claimed by multiple workers
    #[serde(rename = "persistent")]
    #[serde(default)]
    pub persistent: bool,

    /// Whether the action was created during recovery (e.g., by `torc slurm regenerate`)
    /// Recovery actions are ephemeral and deleted when the workflow is reinitialized
    #[serde(rename = "is_recovery")]
    #[serde(default)]
    pub is_recovery: bool,
}

impl WorkflowActionModel {
    #[allow(clippy::new_without_default)]
    pub fn new(
        workflow_id: i64,
        trigger_type: String,
        action_type: String,
        action_config: serde_json::Value,
    ) -> WorkflowActionModel {
        WorkflowActionModel {
            id: None,
            workflow_id,
            trigger_type,
            action_type,
            action_config,
            job_ids: None,
            trigger_count: 0,
            required_triggers: 1, // Default for simple triggers
            executed: false,
            executed_at: None,
            executed_by: None,
            persistent: false,
            is_recovery: false,
        }
    }
}

/// Default value for required_triggers field
fn default_required_triggers() -> i64 {
    1
}

/// Data model for remote workers associated with a workflow.
/// Remote workers are immutable after creation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct RemoteWorkerModel {
    /// Worker identifier (format: [user@]hostname[:port])
    #[serde(rename = "worker")]
    pub worker: String,

    /// Database ID of the workflow this worker is associated with
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,
}

impl RemoteWorkerModel {
    #[allow(clippy::new_without_default)]
    pub fn new(worker: String, workflow_id: i64) -> RemoteWorkerModel {
        RemoteWorkerModel {
            worker,
            workflow_id,
        }
    }
}

/// Response model for reset_job_status endpoint.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ResetJobStatusResponse {
    /// The workflow ID for which jobs were reset
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// The number of jobs that were updated
    #[serde(rename = "updated_count")]
    pub updated_count: i64,

    /// The status that jobs were reset to
    #[serde(rename = "status")]
    pub status: String,

    /// The type of reset performed (e.g., "all" or "failed_only")
    #[serde(rename = "reset_type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_type: Option<String>,
}

impl ResetJobStatusResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64, updated_count: i64, status: String) -> ResetJobStatusResponse {
        ResetJobStatusResponse {
            workflow_id,
            updated_count,
            status,
            reset_type: None,
        }
    }

    pub fn with_reset_type(mut self, reset_type: String) -> Self {
        self.reset_type = Some(reset_type);
        self
    }
}

/// Response model for list_job_ids endpoint.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListJobIdsResponse {
    /// List of job IDs in the workflow
    #[serde(rename = "job_ids")]
    pub job_ids: Vec<i64>,

    /// The number of job IDs returned
    #[serde(rename = "count")]
    pub count: i64,
}

impl ListJobIdsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(job_ids: Vec<i64>) -> ListJobIdsResponse {
        let count = job_ids.len() as i64;
        ListJobIdsResponse { job_ids, count }
    }
}

// ============================================================================
// Access Groups Models for team-based access control
// ============================================================================

/// Access group model - represents a team/group for access control
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct AccessGroupModel {
    /// Database ID
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Name of the group (unique)
    #[serde(rename = "name")]
    pub name: String,

    /// Description of the group
    #[serde(rename = "description")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Timestamp when the group was created
    #[serde(rename = "created_at")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl AccessGroupModel {
    #[allow(clippy::new_without_default)]
    pub fn new(name: String) -> AccessGroupModel {
        AccessGroupModel {
            id: None,
            name,
            description: None,
            created_at: None,
        }
    }
}

/// User group membership model - links users to groups
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct UserGroupMembershipModel {
    /// Database ID
    #[serde(rename = "id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,

    /// Username of the member
    #[serde(rename = "user_name")]
    pub user_name: String,

    /// ID of the group
    #[serde(rename = "group_id")]
    pub group_id: i64,

    /// Role in the group (admin or member)
    #[serde(rename = "role")]
    #[serde(default = "default_membership_role")]
    pub role: String,

    /// Timestamp when the membership was created
    #[serde(rename = "created_at")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

fn default_membership_role() -> String {
    "member".to_string()
}

impl UserGroupMembershipModel {
    #[allow(clippy::new_without_default)]
    pub fn new(user_name: String, group_id: i64) -> UserGroupMembershipModel {
        UserGroupMembershipModel {
            id: None,
            user_name,
            group_id,
            role: "member".to_string(),
            created_at: None,
        }
    }
}

/// Workflow access group model - links workflows to groups for shared access
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct WorkflowAccessGroupModel {
    /// ID of the workflow
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    /// ID of the group
    #[serde(rename = "group_id")]
    pub group_id: i64,

    /// Timestamp when the association was created
    #[serde(rename = "created_at")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl WorkflowAccessGroupModel {
    #[allow(clippy::new_without_default)]
    pub fn new(workflow_id: i64, group_id: i64) -> WorkflowAccessGroupModel {
        WorkflowAccessGroupModel {
            workflow_id,
            group_id,
            created_at: None,
        }
    }
}

/// Response for listing access groups
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListAccessGroupsResponse {
    /// List of access groups
    #[serde(rename = "items")]
    pub items: Vec<AccessGroupModel>,

    /// Offset used for pagination
    #[serde(rename = "offset")]
    pub offset: i64,

    /// Limit used for pagination
    #[serde(rename = "limit")]
    pub limit: i64,

    /// Total count of records
    #[serde(rename = "total_count")]
    pub total_count: i64,

    /// Whether there are more records
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListAccessGroupsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(items: Vec<AccessGroupModel>, offset: i64, limit: i64, total_count: i64) -> Self {
        let has_more = offset + (items.len() as i64) < total_count;
        ListAccessGroupsResponse {
            items,
            offset,
            limit,
            total_count,
            has_more,
        }
    }
}

/// Response for listing user group memberships
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct ListUserGroupMembershipsResponse {
    /// List of memberships
    #[serde(rename = "items")]
    pub items: Vec<UserGroupMembershipModel>,

    /// Offset used for pagination
    #[serde(rename = "offset")]
    pub offset: i64,

    /// Limit used for pagination
    #[serde(rename = "limit")]
    pub limit: i64,

    /// Total count of records
    #[serde(rename = "total_count")]
    pub total_count: i64,

    /// Whether there are more records
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl ListUserGroupMembershipsResponse {
    #[allow(clippy::new_without_default)]
    pub fn new(
        items: Vec<UserGroupMembershipModel>,
        offset: i64,
        limit: i64,
        total_count: i64,
    ) -> Self {
        let has_more = offset + (items.len() as i64) < total_count;
        ListUserGroupMembershipsResponse {
            items,
            offset,
            limit,
            total_count,
            has_more,
        }
    }
}

/// Response for access check
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, validator::Validate)]
#[cfg_attr(feature = "conversion", derive(frunk::LabelledGeneric))]
pub struct AccessCheckResponse {
    /// Whether the user has access
    #[serde(rename = "has_access")]
    pub has_access: bool,

    /// The user name that was checked
    #[serde(rename = "user_name")]
    pub user_name: String,

    /// The workflow ID that was checked
    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,
}
