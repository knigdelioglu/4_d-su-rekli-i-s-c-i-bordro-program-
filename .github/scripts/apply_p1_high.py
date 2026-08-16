from pathlib import Path
import re

ROOT = Path('.')


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding='utf-8')


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding='utf-8')


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f'{label}: expected exactly one match, found {count}')
    return text.replace(old, new, 1)


# ---------------------------------------------------------------------------
# 1) Domain models: period-local statutory segments + resolved snapshot
# ---------------------------------------------------------------------------
path = 'src-tauri/src/domain/models.rs'
text = read(path)
anchor = '''#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DonemselKurumDegerleri {'''
insert = '''#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatutoryParameterSegment {
    /// Inclusive effective date inside this payroll period (YYYY-MM-DD).
    pub effectiveFrom: String,
    pub gunlukAsgariUcret: Option<Decimal>,
    pub pekTavanKatsayisi: Option<Decimal>,
    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    pub gunlukYemekIstisnasiGV: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedStatutorySegmentSnapshot {
    pub effectiveFrom: String,
    pub effectiveTo: String,
    pub sgkPrimGunSayisi: i32,
    pub fiiliYemekGunu: i32,
    pub gunlukAsgariUcret: Decimal,
    pub pekTavanKatsayisi: Decimal,
    pub gunlukYemekIstisnasiSGK: Decimal,
    pub gunlukYemekIstisnasiGV: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedStatutorySnapshot {
    pub segments: Vec<ResolvedStatutorySegmentSnapshot>,
    pub sgkPrimGunSayisi: i32,
    pub pekAltSinir: Decimal,
    pub pekUstSinir: Decimal,
    pub sgkYemekIstisnasiToplam: Decimal,
    pub gvYemekIstisnasiToplam: Decimal,
    /// Gelir vergisi asgari ücret istisnası için vergi ayına taşınan son
    /// yürürlükteki günlük asgari ücret değeri.
    pub gvReferansGunlukAsgariUcret: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DonemselKurumDegerleri {'''
text = replace_once(text, anchor, insert, 'models statutory structs')
old = '''    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    pub pekTavanKatsayisi: Option<Decimal>,'''
new = '''    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    /// Gelir vergisi yemek istisnası SGK istisnasından bağımsız tutulur.
    pub gunlukYemekIstisnasiGV: Option<Decimal>,
    /// Yalnız bu açık/gelecek 15-14 dönemi içinde geçerli değişiklikler.
    /// Genel tarihsel mevzuat arşivi değildir.
    pub statutoryParameterSegments: Option<Vec<StatutoryParameterSegment>>,
    pub pekTavanKatsayisi: Option<Decimal>,'''
text = replace_once(text, old, new, 'models institution statutory fields')
old = '''            gunlukYemekIstisnasiSGK: Some(dec!(300.00)),
            pekTavanKatsayisi: Some(dec!(9)),'''
new = '''            gunlukYemekIstisnasiSGK: Some(dec!(300.00)),
            gunlukYemekIstisnasiGV: Some(dec!(300.00)),
            statutoryParameterSegments: None,
            pekTavanKatsayisi: Some(dec!(9)),'''
text = replace_once(text, old, new, 'models default statutory fields')
old = '''    pub gvDetay: Option<GvHesapDetayi>,
    pub odenenRaporluGun: Option<i32>,'''
new = '''    pub gvDetay: Option<GvHesapDetayi>,
    /// Bordro hesaplanırken çözümlenen period-local yasal parametrelerin snapshot'ı.
    pub statutorySnapshot: Option<ResolvedStatutorySnapshot>,
    pub odenenRaporluGun: Option<i32>,'''
text = replace_once(text, old, new, 'models payroll snapshot field')
write(path, text)


# ---------------------------------------------------------------------------
# 2) Sick leave overlap: fail closed on write, self excluded on update
# ---------------------------------------------------------------------------
path = 'src-tauri/src/repositories/sick_leave_repo.rs'
text = read(path)
text = replace_once(
    text,
    'use rusqlite::{params, Connection, Row};',
    'use rusqlite::{params, Connection, OptionalExtension, Row};',
    'sick repo OptionalExtension import',
)
anchor = '''    fn from_row(row: &Row) -> rusqlite::Result<SickLeaveRecord> {'''
insert = '''    fn validate_no_overlap(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        let overlap = conn
            .query_row(
                r#"
                SELECT id, start_date, end_date
                FROM sick_leave_records
                WHERE personnel_id = ?1
                  AND id <> ?2
                  AND start_date <= ?4
                  AND end_date >= ?3
                ORDER BY start_date ASC, end_date ASC, id ASC
                LIMIT 1
                "#,
                params![
                    record.personnelId,
                    record.id,
                    record.startDate,
                    record.endDate,
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some((id, start, end)) = overlap {
            return Err(DomainError::ValidationError(format!(
                "Rapor tarihleri çakışıyor: {}–{} aralığı, {} kaydındaki {}–{} aralığıyla örtüşüyor. Örtüşen raporlar ayrı episode olarak kaydedilemez.",
                record.startDate, record.endDate, id, start, end
            )));
        }
        Ok(())
    }

    fn from_row(row: &Row) -> rusqlite::Result<SickLeaveRecord> {'''
text = replace_once(text, anchor, insert, 'sick repo overlap helper')
old = '''    pub fn save(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        Self::validate_record(record)?;
        let now = chrono::Utc::now().to_rfc3339();'''
new = '''    pub fn save(conn: &Connection, record: &SickLeaveRecord) -> Result<()> {
        Self::validate_record(record)?;
        Self::validate_no_overlap(conn, record)?;
        let now = chrono::Utc::now().to_rfc3339();'''
text = replace_once(text, old, new, 'sick repo save overlap call')
write(path, text)


# ---------------------------------------------------------------------------
# 3) Institution settings: legacy normalization + period-local segment validation
# ---------------------------------------------------------------------------
path = 'src-tauri/src/repositories/settings_repo.rs'
text = read(path)
text = replace_once(text, 'use chrono::Utc;', 'use chrono::{NaiveDate, Utc};', 'settings chrono import')
text = replace_once(
    text,
    'use crate::domain::Result;',
    'use crate::domain::Result;\nuse crate::repositories::period_repo::PeriodRepository;',
    'settings period repo import',
)
old = '''        value.donemId = period_id.to_string();
        Ok(value)'''
new = '''        value.donemId = period_id.to_string();
        // Legacy settings stored a single meal exemption value. Preserve old data by
        // copying it once into the new, independent GV field; new saves persist both.
        if value.gunlukYemekIstisnasiGV.is_none() {
            value.gunlukYemekIstisnasiGV = value.gunlukYemekIstisnasiSGK;
        }
        Ok(value)'''
