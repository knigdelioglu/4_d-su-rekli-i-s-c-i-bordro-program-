use crate::domain::models::{BordroDonemi, SickLeaveRecord};
use crate::domain::Result;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use std::collections::BTreeMap;

pub struct SickLeaveService;

impl SickLeaveService {
    /// Calculates the number of institution-paid sick days for a personnel in a given payroll period.
    ///
    /// Rules:
    /// - Sick leave records are grouped by the calendar year of their `startDate`.
    /// - Each record represents a distinct sick leave occurrence (episode: 1st, 2nd, ... in that calendar year).
    /// - For the first 5 episodes starting in a calendar year, up to the first 2 days of each episode are paid by the institution globally.
    /// - For the 6th and subsequent episodes starting in a calendar year, 0 days are paid.
    /// - For episodes spanning across calendar years or payroll periods, the global payable days (first 2 days of the episode)
    ///   are counted in whichever payroll period range (`baslangicTarihi`..=`bitisTarihi`) they fall into.
    pub fn calculate_paid_sick_days_for_period(
        conn: &Connection,
        personnel_id: &str,
        period: &BordroDonemi,
    ) -> Result<i32> {
        PeriodRepository::validate_period(period)?;
        let all_records = SickLeaveRepository::get_by_personnel(conn, personnel_id)?;
        Ok(Self::calculate_paid_sick_days_from_records(
            &all_records,
            period,
        ))
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

        // Group valid records by their start_date's calendar year
        let mut year_groups: BTreeMap<i32, Vec<(NaiveDate, NaiveDate)>> = BTreeMap::new();

        for r in records {
            if let (Ok(start), Ok(end)) = (
                NaiveDate::parse_from_str(&r.startDate, "%Y-%m-%d"),
                NaiveDate::parse_from_str(&r.endDate, "%Y-%m-%d"),
            ) {
                if end >= start {
                    year_groups
                        .entry(start.year())
                        .or_default()
                        .push((start, end));
                }
            }
        }

        let mut total_paid_in_period = 0;

        for (_year, mut recs) in year_groups {
            recs.sort_by(|a, b| a.0.cmp(&b.0));

            for (idx, (start, end)) in recs.iter().enumerate() {
                let episode_index = idx + 1; // 1-indexed (1st, 2nd, ... episode of this calendar year)

                if episode_index <= 5 {
                    // Global Day 1 of episode: start_date
                    if *start >= period_start && *start <= period_end {
                        total_paid_in_period += 1;
                    }

                    // Global Day 2 of episode: start_date + 1 day (if episode length > 1 day)
                    if *end > *start {
                        if let Some(day2) = start.succ_opt() {
                            if day2 >= period_start && day2 <= period_end {
                                total_paid_in_period += 1;
                            }
                        }
                    }
                }
            }
        }

        total_paid_in_period
    }
}
