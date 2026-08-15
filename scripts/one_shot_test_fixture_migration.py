from pathlib import Path
import re

TEST_PATH = Path("src-tauri/tests/domain_tests.rs")
SELF_PATH = Path(__file__)

s = TEST_PATH.read_text()


def test_block(source: str, test_name: str) -> tuple[int, int, str]:
    needle = f"fn {test_name}"
    fn_pos = source.index(needle)
    start = source.rfind("    #[test]", 0, fn_pos)
    if start < 0:
        raise RuntimeError(f"test marker not found: {test_name}")
    end = source.find("\n    #[test]", fn_pos + len(needle))
    if end < 0:
        end = source.rfind("\n}")
    return start, end, source[start:end]


def replace_in_test(source: str, test_name: str, old: str, new: str, count: int = 1) -> str:
    start, end, block = test_block(source, test_name)
    actual = block.count(old)
    if actual < count:
        raise RuntimeError(
            f"{test_name}: expected at least {count} occurrence(s), found {actual}: {old[:80]!r}"
        )
    block = block.replace(old, new, count)
    return source[:start] + block + source[end:]


helper_marker = "    #[test]\n    fn test_cumulative_gv_regression_and_collision()"
helper = '''    fn work_days_for_period_id(period_id: &str, day_count: i64) -> HashMap<String, String> {
        let mut parts = period_id.split('-');
        let year: i32 = parts
            .next()
            .expect("test period year")
            .parse()
            .expect("numeric test period year");
        let month: u32 = parts
            .next()
            .expect("test period month")
            .parse()
            .expect("numeric test period month");
        let start = chrono::NaiveDate::from_ymd_opt(year, month, 15)
            .expect("valid 15-14 test period start");
        let (end_year, end_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let end = chrono::NaiveDate::from_ymd_opt(end_year, end_month, 14)
            .expect("valid 15-14 test period end");
        let available = (end - start).num_days() + 1;
        let count = day_count.clamp(0, available);
        if count == 0 {
            return HashMap::new();
        }
        let first = end - chrono::Duration::days(count - 1);
        (0..count)
            .map(|offset| {
                (
                    (first + chrono::Duration::days(offset))
                        .format("%Y-%m-%d")
                        .to_string(),
                    "Ç".to_string(),
                )
            })
            .collect()
    }

    fn ensure_previous_period_settings(
        conn: &rusqlite::Connection,
        current: &BordroDonemi,
        current_settings: &DonemselKurumDegerleri,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (year, month) = if current.ay == 1 {
            (current.yil - 1, 12)
        } else {
            (current.yil, current.ay - 1)
        };
        let previous_id = format!("{year:04}-{month:02}");
        if PeriodRepository::get_by_id(conn, &previous_id)?.is_none() {
            let (end_year, end_month) = if month == 12 {
                (year + 1, 1)
            } else {
                (year, month + 1)
            };
            let previous = BordroDonemi {
                id: previous_id.clone(),
                yil: year,
                ay: month,
                baslangicTarihi: format!("{year:04}-{month:02}-15"),
                bitisTarihi: format!("{end_year:04}-{end_month:02}-14"),
                donemAdi: format!("Önceki test dönemi {previous_id}"),
                taxYear: year,
                taxMonth: month,
            };
            PeriodRepository::save(conn, &previous)?;
        }
        let mut previous_settings = current_settings.clone();
        previous_settings.donemId = previous_id;
        SettingsRepository::save_institution_settings(conn, &previous_settings)?;
        Ok(())
    }

'''
if "fn work_days_for_period_id(" not in s:
    if helper_marker not in s:
        raise RuntimeError("helper insertion marker not found")
    s = s.replace(helper_marker, helper + helper_marker, 1)

# A/B/D and equivalent legacy fixtures: calendar day 1..30 is invalid for a 15..14 period.
for ym in ("2026-05", "2026-06"):
    for indent in ("        ", "            "):
        old = (
            f'{indent}let mut gunler = HashMap::new();\n'
            f'{indent}for d in 1..=30 {{\n'
            f'{indent}    gunler.insert(format!("{ym}-{{:02}}", d), "Ç".to_string());\n'
            f'{indent}}}'
        )
        new = f'{indent}let gunler = work_days_for_period_id("{ym}", 30);'
        s = s.replace(old, new)

# Rapor persistence: retain May 25-30 as R while the other 24 dates stay inside May15-Jun14.
rapor_old = '''            let mut gunler = HashMap::new();
            for d in 1..=24 {
                gunler.insert(format!("2026-05-{:02}", d), "Ç".to_string());
            }
            for d in 25..=30 {
                gunler.insert(format!("2026-05-{:02}", d), "R".to_string());
            }'''
rapor_new = '''            let mut gunler = work_days_for_period_id("2026-05", 30);
            for d in 25..=30 {
                gunler.insert(format!("2026-05-{:02}", d), "R".to_string());
            }'''
if rapor_old not in s:
    raise RuntimeError("rapor persistence legacy fixture not found")
