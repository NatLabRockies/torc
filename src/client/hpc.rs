//! HPC (High Performance Computing) management functionality
//!
//! This module provides abstractions for working with HPC schedulers like Slurm.
//! It includes traits for HPC interfaces and concrete implementations for different
//! scheduler types.
//!
//! It also provides HPC system profiles for known HPC systems (like NREL Kestrel)
//! that include partition configurations, resource limits, and auto-detection.

pub mod common;
pub mod dane;
pub mod hpc_interface;
pub mod hpc_manager;
pub mod kestrel;
pub mod profiles;
pub mod slurm_interface;

pub use common::{HpcJobInfo, HpcJobStats, HpcJobStatus, HpcType};
pub use hpc_interface::HpcInterface;
pub use hpc_manager::HpcManager;
pub use profiles::{HpcDetection, HpcPartition, HpcProfile, HpcProfileRegistry};
pub use slurm_interface::SlurmInterface;

use anyhow::Result;

/// Factory function to create an HPC interface based on the type
pub fn create_hpc_interface(hpc_type: HpcType) -> Result<Box<dyn HpcInterface>> {
    match hpc_type {
        HpcType::Slurm => Ok(Box::new(SlurmInterface::new()?)),
        HpcType::Pbs => Err(anyhow::anyhow!("PBS support not yet implemented")),
        HpcType::Fake => Err(anyhow::anyhow!("Fake HPC support not yet implemented")),
    }
}
