use crate::domain::models::{BordroDonemi, SickLeaveRecord};
use crate::domain::Result;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;

pub struct SickLeaveService;

impl SickLeaveService {
    /// Calculates the number of institution-paid sick days for a personnel in a given payroll period.
    ///
    /// Rules:
    /// - For the calendar year of the payroll period, sick leave records for the personnel
    ///   are sorted chronologically by `startDate`.
    /// - Each record represents a distinct sick leave occurrence (episode: 1st, 2nd, ...).
    /// - For the first 5 episodes in the calendar year, up to the first 2 days of each episode are paid by the institution.
    /// - For the 6th and subsequent episodes in the calendar year, 0 days are paid.
    /// - If an episode spans across two payroll periods (e.g. 15th-14th boundary),
    ///   the first 2 payable days are counted only once across periods.
    pub fn calculate_paid_sick_days_for_period(
        conn: &Connection,
        personnel_id: &str,
        period: &BordroDonemi,
    ) -> Result<i32> {
        let all_records = SickLeaveRepository::get_by_personnel(conn, personnel_id)?;
        Ok(Self::calculate_paid_sick_days_from_records(&all_records, period))
    }

    pub fn calculate_paid_sick_days_from_records(
        records: &[SickLeaveRecord],
        period: &BordroDonemi,
    ) -> i32 {
        let period_start = match NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => return 0,
        };
        let period_end = match NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => return 0,
        };

        // Filter and sort records for the calendar year
        // We consider records whose start_date is in period.yil, or whose date range overlaps with period.yil
        let mut year_records: Vec<&SickLeaveRecord> = records
            .iter()
            .filter(|r| {
                if let Ok(start) = NaiveDate::parse_from_str(&r.startDate, "%Y-%m-%d") {
                    start.year() == period.yil
                } else {
                    false
                }
            })
            .collect();

        year_records.sort_by(|a, b| a.startDate.cmp(&b.startDate));

        let mut total_paid_in_period = 0;

        for (idx, record) in year_records.iter().enumerate() {
            let episode_index = idx + 1; // 1-indexed (1st, 2nd, ... episode)
            let start = match NaiveDate::parse_from_str(&record.startDate, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };
            let end = match NaiveDate::parse_from_str(&record.endDate, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };

            if end < start {
                continue;
            }

            let mut curr = start;
            let mut day_index_in_episode = 1; // 1st day, 2nd day, 3rd day of this episode

            while curr <= end {
                let is_day_paid_by_institution = episode_index <= 5 && day_index_in_episode <= 2;
                let is_day_in_period = curr >= period_start && curr <= period_end;

                if is_day_paid_by_institution && is_day_in_period {
                    total_paid_in_period += 1;
                }

                if let Some(next_day) = curr.succ_opt() {
                    curr = next_day;
                } else {
                    break;
                }
                day_index_in_episode += 1;
            }
        }

        total_paid_in_period
    }
}