text = replace_once(text, old, new, 'settings legacy GV meal normalization')
anchor = '''    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {'''
insert = '''    pub fn validate_statutory_segments_for_period(
        period: &BordroDonemi,
        k: &DonemselKurumDegerleri,
    ) -> Result<()> {
        PeriodRepository::validate_period(period)?;
        if k.donemId != period.id {
            return Err(crate::domain::DomainError::ValidationError(format!(
                "Kurum ayarı dönem kimliği eşleşmiyor: {} / {}.",
                k.donemId, period.id
            )));
        }

        let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").map_err(|_| {
            crate::domain::DomainError::ValidationError(format!(
                "Dönem başlangıç tarihi geçersiz: {}.",
                period.baslangicTarihi
            ))
        })?;
        let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").map_err(|_| {
            crate::domain::DomainError::ValidationError(format!(
                "Dönem bitiş tarihi geçersiz: {}.",
                period.bitisTarihi
            ))
        })?;

        let mut previous_date: Option<NaiveDate> = None;
        for segment in k.statutoryParameterSegments.as_deref().unwrap_or(&[]) {
            let effective = NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d")
                .map_err(|_| {
                    crate::domain::DomainError::ValidationError(format!(
                        "Yasal parametre segment tarihi geçersiz: {}.",
                        segment.effectiveFrom
                    ))
                })?;
            if effective < start || effective > end {
                return Err(crate::domain::DomainError::ValidationError(format!(
                    "Yasal parametre segment tarihi {} dönemin dışında ({}–{}).",
                    segment.effectiveFrom, period.baslangicTarihi, period.bitisTarihi
                )));
            }
            if previous_date.is_some_and(|previous| effective <= previous) {
                return Err(crate::domain::DomainError::ValidationError(
                    "Yasal parametre segmentleri strictly artan tarihte ve tekrarsız olmalıdır."
                        .into(),
                ));
            }
            previous_date = Some(effective);

            if segment.gunlukAsgariUcret.is_none()
                && segment.pekTavanKatsayisi.is_none()
                && segment.gunlukYemekIstisnasiSGK.is_none()
                && segment.gunlukYemekIstisnasiGV.is_none()
            {
                return Err(crate::domain::DomainError::ValidationError(format!(
                    "{} tarihli yasal parametre segmentinde en az bir override bulunmalıdır.",
                    segment.effectiveFrom
                )));
            }
            if segment
                .gunlukAsgariUcret
                .is_some_and(|value| value <= rust_decimal::Decimal::ZERO)
            {
                return Err(crate::domain::DomainError::ValidationError(
                    "Segment günlük asgari ücret değeri sıfırdan büyük olmalıdır.".into(),
                ));
            }
            if segment
                .pekTavanKatsayisi
                .is_some_and(|value| value < rust_decimal::Decimal::ONE)
            {
                return Err(crate::domain::DomainError::ValidationError(
                    "Segment PEK tavan katsayısı en az 1 olmalıdır.".into(),
                ));
            }
            if segment
                .gunlukYemekIstisnasiSGK
                .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
                || segment
                    .gunlukYemekIstisnasiGV
                    .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
            {
                return Err(crate::domain::DomainError::ValidationError(
                    "Segment yemek istisnası negatif olamaz.".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {'''
text = replace_once(text, anchor, insert, 'settings segment validator insertion')
old = '''    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let json_str = serde_json::to_string(k)
            .map_err(|e| crate::domain::DomainError::InvalidData(e.to_string()))?;'''
new = '''    pub fn save_institution_settings(conn: &Connection, k: &DonemselKurumDegerleri) -> Result<()> {
        let period = PeriodRepository::get_by_id(conn, &k.donemId)?.ok_or_else(|| {
            crate::domain::DomainError::ValidationError(format!(
                "Kurum ayarı için bordro dönemi bulunamadı: {}.",
                k.donemId
            ))
        })?;
        let mut normalized = k.clone();
        if normalized.gunlukYemekIstisnasiGV.is_none() {
            normalized.gunlukYemekIstisnasiGV = normalized.gunlukYemekIstisnasiSGK;
        }
        Self::validate_statutory_segments_for_period(&period, &normalized)?;

        let now = Utc::now().to_rfc3339();
        let json_str = serde_json::to_string(&normalized)
            .map_err(|e| crate::domain::DomainError::InvalidData(e.to_string()))?;'''
text = replace_once(text, old, new, 'settings save normalization')
write(path, text)


# ---------------------------------------------------------------------------
# 4) Calculation layer consumes resolved period-level limits/exemptions
# ---------------------------------------------------------------------------
path = 'src-tauri/src/domain/calculations.rs'
text = read(path)
text = text.replace('calculate_prime_esas_kazanc_with_prim_gun_sayisi(', 'calculate_prime_esas_kazanc_with_statutory_snapshot(')
text = replace_once(
    text,
    '''    sgk_prim_gun_sayisi: Option<i32>,
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    let raw_prim_gun = puantaj_ozeti.map_or(0, |p| p.c + p.t + p.g + p.i + p.gc + p.gct + p.r);
    let prim_gun_sayisi = sgk_prim_gun_sayisi.unwrap_or(raw_prim_gun).clamp(0, 30);''',
    '''    statutory_snapshot: Option<&ResolvedStatutorySnapshot>,
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    let raw_prim_gun = puantaj_ozeti.map_or(0, |p| p.c + p.t + p.g + p.i + p.gc + p.gct + p.r);
    let prim_gun_sayisi = statutory_snapshot
        .map_or(raw_prim_gun, |snapshot| snapshot.sgkPrimGunSayisi)
        .clamp(0, 30);''',
    'calculations statutory signature',
)
old = '''    let gunluk_yemek_istisnasi = k.gunlukYemekIstisnasiSGK.unwrap_or(dec!(300.00));
    let brut_yemek = gelirler.yemek.unwrap_or(dec!(0));
    let yemek_istisnasi_tutar =
        brut_yemek.min(gunluk_yemek_istisnasi * Decimal::from(fiili_yemek_gunu));'''
new = '''    let brut_yemek = gelirler.yemek.unwrap_or(dec!(0));
    let yemek_istisnasi_tutar = if let Some(snapshot) = statutory_snapshot {
        brut_yemek.min(snapshot.sgkYemekIstisnasiToplam)
    } else {
        let gunluk_yemek_istisnasi = k.gunlukYemekIstisnasiSGK.unwrap_or(dec!(300.00));
        brut_yemek.min(gunluk_yemek_istisnasi * Decimal::from(fiili_yemek_gunu))
    };'''
text = replace_once(text, old, new, 'calculations segmented SGK meal exemption')
old = '''    let gunluk_asgari = k.gunlukAsgariUcret.unwrap_or(dec!(1101.00));
    let pek_alt_sinir = round2(gunluk_asgari * Decimal::from(prim_gun_sayisi));
    let tavan_katsayi = k.pekTavanKatsayisi.unwrap_or(dec!(9));
    let pek_ust_sinir = round2(gunluk_asgari * tavan_katsayi * Decimal::from(prim_gun_sayisi));'''
new = '''    let (pek_alt_sinir, pek_ust_sinir) = if let Some(snapshot) = statutory_snapshot {
        (snapshot.pekAltSinir, snapshot.pekUstSinir)
    } else {
        let gunluk_asgari = k.gunlukAsgariUcret.unwrap_or(dec!(1101.00));
        let tavan_katsayi = k.pekTavanKatsayisi.unwrap_or(dec!(9));
        (
            round2(gunluk_asgari * Decimal::from(prim_gun_sayisi)),
            round2(gunluk_asgari * tavan_katsayi * Decimal::from(prim_gun_sayisi)),
        )
    };'''
