from pathlib import Path


def replace_once(text: str, old: str, new: str) -> str:
    if old in text:
        return text.replace(old, new, 1)
    return text


domain = Path("src-tauri/tests/domain_tests.rs")
text = domain.read_text()

text = replace_once(
    text,
    "        // AYNI tarih aralığına sahip iki ayrı period ID (kopya dönem senaryosu).\n",
    "        // İki ayrı geçerli çalışma dönemi: puantaj yalnız period_id üzerinden çözülmeli.\n",
)

text = replace_once(
    text,
    '''        let donem_b = BordroDonemi {
            id: "2026-05-alt".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026 (kopya)".into(),
            taxYear: 2026,
            taxMonth: 5,
        };
''',
    '''        let donem_b = BordroDonemi {
            id: "2026-06".into(),
            yil: 2026,
            ay: 6,
            baslangicTarihi: "2026-06-15".into(),
            bitisTarihi: "2026-07-14".into(),
            donemAdi: "Haziran 2026".into(),
            taxYear: 2026,
            taxMonth: 7,
        };
''',
)

text = replace_once(
    text,
    'ensure_test_institution_settings(&conn, &["2026-05", "2026-05-alt"])?;',
    'ensure_test_institution_settings(&conn, &["2026-05", "2026-06"])?;',
)
text = replace_once(
    text,
    "// Puantaj yalnız A'ya kaydedildi; B aynı tarih aralığını paylaşsa bile puantajsız.",
    "// Puantaj yalnız A'ya kaydedildi; B kendi period_id'si için puantajsız.",
)
text = replace_once(
    text,
    "// B için hesaplama, tarih aralığı aynı olsa bile puantaj bulamadığı için başarısız olmalı.",
    "// B için hesaplama kendi period_id'sine ait puantaj olmadığı için başarısız olmalı.",
)
text = replace_once(text, '"test-att-c", "2026-05-alt"', '"test-att-c", "2026-06"')

old_gv_period = '''        let donem = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-01".into(),
            bitisTarihi: "2026-01-31".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
'''
new_gv_period = '''        let donem = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-15".into(),
            bitisTarihi: "2026-02-14".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
'''
marker = "fn test_gv_snapshot_finalized_persistence()"
idx = text.find(marker)
if idx != -1:
    tail = text[idx:]
    tail = replace_once(tail, old_gv_period, new_gv_period)
    text = text[:idx] + tail

domain.write_text(text)

sick = Path("src-tauri/src/repositories/sick_leave_repo.rs")
text = sick.read_text()
text = replace_once(
    text,
    ".map_or(true, |old| old.personnelId != record.personnelId)",
    ".is_none_or(|old| old.personnelId != record.personnelId)",
)
sick.write_text(text)

preflight = Path("src-tauri/src/services/payroll_preflight_service.rs")
text = preflight.read_text()
old = '''        match expected_previous_period_id {
            None => {
                return Err(DomainError::ValidationError(format!(
                    "{} personelinde {} döneminden gelen devreden PEK hâlâ geçerli fakat {}-{:02} ara çalışma dönemi oluşturulmamış. PEK süresini yanlış taşımamak için eksik dönemi tamamlayın.",
                    personnel_id, source_period_id, expected_previous_year, expected_previous_month
                )));
            }
            Some(previous_period_id) => {
                let previous_payroll_exists: i64 = conn
                    .query_row(
                        "SELECT EXISTS(
                            SELECT 1 FROM payroll_records
                            WHERE personnel_id = ?1 AND period_id = ?2
                         )",
                        params![personnel_id, previous_period_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
                if previous_payroll_exists != 0 {
                    return Ok(());
                }

                return Err(DomainError::ValidationError(format!(
                    "{} personelinde {} döneminden gelen devreden PEK hâlâ geçerli, ancak aradaki {} dönemi için bordro yok. Devreden PEK'in sessizce kaybolmaması için önce ara dönem bordrosunu tamamlayın.",
                    personnel_id, source_period_id, previous_period_id
                )));
            }
        }
'''
new = '''        match expected_previous_period_id {
            None => Err(DomainError::ValidationError(format!(
                "{} personelinde {} döneminden gelen devreden PEK hâlâ geçerli fakat {}-{:02} ara çalışma dönemi oluşturulmamış. PEK süresini yanlış taşımamak için eksik dönemi tamamlayın.",
                personnel_id, source_period_id, expected_previous_year, expected_previous_month
            ))),
            Some(previous_period_id) => {
                let previous_payroll_exists: i64 = conn
                    .query_row(
                        "SELECT EXISTS(
                            SELECT 1 FROM payroll_records
                            WHERE personnel_id = ?1 AND period_id = ?2
                         )",
                        params![personnel_id, previous_period_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
                if previous_payroll_exists != 0 {
                    return Ok(());
                }

                Err(DomainError::ValidationError(format!(
                    "{} personelinde {} döneminden gelen devreden PEK hâlâ geçerli, ancak aradaki {} dönemi için bordro yok. Devreden PEK'in sessizce kaybolmaması için önce ara dönem bordrosunu tamamlayın.",
                    personnel_id, source_period_id, previous_period_id
                )))
            }
        }
'''
text = replace_once(text, old, new)
preflight.write_text(text)
