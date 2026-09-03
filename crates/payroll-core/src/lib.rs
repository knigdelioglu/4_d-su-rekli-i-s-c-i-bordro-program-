//! Portable payroll domain.
//!
//! The model and formula modules are intentionally shared with the native
//! application while this crate itself has no Tauri, SQLite, filesystem, or
//! process-global dependencies. `payroll_engine` owns the pure snapshot
//! calculation boundary used by both native and browser runtimes.

#[path = "../../../src-tauri/src/domain/calculations.rs"]
pub mod calculations;
#[path = "../../../src-tauri/src/domain/errors.rs"]
pub mod errors;
#[path = "../../../src-tauri/src/domain/models.rs"]
pub mod models;

pub use errors::DomainError;

pub type Result<T> = std::result::Result<T, DomainError>;

pub mod notices;
pub mod payroll_engine;
pub mod policies;

pub use calculations::*;
pub use models::*;
pub use notices::*;
pub use payroll_engine::*;
pub use policies::*;