text = replace_once(text, old, new, 'calculations segmented PEK limits')
text = replace_once(
    text,
    '''    sgk_prim_gun_sayisi: Option<i32>,
) -> (KesintiKalemleri, PekDetayi, Vec<DevredenPekKaydi>) {''',
    '''    statutory_snapshot: Option<&ResolvedStatutorySnapshot>,
) -> (KesintiKalemleri, PekDetayi, Vec<DevredenPekKaydi>) {''',
    'calculations statutory deduction signature',
)
text = text.replace('            sgk_prim_gun_sayisi,\n', '            statutory_snapshot,\n')
text = text.replace('        sgk_prim_gun_sayisi,\n', '        statutory_snapshot,\n')
write(path, text)


# ---------------------------------------------------------------------------
# 5) Payroll service resolves per-day statutory segments and uses separate GV meal exemption
# ---------------------------------------------------------------------------
path = 'src-tauri/src/services/payroll_service.rs'
text = read(path)
pattern = re.compile(r'''/// SGK kamu sektörü 15-14 bordrolarında aylık PEK alt/üst sınırı 30 günlük\n.*?\nfn parse_period_date''', re.S)
replacement = r'''#[derive(Clone)]
struct ResolvedStatutoryValues {
    gunluk_asgari_ucret: Decimal,
    pek_tavan_katsayisi: Decimal,
    gunluk_yemek_istisnasi_sgk: Decimal,
    gunluk_yemek_istisnasi_gv: Decimal,
}

fn apply_statutory_segment(
    values: &mut ResolvedStatutoryValues,
    segment: &StatutoryParameterSegment,
) {
    if let Some(value) = segment.gunlukAsgariUcret {
        values.gunluk_asgari_ucret = value;
    }
    if let Some(value) = segment.pekTavanKatsayisi {
        values.pek_tavan_katsayisi = value;
    }
    if let Some(value) = segment.gunlukYemekIstisnasiSGK {
        values.gunluk_yemek_istisnasi_sgk = value;
    }
    if let Some(value) = segment.gunlukYemekIstisnasiGV {
        values.gunluk_yemek_istisnasi_gv = value;
    }
}

fn days_in_month(date: NaiveDate) -> Result<u32> {
    let next_month = if date.month() == 12 {
        NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
    }
    .ok_or_else(|| DomainError::InvalidData("Ay sonu çözümlenemedi.".into()))?;
    next_month
        .pred_opt()
        .map(|day| day.day())
        .ok_or_else(|| DomainError::InvalidData("Ay sonu çözümlenemedi.".into()))
}

/// Kamu 15-14 bildiriminde tam dönem 30 SGK günüdür. Ayın 15'inden ay
/// sonuna düşen kısım 16, takip eden ayın 1-14 kısmı 14 SGK günü taşır.
/// Böylece 31 günlük ayda 31. gün 0 ek SGK günü, Şubat sonunda ise eksik
/// takvim günleri aynı ilk parçaya sanal SGK günleri olarak eklenir.
fn full_period_sgk_day_weight(date: NaiveDate, period_start: NaiveDate) -> Result<i32> {
    if date.year() == period_start.year() && date.month() == period_start.month() {
        let month_days = days_in_month(period_start)?;
        if date.day() == month_days {
            let actual_second_half_days = (month_days - 14) as i32;
            return Ok(1 + (16 - actual_second_half_days));
        }
    }
    Ok(1)
}

fn is_prim_bearing_code(code: &str) -> bool {
    matches!(code, "Ç" | "T" | "G" | "İ" | "GÇ" | "GÇT" | "R")
}

pub fn resolve_statutory_snapshot_for_period(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    k: &DonemselKurumDegerleri,
) -> Result<ResolvedStatutorySnapshot> {
    SettingsRepository::validate_statutory_segments_for_period(period, k)?;
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let calendar_day_count = (end - start).num_days() + 1;
    let full_calendar_coverage = attendance.gunler.len() as i64 == calendar_day_count;

    let base_gv_meal = k
        .gunlukYemekIstisnasiGV
        .or(k.gunlukYemekIstisnasiSGK)
        .ok_or_else(|| DomainError::ValidationError("Günlük GV yemek istisnası eksik.".into()))?;
    let base = ResolvedStatutoryValues {
        gunluk_asgari_ucret: k
            .gunlukAsgariUcret
            .ok_or_else(|| DomainError::ValidationError("Günlük asgari ücret eksik.".into()))?,
        pek_tavan_katsayisi: k
            .pekTavanKatsayisi
            .ok_or_else(|| DomainError::ValidationError("PEK tavan katsayısı eksik.".into()))?,
        gunluk_yemek_istisnasi_sgk: k.gunlukYemekIstisnasiSGK.ok_or_else(|| {
            DomainError::ValidationError("Günlük SGK yemek istisnası eksik.".into())
        })?,
        gunluk_yemek_istisnasi_gv: base_gv_meal,
    };

    let mut points: Vec<(NaiveDate, ResolvedStatutoryValues)> = vec![(start, base.clone())];
    for segment in k.statutoryParameterSegments.as_deref().unwrap_or(&[]) {
        let effective = NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d")
            .map_err(|_| DomainError::ValidationError("Yasal segment tarihi geçersiz.".into()))?;
        if effective == start {
            apply_statutory_segment(&mut points[0].1, segment);
        } else {
            let mut next_values = points
                .last()
                .map(|(_, values)| values.clone())
                .ok_or_else(|| DomainError::InvalidData("Yasal parametre baseline eksik.".into()))?;
            apply_statutory_segment(&mut next_values, segment);
            points.push((effective, next_values));
        }
    }

    let mut snapshots = Vec::new();
    let mut total_sgk_days = 0i32;
    let mut pek_alt_sinir = Decimal::ZERO;
    let mut pek_ust_sinir = Decimal::ZERO;
    let mut sgk_meal_total = Decimal::ZERO;
    let mut gv_meal_total = Decimal::ZERO;

    for (index, (range_start, values)) in points.iter().enumerate() {
        let range_end = if let Some((next_start, _)) = points.get(index + 1) {
            next_start.pred_opt().ok_or_else(|| {
                DomainError::InvalidData("Yasal segment bitiş tarihi çözümlenemedi.".into())
            })?
        } else {
            end
        };

        let mut segment_sgk_days = 0i32;
        let mut worked_days = 0i32;
        for (date_text, code) in &attendance.gunler {
            let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| {
                DomainError::InvalidData(format!("Puantaj tarihi geçersiz: {}", date_text))
            })?;
            if date < *range_start || date > range_end {
                continue;
            }
            if is_prim_bearing_code(code) {
                segment_sgk_days += if full_calendar_coverage {
                    full_period_sgk_day_weight(date, start)?
                } else {
                    1
                };
            }
            if matches!(code.as_str(), "Ç" | "GÇ") {
                worked_days += 1;
            }
        }

        total_sgk_days += segment_sgk_days;
        pek_alt_sinir += values.gunluk_asgari_ucret * Decimal::from(segment_sgk_days);
        pek_ust_sinir += values.gunluk_asgari_ucret
            * values.pek_tavan_katsayisi
            * Decimal::from(segment_sgk_days);
        sgk_meal_total += values.gunluk_yemek_istisnasi_sgk * Decimal::from(worked_days);
        gv_meal_total += values.gunluk_yemek_istisnasi_gv * Decimal::from(worked_days);

        snapshots.push(ResolvedStatutorySegmentSnapshot {
            effectiveFrom: range_start.format("%Y-%m-%d").to_string(),
            effectiveTo: range_end.format("%Y-%m-%d").to_string(),
            sgkPrimGunSayisi: segment_sgk_days,
            fiiliYemekGunu: worked_days,
            gunlukAsgariUcret: values.gunluk_asgari_ucret,
            pekTavanKatsayisi: values.pek_tavan_katsayisi,
            gunlukYemekIstisnasiSGK: values.gunluk_yemek_istisnasi_sgk,
            gunlukYemekIstisnasiGV: values.gunluk_yemek_istisnasi_gv,
        });
    }

    if total_sgk_days > 30 {
        return Err(DomainError::InvalidData(format!(
            "{} dönemi çözümlenen SGK prim günü 30'u aşıyor: {}.",
            period.id, total_sgk_days
        )));
    }
    let gv_reference = snapshots
        .last()
        .map(|segment| segment.gunlukAsgariUcret)
        .ok_or_else(|| DomainError::InvalidData("Yasal parametre snapshot'ı boş.".into()))?;

    Ok(ResolvedStatutorySnapshot {
        segments: snapshots,
        sgkPrimGunSayisi: total_sgk_days,
        pekAltSinir: pek_alt_sinir.round_dp(2),
        pekUstSinir: pek_ust_sinir.round_dp(2),
        sgkYemekIstisnasiToplam: sgk_meal_total.round_dp(2),
        gvYemekIstisnasiToplam: gv_meal_total.round_dp(2),
        gvReferansGunlukAsgariUcret: gv_reference,
    })
}

fn parse_period_date'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1:
    raise RuntimeError(f'payroll service resolver replacement: expected 1, found {count}')
old = '''        let sgk_prim_gun_sayisi = resolve_sgk_prim_gun_sayisi(&attendance, &period, &summary)?;

        // Calculate exact payable sick dates'''
