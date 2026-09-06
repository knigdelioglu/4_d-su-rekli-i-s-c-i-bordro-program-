pub mod commands;
pub mod db;
pub mod domain;
pub mod repositories;
pub mod services;

use db::{create_connection, DbState};
use std::sync::{Arc, Mutex};

pub fn run() {
    let conn = create_connection(None).expect("Failed to initialize SQLite database");
    let state: DbState = Arc::new(Mutex::new(conn));

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_personnel_list,
            commands::save_personnel,
            commands::delete_personnel,
            commands::get_tax_openings,
            commands::save_tax_opening,
            commands::get_periods,
            commands::save_period,
            commands::save_period_with_settings,
            commands::get_attendance_list,
            commands::save_attendance,
            commands::get_annual_payroll_parameters,
            commands::save_annual_payroll_parameters,
            commands::get_payroll_list,
            commands::get_payroll_notices,
            commands::calculate_payroll,
            commands::delete_payroll_accrual,
            commands::finalize_payroll,
            commands::evaluate_mutation_policy,
            commands::set_payroll_status,
            commands::get_institution_settings,
            commands::save_institution_settings,
            commands::get_app_setting,
            commands::set_app_setting,
            commands::check_legacy_migrated,
            commands::migrate_legacy_payload,
            commands::replace_backup_payload,
            commands::get_sick_leave_records,
            commands::save_sick_leave_record,
            commands::delete_sick_leave_record,
            commands::get_compensation_revisions,
            commands::get_compensation_revision_overrides,
            commands::get_retro_adjustment_batches,
            commands::get_retro_adjustment_allocations,
            commands::save_compensation_revision,
            commands::calculate_retro_preview,
            commands::save_retro_adjustment_batch,
            commands::create_retro_payment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
