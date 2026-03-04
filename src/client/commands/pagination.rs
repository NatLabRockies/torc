//! Pagination utilities with both lazy iteration and vector collection support.
//!
//! This module provides two ways to work with paginated API responses:
//! 1. **Lazy iteration** using `iter_*` functions - memory efficient, items are fetched on-demand
//! 2. **Vector collection** using `paginate_*` functions - collects all items into a Vec
//!
//! # Generic Framework
//!
//! The `base` module provides a trait-based generic pagination framework.
//! New resource types can implement the `Paginatable` trait to get
//! automatic pagination support.
//!
//! # Simple Iteration APIs
//!
//! The main APIs provide simple, clean interfaces for iterating over jobs, files, events, results, user data, resource requirements, and workflows:
//!

pub mod base;
pub mod compute_nodes;
pub mod events;
pub mod files;
pub mod jobs;
pub mod resource_requirements;
pub mod results;
pub mod ro_crate_entities;
pub mod scheduled_compute_nodes;
pub mod slurm_schedulers;
pub mod user_data;
pub mod workflows;

// Re-export all parameter types and iterator types and functions
pub use compute_nodes::*;
pub use events::*;
pub use files::*;
pub use jobs::*;
pub use resource_requirements::*;
pub use results::*;
pub use ro_crate_entities::*;
pub use scheduled_compute_nodes::*;
pub use slurm_schedulers::*;
pub use user_data::*;
pub use workflows::*;