text = replace_once(text, old, '''        // Calculate exact payable sick dates''', 'remove legacy SGK day resolver call')
old = '''        validate_kurum_degerleri_for_payroll(&kurum_degerleri)?;

        let zam_aylari = get_zam_aylari(conn)?;'''
new = '''        validate_kurum_degerleri_for_payroll(&kurum_degerleri)?;
        SettingsRepository::validate_statutory_segments_for_period(&period, &kurum_degerleri)?;
        let statutory_snapshot =
            resolve_statutory_snapshot_for_period(&attendance, &period, &kurum_degerleri)?;

        let zam_aylari = get_zam_aylari(conn)?;'''
text = replace_once(text, old, new, 'payroll service resolve statutory snapshot')
text = replace_once(
    text,
    '                Some(sgk_prim_gun_sayisi),',
    '                Some(&statutory_snapshot),',
    'payroll service deduction statutory arg',
)
old = '''        let gunluk_asgari = kurum_degerleri
            .gunlukAsgariUcret
            .ok_or_else(|| DomainError::InvalidData("Günlük asgari ücret eksik.".into()))?;'''
new = '''        let gunluk_asgari = statutory_snapshot.gvReferansGunlukAsgariUcret;'''
text = replace_once(text, old, new, 'payroll service GV reference minimum wage')
old = '''        // GVK 23/8: nakit yemek yardımında fiilen çalışılan gün başına istisna limiti
        // uygulanır; limit üstü yemek ücret olarak GV matrahında kalır. Mevcut dönem
        // şemasında 2026 için aynı 300 TL değeri SGK yemek istisnası alanında tutuluyor.
        let fiili_yemek_gunu = (summary.c + summary.gc).max(0);
        let gunluk_yemek_gv_istisnasi = effective_kurum_degerleri
            .gunlukYemekIstisnasiSGK
            .ok_or_else(|| DomainError::InvalidData("Günlük yemek istisnası eksik.".into()))?;
        let yemek_gv_istisnasi = gelirler
            .yemek
            .unwrap_or_default()
            .min(gunluk_yemek_gv_istisnasi * Decimal::from(fiili_yemek_gunu));'''
new = '''        // SGK ve gelir vergisi yemek istisnaları bağımsız parametrelerdir.
        // Period-local segment çözümü fiilen çalışılan her tarihi doğru limite bağlar.
        let yemek_gv_istisnasi = gelirler
            .yemek
            .unwrap_or_default()
            .min(statutory_snapshot.gvYemekIstisnasiToplam);'''
text = replace_once(text, old, new, 'payroll service separate GV meal exemption')
old = '''            gvDetay: Some(gv_detay),
            odenenRaporluGun: Some(odenen_raporlu_gun),'''
new = '''            gvDetay: Some(gv_detay),
            statutorySnapshot: Some(statutory_snapshot),
            odenenRaporluGun: Some(odenen_raporlu_gun),'''
text = replace_once(text, old, new, 'payroll service snapshot field')
write(path, text)


# ---------------------------------------------------------------------------
# 6) Payroll persistence: resolved statutory snapshot survives FINALIZED lifecycle/backup
# ---------------------------------------------------------------------------
path = 'src-tauri/src/repositories/payroll_repo.rs'
text = read(path)
old = '''                        is_primi_snapshot_json, gv_snapshot_json, notlar
                 FROM payroll_records ORDER BY calculated_at ASC, id ASC'''
new = '''                        is_primi_snapshot_json, gv_snapshot_json, statutory_snapshot_json, notlar
                 FROM payroll_records ORDER BY calculated_at ASC, id ASC'''
text = replace_once(text, old, new, 'payroll repo select statutory snapshot')
old = '''                    row.get::<_, Option<String>>(22)?,
                    row.get::<_, Option<String>>(23)?,
                ))'''
new = '''                    row.get::<_, Option<String>>(22)?,
                    row.get::<_, Option<String>>(23)?,
                    row.get::<_, Option<String>>(24)?,
                ))'''
text = replace_once(text, old, new, 'payroll repo row indices')
old = '''            gv_snapshot_json,
            notlar,
        ) in raw_records'''
new = '''            gv_snapshot_json,
            statutory_snapshot_json,
            notlar,
        ) in raw_records'''
text = replace_once(text, old, new, 'payroll repo tuple destructure')
old = '''            let gv_detay = Self::parse_optional_json(
                gv_snapshot_json.as_deref(),
                &format!("{} GV snapshot'ı", id),
            )?;
            let gelirler = income_by_payroll.remove(&id).unwrap_or_default();'''
new = '''            let gv_detay = Self::parse_optional_json(
                gv_snapshot_json.as_deref(),
                &format!("{} GV snapshot'ı", id),
            )?;
            let statutory_snapshot = Self::parse_optional_json(
                statutory_snapshot_json.as_deref(),
                &format!("{} yasal parametre snapshot'ı", id),
            )?;
            let gelirler = income_by_payroll.remove(&id).unwrap_or_default();'''
text = replace_once(text, old, new, 'payroll repo parse statutory snapshot')
old = '''                gvDetay: gv_detay,
                odenenRaporluGun: odenen_raporlu_gun,'''
new = '''                gvDetay: gv_detay,
                statutorySnapshot: statutory_snapshot,
                odenenRaporluGun: odenen_raporlu_gun,'''
text = replace_once(text, old, new, 'payroll repo construct statutory snapshot')
old = '''        let gv_snapshot_json = Self::serialize_optional_json(b.gvDetay.as_ref(), "GV snapshot'ı")?;

        conn.execute('''
