use crate::domain::models::{BordroDonemi, DevredenPekKaydi};
use crate::domain::{DomainError, Result};
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::SettingsRepository;
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;

pub struct PayrollPreflightService;

impl PayrollPreflightService {
    /// Production hesaplama/finalize öncesi veri zinciri kontrolü.
    /// Amaç eksik veya çelişkili girdiden tahmin üretmek değil, Excel'in yerine
    /// geçen uygulamada yanlış bordroyu fail-closed biçimde engellemektir.
    pub fn validate_for_calculation(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<()> {
        let period = PeriodRepository::get_by_id(conn, period_id)?
            .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", period_id)))?;

        PeriodRepository::validate_tax_chronology(conn, &period)?;
        Self::validate_tax_month_chain(conn, personnel_id, &period)?;
        Self::validate_statutory_tax_month_reference(conn, &period)?;
        Self::validate_devreden_pek_gap(conn, personnel_id, &period)?;
        Ok(())
    }

    fn validate_tax_month_chain(
        conn: &Connection,
        personnel_id: &str,
        period: &BordroDonemi,
    ) -> Result<()> {
        if period.taxMonth <= 1 {
            return Ok(());
        }

        let personel = PersonnelRepository::get_by_id(conn, personnel_id)?
            .ok_or_else(|| DomainError::NotFound(format!("Personel bulunamadı: {}", personnel_id)))?;

        // Yıl ortasında sisteme geçişte kullanıcı asgari GV referans devrini
        // açıkça girdiyse, eksiksiz dönem beklentisi bu başlangıç ayından başlar.
        // Devir yoksa vergi yılının 1. ayından itibaren dönem zinciri eksiksiz olmalıdır.
        let has_asgari_opening = personel.devirKumulatifAsgariGvMatrahi.is_some()
            && personel
                .devirKumulatifAsgariGvMatrahiYili
                .unwrap_or(period.taxYear)
                == period.taxYear;
        let start_month = if has_asgari_opening {
            personel
                .devirKumulatifGvMatrahiBaslangicAyi
                .unwrap_or(1)
        } else {
            1
        };

        if !(1..=12).contains(&start_month) {
            return Err(DomainError::ValidationError(
                "Asgari GV devir başlangıç ayı 1-12 arasında olmalıdır.".into(),
            ));
        }
        if start_month > period.taxMonth {
            return Err(DomainError::ValidationError(format!(
                "Asgari GV devir başlangıç ayı {} aktif vergi ayı {} sonrasında olamaz.",
                start_month, period.taxMonth
            )));
        }
        if start_month == period.taxMonth {
            return Ok(());
        }

        let mut stmt = conn
            .prepare(
                "SELECT tax_month
                 FROM payroll_periods
                 WHERE tax_year = ?1
                   AND tax_month >= ?2
                   AND tax_month < ?3
                 ORDER BY tax_month ASC",
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map(params![period.taxYear, start_month, period.taxMonth], |row| {
                row.get::<_, i32>(0)
            })
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        let mut present = std::collections::BTreeSet::new();
        for row in rows {
            present.insert(row.map_err(|e| DomainError::DatabaseError(e.to_string()))?);
        }

        let missing: Vec<i32> = (start_month..period.taxMonth)
            .filter(|month| !present.contains(month))
            .collect();
        if !missing.is_empty() {
            let months = missing
                .iter()
                .map(|month| format!("{:02}", month))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(DomainError::ValidationError(format!(
                "{} vergi yılı asgari ücret GV referans zinciri eksik. Önce şu vergi aylarına ait dönemleri oluşturun veya uygun devir başlangıcını girin: {}.",
                period.taxYear, months
            )));
        }

        Ok(())
    }

    fn validate_statutory_tax_month_reference(
        conn: &Connection,
        period: &BordroDonemi,
    ) -> Result<()> {
        let settings = SettingsRepository::get_institution_settings(conn, &period.id)?
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bulunamadı; bordro hesaplanamaz.",
                    period.id
                ))
            })?;
        SettingsRepository::validate_statutory_segments_for_period(period, &settings)?;

        let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .map_err(|e| DomainError::InvalidData(e.to_string()))?;
        let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")
            .map_err(|e| DomainError::InvalidData(e.to_string()))?;

        let target_date = if period.taxYear == start.year()
            && period.taxMonth == start.month() as i32
        {
            start
        } else if period.taxYear == end.year() && period.taxMonth == end.month() as i32 {
            NaiveDate::from_ymd_opt(end.year(), end.month(), 1).ok_or_else(|| {
                DomainError::InvalidData("Vergi ayı referans tarihi çözümlenemedi.".into())
            })?
        } else {
            return Err(DomainError::ValidationError(format!(
                "Vergi ayı {}-{:02} çalışma dönemiyle örtüşmüyor.",
                period.taxYear, period.taxMonth
            )));
        };

        let base = settings.gunlukAsgariUcret.ok_or_else(|| {
            DomainError::ValidationError("Günlük asgari ücret eksik.".into())
        })?;
        let mut target_value = base;
        let mut final_value = base;
        for segment in settings.statutoryParameterSegments.as_deref().unwrap_or(&[]) {
            let effective = NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d")
                .map_err(|_| {
                    DomainError::ValidationError(format!(
                        "Yasal parametre segment tarihi geçersiz: {}.",
                        segment.effectiveFrom
                    ))
                })?;
            if let Some(value) = segment.gunlukAsgariUcret {
                final_value = value;
                if effective <= target_date {
                    target_value = value;
                }
            }
        }

        // Mevcut production motoru GV/DV asgari ücret referansında dönemin son
        // resolved değerini kullanıyor. Seçilen taxMonth'ta yürürlükteki değer
        // bundan farklıysa yanlış ay referansı üretmek yerine hesaplamayı durdur.
        if target_value != final_value {
            return Err(DomainError::ValidationError(format!(
                "{} döneminde asgari ücret dönem içinde değişiyor ve seçilen vergi ayı {}-{:02} son yasal segmentle uyuşmuyor. Yanlış GV/DV istisnası üretmemek için vergi ayını yürürlükteki asgari ücret segmentiyle uyumlu seçin.",
                period.id, period.taxYear, period.taxMonth
            )));
        }

        Ok(())
    }

    fn validate_devreden_pek_gap(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<()> {
        let prior = conn
            .query_row(
                "SELECT pr.status, pr.sonraki_devreden_pek_json,
                        pp.id, pp.yil, pp.ay
                 FROM payroll_records AS pr
                 JOIN payroll_periods AS pp ON pp.id = pr.period_id
                 WHERE pr.personnel_id = ?1
                   AND pp.baslangic_tarihi < ?2
                 ORDER BY pp.baslangic_tarihi DESC, pp.id DESC
                 LIMIT 1",
                params![personnel_id, active_period.baslangicTarihi],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i32>(3)?,
                        row.get::<_, i32>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let Some((status, json, source_period_id, source_year, source_month)) = prior else {
            return Ok(());
        };
        let Some(json) = json else {
            return Ok(());
        };
        let carry: Vec<DevredenPekKaydi> = serde_json::from_str(&json).map_err(|e| {
            DomainError::InvalidData(format!(
                "{} dönemi devreden PEK kaydı bozuk JSON içeriyor: {}",
                source_period_id, e
            ))
        })?;
        let positive: Vec<&DevredenPekKaydi> = carry
            .iter()
            .filter(|item| item.tutar > Decimal::ZERO && item.kalanAySayisi > 0)
            .collect();
        if positive.is_empty() {
            return Ok(());
        }

        let source_ordinal = i64::from(source_year) * 12 + i64::from(source_month);
        let active_ordinal = i64::from(active_period.yil) * 12 + i64::from(active_period.ay);
        let distance = active_ordinal - source_ordinal;
        if distance <= 0 {
            return Err(DomainError::InvalidData(
                "Devreden PEK çalışma dönemi kronolojisi geçersiz.".into(),
            ));
        }
        if distance == 1 {
            return Ok(());
        }

        let skipped_months = (distance - 1) as i32;
        let still_live = positive
            .iter()
            .any(|item| item.kalanAySayisi - skipped_months > 0);
        if !still_live {
            // Taşıma penceresi aradaki aylar içinde zaten dolmuşsa aktif
            // dönemde boş PEK gelmesi doğru davranıştır; gereksiz bloklama yapma.
            return Ok(());
        }

        if status != "CALCULATED" && status != "FINALIZED" {
            return Err(DomainError::ValidationError(format!(
                "{} dönemindeki son önceki bordro {} durumda ve devreden PEK taşıyor. Önce bu bordroyu yeniden hesaplayın.",
                source_period_id, status
            )));
        }

        let (expected_previous_year, expected_previous_month) = if active_period.ay == 1 {
            (active_period.yil - 1, 12)
        } else {
            (active_period.yil, active_period.ay - 1)
        };
        let expected_previous_period_id = conn
            .query_row(
                "SELECT id FROM payroll_periods WHERE yil = ?1 AND ay = ?2 LIMIT 1",
                params![expected_previous_year, expected_previous_month],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        match expected_previous_period_id {
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
    }
}