s = s.replace(rapor_old, rapor_new, 1)

# Remove fake day_N maps used by PEK carry-over tests.
fake_map = re.compile(
    r'\n(?P<i>\s*)let mut gunler_30 = HashMap::new\(\);\n'
    r'(?P=i)for d in 1\.\.=30 \{\n'
    r'(?P=i)    gunler_30\.insert\(format!\("day_\{\}", d\), "Ç"\.to_string\(\)\);\n'
    r'(?P=i)\}\n'
)
s, removed_fake_maps = fake_map.subn("\n", s)
if removed_fake_maps != 5:
    raise RuntimeError(f"expected 5 fake day_N map definitions, removed {removed_fake_maps}")

field_pattern = re.compile(
    r'(?P<prefix>donemId:\s*(?P<expr>[^,\n]+),\n(?P<indent>\s*))'
    r'gunler:\s*gunler_30\.clone\(\),'
)


def replace_fake_field(match: re.Match) -> str:
    expr = match.group("expr").strip()
    literal = re.fullmatch(r'"([0-9]{4}-[0-9]{2})"\.into\(\)', expr)
    if literal:
        days = f'work_days_for_period_id("{literal.group(1)}", 30)'
    else:
        var = re.fullmatch(r'([A-Za-z_][A-Za-z0-9_]*)\.id\.clone\(\)', expr)
        if not var:
            raise RuntimeError(f"unsupported legacy donemId expression: {expr}")
        days = f'work_days_for_period_id(&{var.group(1)}.id, 30)'
    return match.group("prefix") + f'gunler: {days},'


s, fake_fields = field_pattern.subn(replace_fake_field, s)
if fake_fields < 7:
    raise RuntimeError(f"expected at least 7 fake-date struct fields, replaced {fake_fields}")
if "gunler_30" in s or 'format!("day_{}"' in s:
    raise RuntimeError("legacy day_N fixture remained after migration")

# Test C's old setup relied on two duplicate work/tax periods, which is now forbidden by design.
c_start, c_end, _ = test_block(s, "test_c_attendance_bound_to_period_id_not_date_range")
c_block = '''    #[test]
    fn test_c_duplicate_work_period_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let donem_a = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
            taxYear: 2026,
            taxMonth: 6,
        };
        let donem_b = BordroDonemi {
            id: "2026-05-alt".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026 (kopya)".into(),
            taxYear: 2026,
            taxMonth: 6,
        };

        PeriodRepository::save(&conn, &donem_a)?;
        let duplicate = PeriodRepository::save(&conn, &donem_b);
        assert!(
            matches!(duplicate, Err(DomainError::ValidationError(msg)) if msg.contains("çalışma dönemi")),
            "aynı çalışma dönemi farklı ID ile ikinci kez oluşturulmamalı"
        );
        assert!(PeriodRepository::get_by_id(&conn, "2026-05-alt")?.is_none());

        Ok(())
    }
'''
s = s[:c_start] + c_block + s[c_end:]

# The year-crossing sick-leave persistence test now needs a prior period threshold snapshot.
sick_name = "test_year_crossing_sick_leave_persistence_and_finalized_reload"
sick_old = '            ensure_test_institution_settings(&conn, &["2026-12"])?;'
sick_new = sick_old + '''
            let current_settings = SettingsRepository::get_institution_settings(&conn, "2026-12")?
                .expect("2026-12 test settings");
            ensure_previous_period_settings(&conn, &donem, &current_settings)?;'''
s = replace_in_test(s, sick_name, sick_old, sick_new)

# PEK year-transition tests need the previous work period's threshold snapshot too.
for test_name, person_id in (
    ("test_devreden_pek_aralik_ocak_gecisi", "p-pek-a"),
    ("test_devreden_pek_yeni_yil_ocak_tavani", "p-pek-c"),
    ("test_devreden_pek_tax_year_bagimsizligi", "p-pek-e"),
):
    marker = f'        PayrollService::calculate_payroll_for_personnel(&conn, "{person_id}", "2026-12")?;'
    replacement = f'''        let current_settings = SettingsRepository::get_institution_settings(&conn, "2026-12")?
            .expect("2026-12 PEK test settings");
        ensure_previous_period_settings(&conn, &aralik2026, &current_settings)?;

{marker}'''
    s = replace_in_test(s, test_name, marker, replacement)

# Strict post-conditions: legacy invalid fixture patterns must be gone.
if 'for d in 1..=30 {\n            gunler.insert(format!("2026-05-{:02}"' in s:
    raise RuntimeError("invalid May 1..30 fixture remains")
if 'for d in 1..=30 {\n                gunler.insert(format!("2026-06-{:02}"' in s:
    raise RuntimeError("invalid June 1..30 fixture remains")
if "test_c_attendance_bound_to_period_id_not_date_range" in s:
    raise RuntimeError("old duplicate-period Test C remains")

TEST_PATH.write_text(s)
SELF_PATH.unlink()
