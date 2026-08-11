//! Common definitions for HPC functionality

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents the status of an HPC job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HpcJobStatus {
    /// Status is unknown
    Unknown,
    /// No job found
    None,
    /// Job is queued/pending
    Queued,
    /// Job is currently running
    Running,
    /// Job has completed
    Complete,
}

impl std::fmt::Display for HpcJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HpcJobStatus::Unknown => write!(f, "unknown"),
            HpcJobStatus::None => write!(f, "none"),
            HpcJobStatus::Queued => write!(f, "queued"),
            HpcJobStatus::Running => write!(f, "running"),
            HpcJobStatus::Complete => write!(f, "complete"),
        }
    }
}

/// Defines the status of a job running to the HPC
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HpcJobInfo {
    /// Job ID from the scheduler
    pub job_id: String,
    /// Job name
    pub name: String,
    /// Current job status
    pub status: HpcJobStatus,
}

impl HpcJobInfo {
    pub(crate) fn new(job_id: String, name: String, status: HpcJobStatus) -> Self {
        Self {
            job_id,
            name,
            status,
        }
    }

    /// Create an empty job info for when no job is found
    pub(crate) fn none() -> Self {
        Self {
            job_id: String::new(),
            name: String::new(),
            status: HpcJobStatus::None,
        }
    }
}

/// Defines the stats for an HPC job
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HpcJobStats {
    /// HPC job ID
    pub(crate) hpc_job_id: String,
    /// Job name
    pub(crate) name: String,
    /// Job start time
    pub(crate) start: DateTime<Utc>,
    /// Job end time (if finished)
    pub(crate) end: Option<DateTime<Utc>>,
    /// Job state as a string
    pub(crate) state: String,
    /// Account used for the job
    pub(crate) account: String,
    /// Partition/queue name
    pub(crate) partition: String,
    /// Quality of Service
    pub(crate) qos: String,
}

/// HPC types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HpcType {
    /// PBS/Torque scheduler
    Pbs,
    /// Slurm scheduler
    Slurm,
    /// Fake/test scheduler
    Fake,
}

impl std::fmt::Display for HpcType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HpcType::Pbs => write!(f, "pbs"),
            HpcType::Slurm => write!(f, "slurm"),
            HpcType::Fake => write!(f, "fake"),
        }
    }
}

impl std::str::FromStr for HpcType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pbs" => Ok(HpcType::Pbs),
            "slurm" => Ok(HpcType::Slurm),
            "fake" => Ok(HpcType::Fake),
            _ => Err(anyhow::anyhow!("Unknown HPC type: {}", s)),
        }
    }
}
