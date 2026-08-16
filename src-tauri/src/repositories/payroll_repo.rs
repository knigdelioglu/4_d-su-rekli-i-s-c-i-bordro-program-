use super::{dec_to_kurus, kurus_to_dec};
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PayrollDependencyFingerprint {
    gv_base: i64,
    sgk_base: i64,
    new_cumulative_gv: i64,
    net_payment: i64,
    sonraki_devreden_pek_json: Option<String>,
}

pub struct PayrollRepository;

impl PayrollRepository {
    fn status_to_str(status: BordroStatus) -> &'static str {
        match status {
            BordroStatus::DRAFT => "DRAFT",
            BordroStatus::CALCULATED => "CALCULATED",
            BordroStatus::STALE => "STALE",
            BordroStatus::FINALIZED => "FINALIZED",
        }
    }

    fn parse_status(id: &str, status: &str) -> Result<BordroStatus> {
        match status {
            "DRAFT" => Ok(BordroStatus::DRAFT),
            "CALCULATED" => Ok(BordroStatus::CALCULATED),
            "STALE" => Ok(BordroStatus::STALE),
            "FINALIZED" => Ok(BordroStatus::FINALIZED),
            _ => Err(DomainError::InvalidData(format!(
                "{} bordrosunun durumu geçersiz: {}",
                id, status
            ))),
        }
    }

    fn dependency_fingerprint(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<Option<PayrollDependencyFingerprint>> {
        conn.query_row(
            "SELECT gv_base, sgk_base, new_cumulative_gv, net_payment, sonraki_devreden_pek_json
             FROM payroll_records
             WHERE personnel_id = ?1 AND period_id = ?2",
            params![personnel_id, period_id],
            |row| {
                Ok(PayrollDependencyFingerprint {
                    gv_base: row.get(0)?,
                    sgk_base: row.get(1)?,
                    new_cumulative_gv: row.get(2)?,
                    net_payment: row.get(3)?,
                    sonraki_devreden_pek_json: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    fn has_later_finalized_by_work_period(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<bool> {
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM payroll_records AS pr
                    JOIN payroll_periods AS later ON later.id = pr.period_id
                    JOIN payroll_periods AS current ON current.id = ?2
                    WHERE pr.personnel_id = ?1
                      AND pr.status = 'FINALIZED'
                      AND (later.yil > current.yil OR (later.yil = current.yil AND later.ay > current.ay))
                 )",
                params![personnel_id, period_id],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(exists != 0)
    }

    fn mark_later_calculated_stale(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE personnel_id = ?2
               AND status = 'CALCULATED'
               AND period_id IN (
                    SELECT later.id
                    FROM payroll_periods AS later
                    JOIN payroll_periods AS current ON current.id = ?3
                    WHERE later.yil > current.yil
                       OR (later.yil = current.yil AND later.ay > current.ay)
               )",
            params![now, personnel_id, period_id],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    fn has_nonfinalized_prior_tax_chain(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<bool> {
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM payroll_records AS pr
                    JOIN payroll_periods AS prior ON prior.id = pr.period_id
                    JOIN payroll_periods AS current ON current.id = ?2
                    WHERE pr.personnel_id = ?1
                      AND prior.tax_year = current.tax_year
                      AND prior.tax_month < current.tax_month
                      AND pr.status <> 'FINALIZED'
                 )",
                params![personnel_id, period_id],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(exists != 0)
    }

    pub fn get_status_and_created_at(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<Option<(BordroStatus, String)>> {
        let row = conn
            .query_row(
                "SELECT id, status, calculated_at
                 FROM payroll_records
                 WHERE personnel_id = ?1 AND period_id = ?2",
                params![personnel_id, period_id],
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

        row.map(|(id, status, calculated_at)| {
            Ok((Self::parse_status(&id, &status)?, calculated_at))
        })
        .transpose()
    }

    pub fn update_status(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        status: BordroStatus,
    ) -> Result<()> {
        let current = Self::get_status_and_created_at(conn, personnel_id, period_id)?
            .ok_or_else(|| DomainError::NotFound("Bordro kaydı bulunamadı.".into()))?;
        let current_status = current.0;

        if current_status == BordroStatus::FINALIZED && status != BordroStatus::FINALIZED {
            return Err(DomainError::PayrollFinalized(
                "Kesinleştirilmiş (FINALIZED) bordronun durumu değiştirilemez.".into(),
            ));
        }

        if status == BordroStatus::FINALIZED {
            if current_status == BordroStatus::STALE {
                return Err(DomainError::ValidationError(
                    "STALE bordro kesinleştirilemez. Önce bordroyu yeniden hesaplayın.".into(),
                ));
            }
            if current_status == BordroStatus::DRAFT {
                return Err(DomainError::ValidationError(
                    "DRAFT bordro kesinleştirilemez. Önce bordroyu hesaplayın.".into(),
                ));
            }
            if current_status != BordroStatus::FINALIZED
                && Self::has_nonfinalized_prior_tax_chain(conn, personnel_id, period_id)?
            {
                return Err(DomainError::ValidationError(
                    "Bu bordro kesinleştirilemez: aynı vergi yılındaki önceki mevcut bordrolar önce FINALIZED olmalıdır.".into(),
                ));
            }
        }

        let updated_at = Utc::now().to_rfc3339();
        let changed = conn
            .execute(
                "UPDATE payroll_records
                 SET status = ?1, updated_at = ?2
                 WHERE personnel_id = ?3 AND period_id = ?4",
                params![
                    Self::status_to_str(status),
                    updated_at,
                    personnel_id,
                    period_id
                ],
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        if changed == 0 {
            return Err(DomainError::NotFound("Bordro kaydı bulunamadı.".into()));
        }
        Ok(())
    }

    pub fn exists_for_period(conn: &Connection, period_id: &str) -> Result<bool> {
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM payroll_records WHERE period_id = ?1
                 )",
                params![period_id],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(exists != 0)
    }

    pub fn has_personnel_tax_month_before(
        conn: &Connection,
        personnel_id: &str,
        tax_year: i32,
        tax_month: i32,
    ) -> Result<bool> {
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM payroll_records AS pr
                    JOIN payroll_periods AS pp ON pp.id = pr.period_id
                    WHERE pr.personnel_id = ?1
                      AND pp.tax_year = ?2
                      AND pp.tax_month < ?3
                      AND pr.status IN ('CALCULATED', 'FINALIZED')
                 )",
                params![personnel_id, tax_year, tax_month],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(exists != 0)
    }

    pub fn sum_gv_base_for_tax_month_range(
        conn: &Connection,
        personnel_id: &str,
        tax_year: i32,
        start_tax_month: i32,
        end_tax_month_exclusive: i32,
    ) -> Result<Decimal> {
        let non_authoritative: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM payroll_records AS pr
                 JOIN payroll_periods AS pp ON pp.id = pr.period_id
                 WHERE pr.personnel_id = ?1
                   AND pp.tax_year = ?2
                   AND pp.tax_month >= ?3
                   AND pp.tax_month < ?4
                   AND pr.status IN ('DRAFT', 'STALE')",
                params![
                    personnel_id,
                    tax_year,
                    start_tax_month,
                    end_tax_month_exclusive
                ],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if non_authoritative > 0 {
            return Err(DomainError::ValidationError(
                "Önceki vergi zincirinde DRAFT/STALE bordro var. Kümülatif GV hesabına devam etmeden önce bu bordroları yeniden hesaplayın.".into(),
            ));
        }

        let sum_kurus: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(pr.gv_base), 0)
                 FROM payroll_records AS pr
                 JOIN payroll_periods AS pp ON pp.id = pr.period_id
                 WHERE pr.personnel_id = ?1
                   AND pp.tax_year = ?2
                   AND pp.tax_month >= ?3
                   AND pp.tax_month < ?4
                   AND pr.status IN ('CALCULATED', 'FINALIZED')",
                params![
                    personnel_id,
                    tax_year,
                    start_tax_month,
                    end_tax_month_exclusive
                ],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(kurus_to_dec(sum_kurus))
    }

    pub fn get_next_devreden_pek(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<Option<Vec<DevredenPekKaydi>>> {
        let row = conn
            .query_row(
                "SELECT id, status, sonraki_devreden_pek_json
                 FROM payroll_records
                 WHERE personnel_id = ?1 AND period_id = ?2",
                params![personnel_id, period_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let Some((id, status_text, json)) = row else {
            return Ok(None);
        };
        match Self::parse_status(&id, &status_text)? {
            BordroStatus::DRAFT | BordroStatus::STALE => {
                return Err(DomainError::ValidationError(format!(
                    "{} dönemindeki önceki bordro {} durumda; devreden PEK authoritative değildir. Önce bordroyu yeniden hesaplayın.",
                    period_id, status_text
                )))
            }
            BordroStatus::CALCULATED | BordroStatus::FINALIZED => {}
        }

        Self::parse_optional_json(
            json.as_deref(),
            &format!("{} {} sonraki devreden PEK", personnel_id, period_id),
        )
    }

    fn parse_json<T: DeserializeOwned>(json: &str, field: &str) -> Result<T> {
        serde_json::from_str(json)
            .map_err(|e| DomainError::InvalidData(format!("{} bozuk JSON içeriyor: {}", field, e)))
    }

    fn parse_optional_json<T: DeserializeOwned>(
        json: Option<&str>,
        field: &str,
    ) -> Result<Option<T>> {
        json.map(|value| Self::parse_json(value, field)).transpose()
    }

    fn serialize_json<T: Serialize>(value: &T, field: &str) -> Result<String> {
        serde_json::to_string(value)
            .map_err(|e| DomainError::InvalidData(format!("{} serileştirilemedi: {}", field, e)))
    }

    fn serialize_optional_json<T: Serialize>(
        value: Option<&T>,
        field: &str,
    ) -> Result<Option<String>> {
        value
            .map(|item| Self::serialize_json(item, field))
            .transpose()
    }

    fn set_income_item(
        gelirler: &mut GelirKalemleri,
        item_type: &str,
        amount: Decimal,
    ) -> Result<()> {
        match item_type {
            "tabanBrutAylik" => gelirler.tabanBrutAylik = Some(amount),
            "tediye" => gelirler.tediye = Some(amount),
            "tisIkramiyesi" => gelirler.tisIkramiyesi = Some(amount),
            "ekOdeme" => gelirler.ekOdeme = Some(amount),
            "yemek" => gelirler.yemek = Some(amount),
            "birlestirilmisSosyalYardim" => gelirler.birlestirilmisSosyalYardim = Some(amount),
            "vasitaYol" => gelirler.vasitaYol = Some(amount),
            "giyimYardimi" => gelirler.giyimYardimi = Some(amount),
            "isPrimi" => gelirler.isPrimi = Some(amount),
            "geceCalismasiUcreti" => gelirler.geceCalismasiUcreti = Some(amount),
            "geceCalismasiTatiliUcreti" => gelirler.geceCalismasiTatiliUcreti = Some(amount),
            "hizmetZammi" => gelirler.hizmetZammi = Some(amount),
            "digerGelir" => gelirler.digerGelir = Some(amount),
            _ => {
                return Err(DomainError::InvalidData(format!(
                    "Bilinmeyen bordro gelir kalemi: {}",
                    item_type
                )))
            }
        }
        Ok(())
    }

    fn set_deduction_item(
        kesintiler: &mut KesintiKalemleri,
        item_type: &str,
        amount: Decimal,
    ) -> Result<()> {
        match item_type {
            "isciSgkPrimi" => kesintiler.isciSgkPrimi = Some(amount),
            "isciIssizlikPrimi" => kesintiler.isciIssizlikPrimi = Some(amount),
            "gelirVergisi" => kesintiler.gelirVergisi = Some(amount),
            "damgaVergisi" => kesintiler.damgaVergisi = Some(amount),
            "sendikaAidati" => kesintiler.sendikaAidati = Some(amount),
            "bes" => kesintiler.bes = Some(amount),
            "icra" => kesintiler.icra = Some(amount),
            "kisiBorcu" => kesintiler.kisiBorcu = Some(amount),
            "dogumAskerlikBorclanmasi" => kesintiler.dogumAskerlikBorclanmasi = Some(amount),
            "hayatSaglikSigortasi" => kesintiler.hayatSaglikSigortasi = Some(amount),
            "digerKesinti" => kesintiler.digerKesinti = Some(amount),
            _ => {
                return Err(DomainError::InvalidData(format!(
                    "Bilinmeyen bordro kesinti kalemi: {}",
                    item_type
                )))
            }
        }
        Ok(())
    }

    pub fn get_all(conn: &Connection) -> Result<Vec<BordroKaydi>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, personnel_id, period_id, gross_total, sgk_base, gv_base,
                        previous_cumulative_gv, new_cumulative_gv, income_tax, stamp_tax,
                        total_deductions, net_payment, status, puantaj_summary_json,
                        pek_detail_json, devreden_pek_gelen_json, sonraki_devreden_pek_json,
                        calculated_at, updated_at, raporlu_gun, odenen_raporlu_gun,
                        is_primi_snapshot_json, gv_snapshot_json, notlar
                 FROM payroll_records ORDER BY calculated_at ASC, id ASC",
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, Option<String>>(16)?,
                    row.get::<_, String>(17)?,
                    row.get::<_, String>(18)?,
                    row.get::<_, Option<i32>>(19)?,
                    row.get::<_, Option<i32>>(20)?,
                    row.get::<_, Option<String>>(21)?,
                    row.get::<_, Option<String>>(22)?,
                    row.get::<_, Option<String>>(23)?,
                ))
            })
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut raw_records = Vec::new();
        for row in rows {
            raw_records.push(row.map_err(|e| DomainError::DatabaseError(e.to_string()))?);
        }

        // Kalemler iki toplu sorgu ile okunur; her bordro için ayrı SELECT yapılmaz.
        let mut income_stmt = conn
            .prepare("SELECT payroll_id, item_type, amount FROM payroll_income_items ORDER BY payroll_id, item_type")
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        let mut deduction_stmt = conn
            .prepare("SELECT payroll_id, item_type, amount FROM payroll_deduction_items ORDER BY payroll_id, item_type")
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut income_by_payroll: std::collections::HashMap<String, GelirKalemleri> =
            std::collections::HashMap::new();
        let income_rows = income_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        for row in income_rows {
            let (payroll_id, item_type, amount) =
                row.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            let entry = income_by_payroll.entry(payroll_id).or_default();
            Self::set_income_item(entry, &item_type, kurus_to_dec(amount))?;
        }

        let mut deductions_by_payroll: std::collections::HashMap<String, KesintiKalemleri> =
            std::collections::HashMap::new();
        let deduction_rows = deduction_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        for row in deduction_rows {
            let (payroll_id, item_type, amount) =
                row.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            let entry = deductions_by_payroll.entry(payroll_id).or_default();
            Self::set_deduction_item(entry, &item_type, kurus_to_dec(amount))?;
        }

        let mut result = Vec::with_capacity(raw_records.len());
        for (
            id,
            personnel_id,
            period_id,
            gross_total,
            previous_cumulative_gv,
            total_deductions,
            net_payment,
            status_str,
            puantaj_summary_json,
            pek_detail_json,
            devreden_pek_gelen_json,
            sonraki_devreden_pek_json,
            calculated_at,
            updated_at,
            raporlu_gun,
            odenen_raporlu_gun,
            is_primi_snapshot_json,
            gv_snapshot_json,
            notlar,
        ) in raw_records
        {
            let status = Self::parse_status(&id, &status_str)?;

            let puantaj_summary: PuantajOzeti =
                Self::parse_json(&puantaj_summary_json, &format!("{} puantaj özeti", id))?;
            let pek_detay = Self::parse_optional_json(
                pek_detail_json.as_deref(),
                &format!("{} PEK detayı", id),
            )?;
            let devreden_pek_gelen = Self::parse_optional_json(
                devreden_pek_gelen_json.as_deref(),
                &format!("{} gelen devreden PEK", id),
            )?;
            let sonraki_devreden_pek = Self::parse_optional_json(
                sonraki_devreden_pek_json.as_deref(),
                &format!("{} sonraki devreden PEK", id),
            )?;
            let is_primi_detay = Self::parse_optional_json(
                is_primi_snapshot_json.as_deref(),
                &format!("{} iş primi snapshot'ı", id),
            )?;
            let gv_detay = Self::parse_optional_json(
                gv_snapshot_json.as_deref(),
                &format!("{} GV snapshot'ı", id),
            )?;
            let gelirler = income_by_payroll.remove(&id).unwrap_or_default();
            let kesintiler = deductions_by_payroll.remove(&id).unwrap_or_default();

            result.push(BordroKaydi {
                id,
                personelId: personnel_id,
                donemId: period_id,
                puantajOzeti: puantaj_summary,
                gelirler,
                gelirToplam: kurus_to_dec(gross_total),
                kesintiler,
                kesintiToplam: kurus_to_dec(total_deductions),
                netOdeme: kurus_to_dec(net_payment),
                status,
                olusturulmaTarihi: calculated_at,
                sonGuncellemeTarihi: updated_at,
                notlar,
                oncekiKumulatifGvMatrahi: Some(kurus_to_dec(previous_cumulative_gv)),
                oncekiKumulatifAsgariGvMatrahi: gv_detay.as_ref().map(|g: &GvHesapDetayi| {
                    (g.asgariUcretReferansKumulatifMatrahi - g.asgariUcretGvMatrahi)
                        .max(rust_decimal_macros::dec!(0))
                }),
                manuelKumulatifGvMatrahi: None,
                devredenPekGelen: devreden_pek_gelen,
                sonrakiDevredenPek: sonraki_devreden_pek,
                pekDetay: pek_detay,
                isPrimiDetay: is_primi_detay,
                gvDetay: gv_detay,
                odenenRaporluGun: odenen_raporlu_gun,
                raporluGun: raporlu_gun,
            });
        }

        Ok(result)
    }

    pub fn save(conn: &Connection, bordro: &BordroKaydi) -> Result<()> {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some((current_status, _)) =
            Self::get_status_and_created_at(&tx, &bordro.personelId, &bordro.donemId)?
        {
            if current_status == BordroStatus::FINALIZED {
                return Err(DomainError::PayrollFinalized(
                    "Kesinleştirilmiş (FINALIZED) bordro değiştirilemez.".into(),
                ));
            }
        }

        if Self::has_later_finalized_by_work_period(&tx, &bordro.personelId, &bordro.donemId)? {
            return Err(DomainError::PayrollFinalized(
                "Bu bordro değiştirilemez: daha sonraki bir bordro FINALIZED durumunda ve bu bordronun bağımlılık zincirine dayanıyor.".into(),
            ));
        }

        let before = Self::dependency_fingerprint(&tx, &bordro.personelId, &bordro.donemId)?;
        Self::save_in_transaction(&tx, bordro)?;
        let after = Self::dependency_fingerprint(&tx, &bordro.personelId, &bordro.donemId)?;

        if before != after {
            Self::mark_later_calculated_stale(&tx, &bordro.personelId, &bordro.donemId)?;
        }

        tx.commit()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    /// MigrationService transaction'ı içinden çağrılır; yeni transaction açmaz.
    /// Bulk restore/import snapshot'ları olduğu gibi korur; production dependency
    /// invalidation `save()` giriş noktasında uygulanır.
    pub fn save_in_transaction(conn: &Connection, b: &BordroKaydi) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let status_str = Self::status_to_str(b.status);

        let gross_total = dec_to_kurus(Some(b.gelirToplam));
        let sgk_base = dec_to_kurus(b.pekDetay.as_ref().map(|p| p.finalPek));
        let isci_sgk = b.kesintiler.isciSgkPrimi.unwrap_or_default();
        let isci_issizlik = b.kesintiler.isciIssizlikPrimi.unwrap_or_default();
        // Production bordrosunda authoritative GV matrahı, hesap sırasında oluşturulan
        // GvHesapDetayi snapshot'ıdır. Eski kayıt/migration yolları için snapshot yoksa
        // geriye dönük yalın formül fallback olarak korunur.
        let gv_base_decimal = b
            .gvDetay
            .as_ref()
            .map(|g| g.cariGvMatrahi)
            .unwrap_or_else(|| (b.gelirToplam - isci_sgk - isci_issizlik).max(Decimal::ZERO));
        let gv_base = dec_to_kurus(Some(gv_base_decimal));
        let prev_gv = dec_to_kurus(b.oncekiKumulatifGvMatrahi);
        let new_gv = prev_gv + gv_base;
        let income_tax = dec_to_kurus(b.kesintiler.gelirVergisi);
        let stamp_tax = dec_to_kurus(b.kesintiler.damgaVergisi);
        let total_deductions = dec_to_kurus(Some(b.kesintiToplam));
        let net_payment = dec_to_kurus(Some(b.netOdeme));

        let puantaj_summary_json = Self::serialize_json(&b.puantajOzeti, "Puantaj özeti")?;
        let pek_detail_json = Self::serialize_optional_json(b.pekDetay.as_ref(), "PEK detayı")?;
        let devreden_pek_gelen_json =
            Self::serialize_optional_json(b.devredenPekGelen.as_ref(), "Gelen devreden PEK")?;
        let sonraki_devreden_pek_json =
            Self::serialize_optional_json(b.sonrakiDevredenPek.as_ref(), "Sonraki devreden PEK")?;
        let is_primi_snapshot_json =
            Self::serialize_optional_json(b.isPrimiDetay.as_ref(), "İş primi snapshot'ı")?;
        let gv_snapshot_json = Self::serialize_optional_json(b.gvDetay.as_ref(), "GV snapshot'ı")?;

        conn.execute(
            "INSERT INTO payroll_records (
                id, personnel_id, period_id, gross_total, sgk_base, gv_base, previous_cumulative_gv,
                new_cumulative_gv, income_tax, stamp_tax, total_deductions, net_payment, status,
                puantaj_summary_json, pek_detail_json, devreden_pek_gelen_json, sonraki_devreden_pek_json,
                raporlu_gun, odenen_raporlu_gun, is_primi_snapshot_json, gv_snapshot_json, notlar,
                calculated_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
             ON CONFLICT(personnel_id, period_id) DO UPDATE SET
                gross_total=?4, sgk_base=?5, gv_base=?6, previous_cumulative_gv=?7, new_cumulative_gv=?8,
                income_tax=?9, stamp_tax=?10, total_deductions=?11, net_payment=?12, status=?13,
                puantaj_summary_json=?14, pek_detail_json=?15, devreden_pek_gelen_json=?16,
                sonraki_devreden_pek_json=?17, raporlu_gun=?18, odenen_raporlu_gun=?19,
                is_primi_snapshot_json=?20, gv_snapshot_json=?21, notlar=?22, updated_at=?24",
            params![
                b.id,
                b.personelId,
                b.donemId,
                gross_total,
                sgk_base,
                gv_base,
                prev_gv,
                new_gv,
                income_tax,
                stamp_tax,
                total_deductions,
                net_payment,
                status_str,
                puantaj_summary_json,
                pek_detail_json,
                devreden_pek_gelen_json,
                sonraki_devreden_pek_json,
                b.raporluGun,
                b.odenenRaporluGun,
                is_primi_snapshot_json,
                gv_snapshot_json,
                b.notlar,
                now,
                now,
            ],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        conn.execute(
            "DELETE FROM payroll_income_items WHERE payroll_id = ?1",
            params![b.id],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let income_map: [(&str, Option<Decimal>); 13] = [
            ("tabanBrutAylik", b.gelirler.tabanBrutAylik),
            ("tediye", b.gelirler.tediye),
            ("tisIkramiyesi", b.gelirler.tisIkramiyesi),
            ("ekOdeme", b.gelirler.ekOdeme),
            ("yemek", b.gelirler.yemek),
            (
                "birlestirilmisSosyalYardim",
                b.gelirler.birlestirilmisSosyalYardim,
            ),
            ("vasitaYol", b.gelirler.vasitaYol),
            ("giyimYardimi", b.gelirler.giyimYardimi),
            ("isPrimi", b.gelirler.isPrimi),
            ("geceCalismasiUcreti", b.gelirler.geceCalismasiUcreti),
            (
                "geceCalismasiTatiliUcreti",
                b.gelirler.geceCalismasiTatiliUcreti,
            ),
            ("hizmetZammi", b.gelirler.hizmetZammi),
            ("digerGelir", b.gelirler.digerGelir),
        ];
        for (item_type, amount) in income_map
            .into_iter()
            .filter_map(|(kind, value)| value.map(|v| (kind, v)))
        {
            let source = if item_type == "tediye" || item_type == "tisIkramiyesi" {
                "MANUAL"
            } else {
                "CALCULATED"
            };
            conn.execute(
                "INSERT INTO payroll_income_items (id, payroll_id, item_type, description, amount, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    format!("{}_{}", b.id, item_type),
                    b.id,
                    item_type,
                    item_type,
                    dec_to_kurus(Some(amount)),
                    source,
                ],
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        }

        conn.execute(
            "DELETE FROM payroll_deduction_items WHERE payroll_id = ?1",
            params![b.id],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let deduction_map: [(&str, Option<Decimal>); 11] = [
            ("isciSgkPrimi", b.kesintiler.isciSgkPrimi),
            ("isciIssizlikPrimi", b.kesintiler.isciIssizlikPrimi),
            ("gelirVergisi", b.kesintiler.gelirVergisi),
            ("damgaVergisi", b.kesintiler.damgaVergisi),
            ("sendikaAidati", b.kesintiler.sendikaAidati),
            ("bes", b.kesintiler.bes),
            ("icra", b.kesintiler.icra),
            ("kisiBorcu", b.kesintiler.kisiBorcu),
            (
                "dogumAskerlikBorclanmasi",
                b.kesintiler.dogumAskerlikBorclanmasi,
            ),
            ("hayatSaglikSigortasi", b.kesintiler.hayatSaglikSigortasi),
            ("digerKesinti", b.kesintiler.digerKesinti),
        ];
        for (item_type, amount) in deduction_map
            .into_iter()
            .filter_map(|(kind, value)| value.map(|v| (kind, v)))
        {
            conn.execute(
                "INSERT INTO payroll_deduction_items (id, payroll_id, item_type, description, amount, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    format!("{}_{}", b.id, item_type),
                    b.id,
                    item_type,
                    item_type,
                    dec_to_kurus(Some(amount)),
                    "CALCULATED",
                ],
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }
}