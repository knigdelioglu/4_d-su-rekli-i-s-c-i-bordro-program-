from pathlib import Path

path = Path("src-tauri/tests/domain_tests.rs")
text = path.read_text()

old = '''    fn thirty_work_days(period: &BordroDonemi) -> HashMap<String, String> {
        let start = chrono::NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .expect("test period start date must be valid");
        let end = chrono::NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")
            .expect("test period end date must be valid");

        (0..30)
            .map(|offset| {
                let date = start + chrono::Duration::days(offset);
                assert!(
                    date <= end,
                    "generated attendance date {date} must stay inside period {}",
                    period.id
                );
                (date.format("%Y-%m-%d").to_string(), "Ç".to_string())
            })
            .collect()
    }
'''

new = '''    fn thirty_work_days(period: &BordroDonemi) -> HashMap<String, String> {
        let start = chrono::NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .expect("test period start date must be valid");
        let end = chrono::NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")
            .expect("test period end date must be valid");
        let period_day_count = (end - start).num_days() + 1;
        let work_day_count = period_day_count.min(30);

        (0..work_day_count)
            .map(|offset| {
                let date = start + chrono::Duration::days(offset);
                (date.format("%Y-%m-%d").to_string(), "Ç".to_string())
            })
            .collect()
    }
'''

count = text.count(old)
if count != 1:
    raise SystemExit(f"expected exactly one helper block, found {count}")

path.write_text(text.replace(old, new, 1))