new = '''        let gv_snapshot_json = Self::serialize_optional_json(b.gvDetay.as_ref(), "GV snapshot'ı")?;
        let statutory_snapshot_json = Self::serialize_optional_json(
            b.statutorySnapshot.as_ref(),
            "Yasal parametre snapshot'ı",
        )?;

        conn.execute('''
text = replace_once(text, old, new, 'payroll repo serialize statutory snapshot')
old = '''                raporlu_gun, odenen_raporlu_gun, is_primi_snapshot_json, gv_snapshot_json, notlar,
                calculated_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
             ON CONFLICT(personnel_id, period_id) DO UPDATE SET
                gross_total=?4, sgk_base=?5, gv_base=?6, previous_cumulative_gv=?7, new_cumulative_gv=?8,
                income_tax=?9, stamp_tax=?10, total_deductions=?11, net_payment=?12, status=?13,
                puantaj_summary_json=?14, pek_detail_json=?15, devreden_pek_gelen_json=?16,
                sonraki_devreden_pek_json=?17, raporlu_gun=?18, odenen_raporlu_gun=?19,
                is_primi_snapshot_json=?20, gv_snapshot_json=?21, notlar=?22, updated_at=?24",'''
new = '''                raporlu_gun, odenen_raporlu_gun, is_primi_snapshot_json, gv_snapshot_json,
                statutory_snapshot_json, notlar, calculated_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
             ON CONFLICT(personnel_id, period_id) DO UPDATE SET
                gross_total=?4, sgk_base=?5, gv_base=?6, previous_cumulative_gv=?7, new_cumulative_gv=?8,
                income_tax=?9, stamp_tax=?10, total_deductions=?11, net_payment=?12, status=?13,
                puantaj_summary_json=?14, pek_detail_json=?15, devreden_pek_gelen_json=?16,
                sonraki_devreden_pek_json=?17, raporlu_gun=?18, odenen_raporlu_gun=?19,
                is_primi_snapshot_json=?20, gv_snapshot_json=?21, statutory_snapshot_json=?22,
                notlar=?23, updated_at=?25",'''
text = replace_once(text, old, new, 'payroll repo insert SQL snapshot')
old = '''                is_primi_snapshot_json,
                gv_snapshot_json,
                b.notlar,
                now,
                now,'''
new = '''                is_primi_snapshot_json,
                gv_snapshot_json,
                statutory_snapshot_json,
                b.notlar,
                now,
                now,'''
text = replace_once(text, old, new, 'payroll repo insert params snapshot')
write(path, text)


# Add statutorySnapshot: None to existing Rust BordroKaydi literals that did not
# exist before this field. Production/repository constructors above already set it.
for rs_path in (ROOT / 'src-tauri').rglob('*.rs'):
    rel = rs_path.as_posix()
    if rel in {
        'src-tauri/src/domain/models.rs',
        'src-tauri/src/services/payroll_service.rs',
        'src-tauri/src/repositories/payroll_repo.rs',
    }:
        continue
    source = rs_path.read_text(encoding='utf-8')
    lines = source.splitlines(keepends=True)
    out = []
    changed = False
    for line in lines:
        stripped = line.lstrip()
        if stripped.startswith('odenenRaporluGun:'):
            indent = line[: len(line) - len(stripped)]
            # Only BordroKaydi literals use this exact field spelling without `pub`.
            if not (out and out[-1].lstrip().startswith('statutorySnapshot:')):
                out.append(f'{indent}statutorySnapshot: None,\n')
                changed = True
        out.append(line)
    if changed:
        rs_path.write_text(''.join(out), encoding='utf-8')


# ---------------------------------------------------------------------------
# 7) DB migration: additive snapshot column + repair path
# ---------------------------------------------------------------------------
path = 'src-tauri/src/db/migrations.rs'
text = read(path)
needle = '''        M::up(
            r#"
            INSERT OR IGNORE INTO annual_payroll_parameters (year, params_json, updated_at)
            VALUES (
                2026,
                '{"year":2026,"gelirVergisiDilimleri":[{"limit":190000,"oran":0.15},{"limit":400000,"oran":0.20},{"limit":1500000,"oran":0.27},{"limit":5300000,"oran":0.35},{"limit":1000000000000000,"oran":0.40}]}',
                CURRENT_TIMESTAMP
            );
            "#,
        ),
    ])'''
replacement = '''        M::up(
            r#"
            INSERT OR IGNORE INTO annual_payroll_parameters (year, params_json, updated_at)
            VALUES (
                2026,
                '{"year":2026,"gelirVergisiDilimleri":[{"limit":190000,"oran":0.15},{"limit":400000,"oran":0.20},{"limit":1500000,"oran":0.27},{"limit":5300000,"oran":0.35},{"limit":1000000000000000,"oran":0.40}]}',
                CURRENT_TIMESTAMP
            );
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE payroll_records ADD COLUMN statutory_snapshot_json TEXT;
            "#,
        ),
    ])'''
text = replace_once(text, needle, replacement, 'migration statutory snapshot')
old = '''        ("gv_snapshot_json", "TEXT"),
        ("notlar", "TEXT"),'''
new = '''        ("gv_snapshot_json", "TEXT"),
        ("statutory_snapshot_json", "TEXT"),
        ("notlar", "TEXT"),'''
text = replace_once(text, old, new, 'migration optional snapshot repair')
write(path, text)


# ---------------------------------------------------------------------------
# 8) Frontend contract/defaults/editor for period-local statutory segments
# ---------------------------------------------------------------------------
path = 'src/types/payroll.ts'
text = read(path)
anchor = '''export interface DönemselKurumDegerleri {'''
insert = '''export interface StatutoryParameterSegment {
  effectiveFrom: string; // YYYY-MM-DD, active period inclusive
  gunlukAsgariUcret?: number;
  pekTavanKatsayisi?: number;
  gunlukYemekIstisnasiSGK?: number;
  gunlukYemekIstisnasiGV?: number;
}

export interface ResolvedStatutorySegmentSnapshot {
  effectiveFrom: string;
  effectiveTo: string;
  sgkPrimGunSayisi: number;
  fiiliYemekGunu: number;
  gunlukAsgariUcret: number;
  pekTavanKatsayisi: number;
  gunlukYemekIstisnasiSGK: number;
  gunlukYemekIstisnasiGV: number;
}

export interface ResolvedStatutorySnapshot {
  segments: ResolvedStatutorySegmentSnapshot[];
  sgkPrimGunSayisi: number;
  pekAltSinir: number;
  pekUstSinir: number;
  sgkYemekIstisnasiToplam: number;
  gvYemekIstisnasiToplam: number;
  gvReferansGunlukAsgariUcret: number;
}

export interface DönemselKurumDegerleri {'''
text = replace_once(text, anchor, insert, 'TS statutory types')
old = '''  gunlukYemekIstisnasiSGK?: number;
  pekTavanKatsayisi?: number;'''
new = '''  gunlukYemekIstisnasiSGK?: number;
  gunlukYemekIstisnasiGV?: number;
  statutoryParameterSegments?: StatutoryParameterSegment[];
  pekTavanKatsayisi?: number;'''
text = replace_once(text, old, new, 'TS institution statutory fields')
old = '''  gvDetay?: GvHesapDetayi;
  odenenRaporluGun?: number;'''
