//! HPC (High Performance Computing) management functionality
//!
//! This module provides abstractions for working with HPC schedulers like Slurm.
//! It includes traits for HPC interfaces and concrete implementations for different
//! scheduler types.
//!
//! It also provides HPC system profiles for known HPC systems (like NLR Kestrel)
//! that include partition configurations, resource limits, and auto-detection.

pub mod common;
mod dane;
pub mod hpc_interface;
pub mod kestrel;
pub mod profiles;
pub mod slurm;
pub mod slurm_interface;

pub use common::{HpcJobInfo, HpcJobStats, HpcJobStatus};
pub use hpc_interface::HpcInterface;
pub use profiles::{HpcDetection, HpcPartition, HpcProfile, HpcProfileRegistry};
pub use slurm_interface::SlurmInterface;
