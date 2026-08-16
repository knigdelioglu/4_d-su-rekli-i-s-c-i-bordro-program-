from pathlib import Path

path = Path('src-tauri/tests/native_smoke_test.rs')
text = path.read_text()

old = '''            // 4. Add Attendance (Puantaj) for May 2026 (30 days "Ç") & June 2026 (30 days "Ç")
            let mut mayis_gunler = HashMap::new();
            let mut haziran_gunler = HashMap::new();
            for day in 1..=30 {
                mayis_gunler.insert(format!("2026-05-{}", day), "Ç".to_string());
                haziran_gunler.insert(format!("2026-06-{}", day), "Ç".to_string());
            }
'''
new = '''            // 4. Add Attendance (Puantaj) for May 2026 (30 days "Ç") & June 2026 (30 days "Ç")
            // Attendance keys must be real calendar dates inside the exact 15-14 payroll period.
            let mayis_start = chrono::NaiveDate::parse_from_str(
                &mayis2026.baslangicTarihi,
                "%Y-%m-%d",
            )?;
            let haziran_start = chrono::NaiveDate::parse_from_str(
                &haziran2026.baslangicTarihi,
                "%Y-%m-%d",
            )?;
            let mut mayis_gunler = HashMap::new();
            let mut haziran_gunler = HashMap::new();
            for offset in 0..30 {
                let mayis_date = mayis_start + chrono::Duration::days(offset);
                let haziran_date = haziran_start + chrono::Duration::days(offset);
                mayis_gunler.insert(mayis_date.format("%Y-%m-%d").to_string(), "Ç".to_string());
                haziran_gunler.insert(
                    haziran_date.format("%Y-%m-%d").to_string(),
                    "Ç".to_string(),
                );
            }
'''

if text.count(old) != 1:
    raise SystemExit(f'expected exactly one old smoke attendance block, found {text.count(old)}')

path.write_text(text.replace(old, new, 1))