new = '''  gvDetay?: GvHesapDetayi;
  statutorySnapshot?: ResolvedStatutorySnapshot;
  odenenRaporluGun?: number;'''
text = replace_once(text, old, new, 'TS payroll snapshot')
write(path, text)

path = 'src/utils/payrollUtils.ts'
text = read(path)
old = '''  gunlukYemekIstisnasiSGK: 300.00, // 2026-08 (17.04.2026 sonrası) Günlük SGK yemek istisnası = 300 TL
  pekTavanKatsayisi: 9,'''
new = '''  gunlukYemekIstisnasiSGK: 300.00, // Dönem baseline değeri; mevzuat değişimi segment ile girilebilir.
  gunlukYemekIstisnasiGV: 300.00, // SGK'dan bağımsız GV yemek istisnası baseline değeri.
  statutoryParameterSegments: [],
  pekTavanKatsayisi: 9,'''
text = replace_once(text, old, new, 'frontend default separate meal exemptions')
write(path, text)

path = 'src/components/PeriodManagerModal.tsx'
text = read(path)
old = '''    tisIkramiyeListesi: activeKurumDegerleri.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
  });'''
new = '''    tisIkramiyeListesi: activeKurumDegerleri.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
    gunlukYemekIstisnasiGV:
      activeKurumDegerleri.gunlukYemekIstisnasiGV ??
      activeKurumDegerleri.gunlukYemekIstisnasiSGK ??
      DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiGV,
    statutoryParameterSegments: activeKurumDegerleri.statutoryParameterSegments || [],
  });'''
text = replace_once(text, old, new, 'period modal initial statutory state')
old = '''      tisIkramiyeListesi: active.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
    });'''
new = '''      tisIkramiyeListesi: active.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
      gunlukYemekIstisnasiGV:
        active.gunlukYemekIstisnasiGV ??
        active.gunlukYemekIstisnasiSGK ??
        DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiGV,
      statutoryParameterSegments: active.statutoryParameterSegments || [],
    });'''
text = replace_once(text, old, new, 'period modal reload statutory state')
old = '''      gunlukYemekIstisnasiSGK: paramsForm.gunlukYemekIstisnasiSGK ?? DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiSGK,
      pekTavanKatsayisi: paramsForm.pekTavanKatsayisi ?? DEFAULT_KURUM_DEGERLERI.pekTavanKatsayisi,'''
new = '''      gunlukYemekIstisnasiSGK: paramsForm.gunlukYemekIstisnasiSGK ?? DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiSGK,
      gunlukYemekIstisnasiGV:
        paramsForm.gunlukYemekIstisnasiGV ?? DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiGV,
      statutoryParameterSegments: paramsForm.statutoryParameterSegments || [],
      pekTavanKatsayisi: paramsForm.pekTavanKatsayisi ?? DEFAULT_KURUM_DEGERLERI.pekTavanKatsayisi,'''
text = replace_once(text, old, new, 'period modal create period statutory fields')
anchor = '''  // Preview generated dates
  const previewDonem = createBordroDonemi(newYear, newMonth, newTaxYear, newTaxMonth);'''
insert = '''  const activePeriodForParams = donemler.find((period) => period.id === aktifDonemId);
  const addStatutorySegment = () => {
    const current = paramsForm.statutoryParameterSegments || [];
    setParamsForm({
      ...paramsForm,
      statutoryParameterSegments: [
        ...current,
        {
          effectiveFrom: activePeriodForParams?.baslangicTarihi || '',
        },
      ],
    });
  };
  const updateStatutorySegment = (
    index: number,
    field: 'effectiveFrom' | 'gunlukAsgariUcret' | 'pekTavanKatsayisi' | 'gunlukYemekIstisnasiSGK' | 'gunlukYemekIstisnasiGV',
    value: string | number | undefined
  ) => {
    setParamsForm({
      ...paramsForm,
      statutoryParameterSegments: (paramsForm.statutoryParameterSegments || []).map((segment, i) =>
        i === index ? { ...segment, [field]: value } : segment
      ),
    });
  };
  const removeStatutorySegment = (index: number) => {
    setParamsForm({
      ...paramsForm,
      statutoryParameterSegments: (paramsForm.statutoryParameterSegments || []).filter((_, i) => i !== index),
    });
  };

  // Preview generated dates
  const previewDonem = createBordroDonemi(newYear, newMonth, newTaxYear, newTaxMonth);'''
text = replace_once(text, anchor, insert, 'period modal segment handlers')
# Make PEK field grid fit the new independent GV field.
text = replace_once(
    text,
    '<div className="grid grid-cols-1 sm:grid-cols-3 gap-4">\n                    <div>\n                      <label className="block text-xs font-semibold text-slate-700 mb-1">\n                        Günlük SGK Yemek İstisnası (TL)',
    '<div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">\n                    <div>\n                      <label className="block text-xs font-semibold text-slate-700 mb-1">\n                        Günlük SGK Yemek İstisnası (TL)',
    'period modal PEK grid columns',
)
sgk_block_end = '''                    </div>

                    <div>
                      <label className="block text-xs font-semibold text-slate-700 mb-1">
                        PEK Tavan Katsayısı (2026)'''
gv_block = '''                    </div>

                    <div>
                      <label className="block text-xs font-semibold text-slate-700 mb-1">
                        Günlük GV Yemek İstisnası (TL)
                      </label>
                      <input
                        type="number"
                        step="0.01"
                        value={paramsForm.gunlukYemekIstisnasiGV ?? ''}
                        onChange={(e) =>
                          setParamsForm({
                            ...paramsForm,
                            gunlukYemekIstisnasiGV: parseFloat(e.target.value) || 0,
                          })
                        }
                        className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                      />
                      <span className="text-[11px] text-slate-500 mt-0.5 block">
                        Gelir vergisi istisnası SGK limitinden bağımsızdır.
                      </span>
                    </div>

                    <div>
                      <label className="block text-xs font-semibold text-slate-700 mb-1">
                        PEK Tavan Katsayısı (2026)'''
text = replace_once(text, sgk_block_end, gv_block, 'period modal GV meal field')
anchor = '''                  </div>
                </div>
              </div>

              <div className="pt-3 border-t border-slate-200 flex justify-end">'''
