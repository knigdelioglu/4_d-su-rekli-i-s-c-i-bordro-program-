//! Portable payroll domain.
//!
//! Portable payroll domain.
//!
//! The model and formula modules live in this crate, which has no Tauri,
//! SQLite, filesystem, or process-global dependencies. `payroll_engine` owns
//! the pure snapshot calculation boundary used by both native and browser
//! runtimes.

pub mod calculations;
pub mod errors;
pub mod models;

pub use errors::DomainError;

pub type Result<T> = std::result::Result<T, DomainError>;

pub mod gv_exemption;
pub mod index;
pub mod notices;
pub mod payroll_engine;
pub mod policies;
pub mod retro;

pub use calculations::*;
pub use index::*;
pub use models::*;
pub use notices::*;
pub use payroll_engine::*;
pub use policies::*;
pub use retro::*;
