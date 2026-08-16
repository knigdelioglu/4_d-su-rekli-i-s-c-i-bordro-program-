use crate::domain::models::{BordroDonemi, SickLeaveRecord};
use crate::domain::Result;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};

pub struct SickLeaveService;

impl SickLeaveService {
    /// Returns the exact institution-paid sick-leave dates that fall into a payroll period.
    ///
    /// Rules:
    /// - Sick leave records are grouped by the calendar year of their `startDate`.
    /// - Each record represents a distinct sick leave occurrence (episode: 1st, 2nd, ... in that calendar year).
    /// - For the first 5 episodes starting in a calendar year, up to the first 2 days of each episode are paid by the institution globally.
    /// - For the 6th and subsequent episodes starting in a calendar year, 0 days are paid.
    /// - For episodes spanning across calendar years or payroll periods, the global payable dates remain attached to the episode start
    ///   and are returned only by the payroll period whose date range contains them.
    /// - Duplicate/overlapping records can never make the same calendar date payable twice.
    pub fn calculate_paid_sick_dates_for_period(
        conn: &Connection,
        personnel_id: &str,
        period: &BordroDonemi,
    ) -> Result<Vec<NaiveDate>> {
        PeriodRepository::validate_period(period)?;
        let all_records = SickLeaveRepository::get_by_personnel(conn, personnel_id)?;
        Ok(Self::calculate_paid_sick_dates_from_records(
            &all_records,
            period,
        ))
    }

    /// Calculates the number of institution-paid sick days for a personnel in a given payroll period.
    /// The count is derived from the exact payable-date calculation so payroll amount and displayed day count
    /// cannot drift apart.
    pub fn calculate_paid_sick_days_for_period(
        conn: &Connection,
        personnel_id: &str,
        period: &BordroDonemi,
    ) -> Result<i32> {
        Ok(Self::calculate_paid_sick_dates_for_period(conn, personnel_id, period)?.len() as i32)
    }

    pub fn calculate_paid_sick_dates_from_records(
        records: &[SickLeaveRecord],
        period: &BordroDonemi,
    ) -> Vec<NaiveDate> {
        let period_start = match NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        let period_end = match NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };

        // Group valid records by their start_date's calendar year.
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

        let mut paid_dates = BTreeSet::new();

        for (_year, mut recs) in year_groups {
            recs.sort_by_key(|a| a.0);

            for (idx, (start, end)) in recs.iter().enumerate() {
                let episode_index = idx + 1; // 1-indexed (1st, 2nd, ... episode of this calendar year)
                if episode_index > 5 {
                    continue;
                }

                // Global Day 1 of episode: start_date.
                if *start >= period_start && *start <= period_end {
                    paid_dates.insert(*start);
                }

                // Global Day 2 of episode: start_date + 1 day, only when the episode actually has a second day.
                if *end > *start {
                    if let Some(day2) = start.succ_opt() {
                        if day2 <= *end && day2 >= period_start && day2 <= period_end {
                            paid_dates.insert(day2);
                        }
                    }
                }
            }
        }

        paid_dates.into_iter().collect()
    }

    pub fn calculate_paid_sick_days_from_records(
        records: &[SickLeaveRecord],
        period: &BordroDonemi,
    ) -> i32 {
        Self::calculate_paid_sick_dates_from_records(records, period).len() as i32
    }
}