segment_ui = '''                  </div>

                  <div className="mt-4 pt-4 border-t border-slate-200 space-y-3">
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <h5 className="text-xs font-bold text-slate-900">Dönem İçi Yasal Parametre Değişimleri</h5>
                        <p className="text-[11px] text-slate-600 mt-1">
                          Yalnız aktif/gelecek {activePeriodForParams?.baslangicTarihi}–{activePeriodForParams?.bitisTarihi}
                          dönemi içindeki yürürlük değişimlerini girin. Bu alan geçmiş mevzuat arşivi oluşturmaz.
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={addStatutorySegment}
                        className="px-3 py-1.5 rounded-lg bg-indigo-100 hover:bg-indigo-200 text-indigo-800 text-xs font-semibold flex items-center gap-1 shrink-0"
                      >
                        <Plus className="w-3.5 h-3.5" /> Segment Ekle
                      </button>
                    </div>

                    {(paramsForm.statutoryParameterSegments || []).map((segment, index) => (
                      <div key={`${segment.effectiveFrom}-${index}`} className="border border-indigo-200 bg-indigo-50/50 rounded-xl p-3 space-y-3">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-xs font-bold text-indigo-900">Segment {index + 1}</span>
                          <button
                            type="button"
                            onClick={() => removeStatutorySegment(index)}
                            className="p-1.5 rounded-lg text-rose-600 hover:bg-rose-100"
                            title="Segmenti kaldır"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
                          <div>
                            <label className="block text-[11px] font-semibold text-slate-700 mb-1">Yürürlük Tarihi</label>
                            <input
                              type="date"
                              min={activePeriodForParams?.baslangicTarihi}
                              max={activePeriodForParams?.bitisTarihi}
                              value={segment.effectiveFrom}
                              onChange={(e) => updateStatutorySegment(index, 'effectiveFrom', e.target.value)}
                              className="w-full px-2 py-2 bg-white border border-slate-300 rounded-lg text-xs"
                            />
                          </div>
                          {[
                            ['gunlukAsgariUcret', 'Günlük Asgari'],
                            ['pekTavanKatsayisi', 'PEK Katsayı'],
                            ['gunlukYemekIstisnasiSGK', 'SGK Yemek'],
                            ['gunlukYemekIstisnasiGV', 'GV Yemek'],
                          ].map(([field, label]) => (
                            <div key={field}>
                              <label className="block text-[11px] font-semibold text-slate-700 mb-1">{label}</label>
                              <input
                                type="number"
                                step="0.01"
                                value={(segment as any)[field] ?? ''}
                                placeholder="Değişmiyor"
                                onChange={(e) =>
                                  updateStatutorySegment(
                                    index,
                                    field as 'gunlukAsgariUcret' | 'pekTavanKatsayisi' | 'gunlukYemekIstisnasiSGK' | 'gunlukYemekIstisnasiGV',
                                    e.target.value === '' ? undefined : Number(e.target.value)
                                  )
                                }
                                className="w-full px-2 py-2 bg-white border border-slate-300 rounded-lg text-xs font-mono"
                              />
                            </div>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>

              <div className="pt-3 border-t border-slate-200 flex justify-end">'''
text = replace_once(text, anchor, segment_ui, 'period modal segment editor')
write(path, text)


# ---------------------------------------------------------------------------
# 9) Regression tests
# ---------------------------------------------------------------------------
sick_test = r'''use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, Personel, SickLeaveRecord};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::services::sick_leave_service::SickLeaveService;

fn person() -> Personel {
    Personel {
        id: "p-sick-overlap".into(),
        tcNo: "11111111111".into(),
        ad: "Rapor".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "1".into(),
        iban: "TR00".into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn leave(id: &str, start: &str, end: &str) -> SickLeaveRecord {
    SickLeaveRecord {
        id: id.into(),
        personnelId: "p-sick-overlap".into(),
        startDate: start.into(),
        endDate: end.into(),
        createdAt: None,
        updatedAt: None,
    }
}

fn setup() -> Result<rusqlite::Connection, Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    PersonnelRepository::save(&conn, &person())?;
    Ok(conn)
}

fn assert_validation<T>(result: Result<T, DomainError>) {
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
}

#[test]
fn partial_overlap_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-01-01", "2026-01-03"))?;
    assert_validation(SickLeaveRepository::save(
        &conn,
        &leave("b", "2026-01-02", "2026-01-04"),
    ));
    Ok(())
}

#[test]
fn touching_same_day_is_overlap_and_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-01-01", "2026-01-03"))?;
    assert_validation(SickLeaveRepository::save(
        &conn,
        &leave("b", "2026-01-03", "2026-01-05"),
    ));
    Ok(())
}

#[test]
fn adjacency_next_day_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-01-01", "2026-01-03"))?;
    SickLeaveRepository::save(&conn, &leave("b", "2026-01-04", "2026-01-06"))?;
    assert_eq!(SickLeaveRepository::get_by_personnel(&conn, "p-sick-overlap")?.len(), 2);
    Ok(())
}

#[test]
fn exact_duplicate_different_id_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-02-01", "2026-02-03"))?;
    assert_validation(SickLeaveRepository::save(
        &conn,
        &leave("b", "2026-02-01", "2026-02-03"),
    ));
    Ok(())
}

#[test]
fn updating_same_record_excludes_itself_from_overlap_check() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-03-01", "2026-03-10"))?;
    SickLeaveRepository::save(&conn, &leave("a", "2026-03-03", "2026-03-05"))?;
    let saved = SickLeaveRepository::get_by_id(&conn, "a")?.expect("record exists");
    assert_eq!(saved.startDate, "2026-03-03");
    assert_eq!(saved.endDate, "2026-03-05");
    Ok(())
}

#[test]
fn first_five_episode_rule_and_sixth_episode_behavior_preserved() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    for i in 0..6u32 {
        let day = 1 + i * 4;
        let start = format!("2026-04-{day:02}");
        let end = format!("2026-04-{:02}", day + 1);
        SickLeaveRepository::save(&conn, &leave(&format!("e{i}"), &start, &end))?;
    }
    let period = BordroDonemi {
        id: "2026-all".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-01".into(),
        bitisTarihi: "2026-12-31".into(),
        donemAdi: "2026".into(),
        taxYear: 2026,
        taxMonth: 12,
    };
    let paid = SickLeaveService::calculate_paid_sick_dates_for_period(
        &conn,
        "p-sick-overlap",
        &period,
    )?;
    assert_eq!(paid.len(), 10);
    Ok(())
}

#[test]
fn one_episode_may_cross_calendar_year_without_being_split_or_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = setup()?;
    SickLeaveRepository::save(&conn, &leave("cross", "2026-12-31", "2027-01-02"))?;
    let stored = SickLeaveRepository::get_by_id(&conn, "cross")?.expect("record exists");
    assert_eq!(stored.startDate, "2026-12-31");
    assert_eq!(stored.endDate, "2027-01-02");
    Ok(())
}
'''
write('src-tauri/tests/sick_leave_overlap_regression_test.rs', sick_test)

