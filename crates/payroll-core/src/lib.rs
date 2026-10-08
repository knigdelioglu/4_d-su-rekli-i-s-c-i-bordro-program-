//! Portable payroll domain.
//!
//! Portable payroll domain.
//!
//! The model and formula modules live in this crate, which has no Tauri,
//! SQLite, filesystem, or process-global dependencies. `payroll_engine` owns
//! the pure snapshot calculation boundary used by both native and browser
//! runtimes.

pub mod calculations;
pub mod compensation_policy;
pub mod errors;
pub mod models;
pub mod money;

pub use errors::DomainError;

pub type Result<T> = std::result::Result<T, DomainError>;

pub mod gv_exemption;
pub mod index;
pub mod notices;
pub mod payroll_engine;
pub mod payroll_snapshot;
pub mod policies;
pub(crate) mod raise_transition;
pub mod retro;
pub(crate) mod transport_rates;

pub use calculations::*;
pub use compensation_policy::*;
pub use index::*;
pub use models::*;
pub use money::*;
pub use notices::*;
pub use payroll_engine::*;
pub use payroll_snapshot::*;
pub use policies::*;
pub use retro::*;

pub mod validation;
pub use validation::*;
