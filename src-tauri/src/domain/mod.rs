//! Compatibility façade for native callers.
//!
//! Domain models, errors, and calculations are owned by `payroll-core`; the
//! Tauri crate re-exports them here so existing repository/service imports do
//! not become a second model representation.
pub use payroll_core::calculations;
pub use payroll_core::errors;
pub use payroll_core::models;

pub use payroll_core::calculations::*;
pub use payroll_core::errors::*;
pub use payroll_core::models::*;