segment_test = r'''use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{
    AnnualPayrollParameters, BordroDonemi, DonemselKurumDegerleri, Personel, PersonelPuantaj,
    StatutoryParameterSegment,
};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::payroll_service::{
    resolve_statutory_snapshot_for_period, PayrollService,
};
use chrono::{Duration, NaiveDate};
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2025-12".into(),
        yil: 2025,
        ay: 12,
        baslangicTarihi: "2025-12-15".into(),
        bitisTarihi: "2026-01-14".into(),
        donemAdi: "Aralık 2025".into(),
        taxYear: 2026,
        taxMonth: 1,
    }
}

fn full_attendance(period: &BordroDonemi) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    let mut date = start;
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".to_string());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: "p-segment_2025-12".into(),
        personelId: "p-segment".into(),
        donemId: period.id.clone(),
        gunler,
    }
}

fn base_settings() -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: "2025-12".into(),
        gunlukAsgariUcret: Some(dec!(100)),
        pekTavanKatsayisi: Some(dec!(10)),
        gunlukYemekIstisnasiSGK: Some(dec!(20)),
        gunlukYemekIstisnasiGV: Some(dec!(30)),
        statutoryParameterSegments: None,
        ..Default::default()
    }
}

fn segment(effective_from: &str) -> StatutoryParameterSegment {
    StatutoryParameterSegment {
        effectiveFrom: effective_from.into(),
        gunlukAsgariUcret: None,
        pekTavanKatsayisi: None,
        gunlukYemekIstisnasiSGK: None,
        gunlukYemekIstisnasiGV: None,
    }
}

#[test]
fn no_segment_keeps_single_baseline_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &base_settings())?;
    assert_eq!(snapshot.sgkPrimGunSayisi, 30);
    assert_eq!(snapshot.pekAltSinir, dec!(3000));
    assert_eq!(snapshot.pekUstSinir, dec!(30000));
    assert_eq!(snapshot.segments.len(), 1);
    Ok(())
}

#[test]
fn january_first_change_splits_public_15_14_pek_limits_as_16_plus_14() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2026-01-01");
    change.gunlukAsgariUcret = Some(dec!(200));
    change.pekTavanKatsayisi = Some(dec!(9));
    change.gunlukYemekIstisnasiSGK = Some(dec!(25));
    settings.statutoryParameterSegments = Some(vec![change]);

    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.segments.len(), 2);
    assert_eq!(snapshot.segments[0].sgkPrimGunSayisi, 16);
    assert_eq!(snapshot.segments[1].sgkPrimGunSayisi, 14);
    assert_eq!(snapshot.pekAltSinir, dec!(4400));
    assert_eq!(snapshot.pekUstSinir, dec!(41200));
    // Meal exemptions follow real worked calendar dates, not virtual SGK days.
    assert_eq!(snapshot.sgkYemekIstisnasiToplam, dec!(690));
    // GV meal exemption did not change: 31 actual worked dates × 30.
    assert_eq!(snapshot.gvYemekIstisnasiToplam, dec!(930));
    assert_eq!(snapshot.gvReferansGunlukAsgariUcret, dec!(200));
    Ok(())
}

#[test]
fn segment_on_first_day_replaces_baseline_for_whole_period() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2025-12-15");
    change.gunlukAsgariUcret = Some(dec!(200));
    settings.statutoryParameterSegments = Some(vec![change]);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.segments.len(), 1);
    assert_eq!(snapshot.pekAltSinir, dec!(6000));
    Ok(())
}

#[test]
fn segment_on_last_day_affects_only_last_sgk_day() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2026-01-14");
    change.gunlukAsgariUcret = Some(dec!(200));
    settings.statutoryParameterSegments = Some(vec![change]);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.segments[0].sgkPrimGunSayisi, 29);
    assert_eq!(snapshot.segments[1].sgkPrimGunSayisi, 1);
    assert_eq!(snapshot.pekAltSinir, dec!(3100));
    Ok(())
}

#[test]
fn sgk_and_gv_meal_exemptions_change_independently() -> Result<(), Box<dyn std::error::Error>> {
    let p = period();
    let attendance = full_attendance(&p);
    let mut settings = base_settings();
    let mut change = segment("2026-01-01");
    change.gunlukYemekIstisnasiSGK = Some(dec!(40));
    settings.statutoryParameterSegments = Some(vec![change]);
    let snapshot = resolve_statutory_snapshot_for_period(&attendance, &p, &settings)?;
    assert_eq!(snapshot.sgkYemekIstisnasiToplam, dec!(900));
    assert_eq!(snapshot.gvYemekIstisnasiToplam, dec!(930));
    Ok(())
}

#[test]
fn out_of_period_segment_rejected_on_settings_save() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = period();
    PeriodRepository::save(&conn, &p)?;
    let mut settings = base_settings();
    let mut bad = segment("2025-12-14");
    bad.gunlukAsgariUcret = Some(dec!(150));
    settings.statutoryParameterSegments = Some(vec![bad]);
    assert!(matches!(
        SettingsRepository::save_institution_settings(&conn, &settings),
        Err(DomainError::ValidationError(_))
    ));
    Ok(())
}

#[test]
fn duplicate_or_unsorted_segment_dates_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = period();
    PeriodRepository::save(&conn, &p)?;
    let mut settings = base_settings();
    let mut a = segment("2026-01-05");
    a.gunlukAsgariUcret = Some(dec!(150));
    let mut b = segment("2026-01-01");
    b.gunlukAsgariUcret = Some(dec!(160));
    settings.statutoryParameterSegments = Some(vec![a, b]);
    assert!(matches!(
        SettingsRepository::save_institution_settings(&conn, &settings),
        Err(DomainError::ValidationError(_))
    ));
    Ok(())
}

fn person() -> Personel {
    Personel {
        id: "p-segment".into(),
        tcNo: "22222222222".into(),
        ad: "Segment".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "2".into(),
        iban: "TR00".into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

#[test]
fn payroll_persists_resolved_statutory_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = period();
    PeriodRepository::save(&conn, &p)?;
    PersonnelRepository::save(&conn, &person())?;
    AttendanceRepository::save(&conn, &full_attendance(&p))?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;

    let mut settings = base_settings();
    let mut change = segment("2026-01-01");
    change.gunlukAsgariUcret = Some(dec!(200));
    settings.statutoryParameterSegments = Some(vec![change]);
    SettingsRepository::save_institution_settings(&conn, &settings)?;

    let calculated = PayrollService::calculate_payroll_for_personnel(&conn, "p-segment", &p.id)?;
    assert_eq!(
        calculated
            .statutorySnapshot
            .as_ref()
            .expect("calculated snapshot")
            .pekAltSinir,
        dec!(4400)
    );
    let reloaded = PayrollRepository::get_all(&conn)?;
    let persisted = reloaded
        .iter()
        .find(|payroll| payroll.id == calculated.id)
        .and_then(|payroll| payroll.statutorySnapshot.as_ref())
        .expect("persisted statutory snapshot");
    assert_eq!(persisted.pekAltSinir, dec!(4400));
    assert_eq!(persisted.segments.len(), 2);
    Ok(())
}
'''
write('src-tauri/tests/statutory_segment_regression_test.rs', segment_test)


# ---------------------------------------------------------------------------
# 10) Plan status: record completed P1-high work without changing scope
# ---------------------------------------------------------------------------
path = 'docs/payroll-engine-hardening-plan.md'
text = read(path)
anchor = '''**Temel ürün kararı:** Uygulama geçmiş dönem bordrolarını yeniden üretmeyecek ve eski mevzuatı tarihsel bir kural arşivi olarak modellemeyecek.

---'''
insert = '''**Temel ürün kararı:** Uygulama geçmiş dönem bordrolarını yeniden üretmeyecek ve eski mevzuatı tarihsel bir kural arşivi olarak modellemeyecek.

### Uygulama durumu — 16 Ağustos 2026

- ✅ Faz 1 — Devreden PEK işçi prim matrahı tamamlandı.
- ✅ Faz 2 — Puantaj tarih/dönem invariantı tamamlandı.
- ✅ Faz 3 — Kümülatif GV / STALE / FINALIZED zinciri tamamlandı.
- ✅ Faz 4 — Örtüşen rapor kayıtları write path'te fail-closed reddediliyor; adjacency ayrı episode olarak kalıyor.
- ✅ Faz 5 — Period-local effective-date segmentleri, bağımsız SGK/GV yemek istisnaları ve resolved statutory snapshot tamamlandı.
- ⏳ Faz 6–11 bu planın kalan işleri olarak devam ediyor.

---'''
text = replace_once(text, anchor, insert, 'hardening plan status')
write(path, text)

print('P1-high source and regression changes staged successfully.')
