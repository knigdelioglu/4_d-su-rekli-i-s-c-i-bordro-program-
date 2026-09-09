//! Shared financial input invariants for persistence and both calculation runtimes.
use crate::{models::*, DomainError, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;

pub const VALID_ATTENDANCE_CODES: [&'static str; 7] = ["Ç", "T", "G", "İ", "GÇ", "GÇT", "R"];

pub fn validate_monetary_amount(field: &str, amount: Decimal) -> Result<()> {
    if amount.normalize().scale() > 2 {
        return Err(DomainError::ValidationError(format!(
            "Parasal alan ({}) 2 ondalık basamaktan (kuruş) daha fazla hassasiyet içeremez: {}.",
            field, amount
        )));
    }
    Ok(())
}

/// Ordinary payroll line items are positive monetary facts.
pub fn validate_ordinary_payroll_line_items(
    gelirler: &GelirKalemleri,
    kesintiler: &KesintiKalemleri,
) -> Result<()> {
    validate_payroll_line_items_internal(gelirler, kesintiler, false)
}

fn validate_payroll_line_items_internal(
    gelirler: &GelirKalemleri,
    kesintiler: &KesintiKalemleri,
    is_retro: bool,
) -> Result<()> {
    let income = [
        ("tabanBrutAylik", gelirler.tabanBrutAylik),
        ("tediye", gelirler.tediye),
        ("tisIkramiyesi", gelirler.tisIkramiyesi),
        ("ekOdeme", gelirler.ekOdeme),
        ("yemek", gelirler.yemek),
        ("birlestirilmisSosyalYardim", gelirler.birlestirilmisSosyalYardim),
        ("vasitaYol", gelirler.vasitaYol),
        ("giyimYardimi", gelirler.giyimYardimi),
        ("isPrimi", gelirler.isPrimi),
        ("geceCalismasiUcreti", gelirler.geceCalismasiUcreti),
        ("geceCalismasiTatiliUcreti", gelirler.geceCalismasiTatiliUcreti),
        ("hizmetZammi", gelirler.hizmetZammi),
        ("digerGelir", gelirler.digerGelir),
    ];
    for (field, value) in income {
        if let Some(amount) = value {
            validate_monetary_amount(field, amount)?;
            if amount < Decimal::ZERO {
                return Err(DomainError::ValidationError(format!(
                    "Ordinary gelir kalemi negatif olamaz: {}.",
                    field
                )));
            }
        }
    }

    let deductions = [
        ("isciSgkPrimi", kesintiler.isciSgkPrimi),
        ("isciIssizlikPrimi", kesintiler.isciIssizlikPrimi),
        ("gelirVergisi", kesintiler.gelirVergisi),
        ("damgaVergisi", kesintiler.damgaVergisi),
        ("sendikaAidati", kesintiler.sendikaAidati),
        ("bes", kesintiler.bes),
        ("icra", kesintiler.icra),
        ("kisiBorcu", kesintiler.kisiBorcu),
        ("dogumAskerlikBorclanmasi", kesintiler.dogumAskerlikBorclanmasi),
        ("hayatSaglikSigortasi", kesintiler.hayatSaglikSigortasi),
        ("digerKesinti", kesintiler.digerKesinti),
    ];
    for (field, value) in deductions {
        if let Some(amount) = value {
            validate_monetary_amount(field, amount)?;
            if amount < Decimal::ZERO {
                // In RETRO, only worker SGK / unemployment can carry a signed adjustment delta
                if is_retro && (field == "isciSgkPrimi" || field == "isciIssizlikPrimi") {
                    continue;
                }
                return Err(DomainError::ValidationError(format!(
                    "Ordinary kesinti kalemi negatif olamaz: {}.",
                    field
                )));
            }
        }
    }

    Ok(())
}

pub fn validate_ordinary_payroll_snapshot(payroll: &BordroKaydi) -> Result<()> {
    let is_retro = payroll.accrualType == AccrualType::RETRO_ADJUSTMENT;
    validate_payroll_line_items_internal(&payroll.gelirler, &payroll.kesintiler, is_retro)?;
    for (field, value) in [
        ("gelirToplam", payroll.gelirToplam),
        ("kesintiToplam", payroll.kesintiToplam),
        ("netOdeme", payroll.netOdeme),
    ] {
        validate_monetary_amount(field, value)?;
    }
    if let Some(pek) = payroll.pekDetay.as_ref() {
        for (field, value) in [
            ("aylikOncekiPekTuketimi", pek.aylikOncekiPekTuketimi),
            ("aylikSonrasiPekTuketimi", pek.aylikSonrasiPekTuketimi),
            ("hesaplananPek", Some(pek.hesaplananPek)),
            ("hamPek", Some(pek.hamPek)),
            ("devredenPekKullanilan", Some(pek.devredenPekKullanilan)),
            ("primMatrahi", Some(pek.primMatrahi)),
            ("finalPek", Some(pek.finalPek)),
            ("devredenPekAşanTutar", Some(pek.devredenPekAşanTutar)),
            ("pekAltSinir", Some(pek.pekAltSinir)),
            ("pekUstSinir", Some(pek.pekUstSinir)),
            ("altSinirTamamlamaFarki", Some(pek.altSinirTamamlamaFarki)),
            ("yemekIstisnasiTutar", Some(pek.yemekIstisnasiTutar)),
            ("isverenSgkPrimi", pek.isverenSgkPrimi),
            ("isverenIssizlikPrimi", pek.isverenIssizlikPrimi),
            (
                "pekAltSinirTamamlamaIsverenPrimi",
                pek.pekAltSinirTamamlamaIsverenPrimi,
            ),
            ("isverenPrimToplami", pek.isverenPrimToplami),
        ] {
            if let Some(amount) = value {
                validate_monetary_amount(field, amount)?;
                if amount < Decimal::ZERO {
                    return Err(DomainError::ValidationError(format!(
                        "{} snapshot değeri negatif olamaz.",
                        field
                    )));
                }
            }
        }
    }
    if let Some(devreden) = payroll.devredenPekGelen.as_ref() {
        for record in devreden {
            validate_monetary_amount("devredenPekGelen.tutar", record.tutar)?;
        }
    }
    if let Some(devreden) = payroll.sonrakiDevredenPek.as_ref() {
        for record in devreden {
            validate_monetary_amount("sonrakiDevredenPek.tutar", record.tutar)?;
        }
    }
    Ok(())
}

pub fn validate_attendance_for_period(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
) -> Result<()> {
    if !attendance.donemId.is_empty() && attendance.donemId != period.id {
        return Err(DomainError::ValidationError(format!(
            "Puantaj dönem kimliği '{}' ile bordro dönemi '{}' eşleşmiyor.",
            attendance.donemId, period.id
        )));
    }

    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").map_err(|_| {
        DomainError::ValidationError(format!(
            "{} dönemi başlangıç tarihi geçersiz: {}",
            period.id, period.baslangicTarihi
        ))
    })?;
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").map_err(|_| {
        DomainError::ValidationError(format!(
            "{} dönemi bitiş tarihi geçersiz: {}",
            period.id, period.bitisTarihi
        ))
    })?;

    if start > end {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi başlangıç tarihi bitiş tarihinden sonra olamaz: {} > {}",
            period.id, period.baslangicTarihi, period.bitisTarihi
        )));
    }

    let calendar_day_count = (end - start).num_days() + 1;
    if attendance.gunler.len() as i64 > calendar_day_count {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi {} takvim günü içeriyor ancak puantajda {} kayıt var.",
            period.id,
            calendar_day_count,
            attendance.gunler.len()
        )));
    }

    for (date_text, code) in &attendance.gunler {
        if !VALID_ATTENDANCE_CODES.contains(&code.as_str()) {
            return Err(DomainError::ValidationError(format!(
                "{} döneminde desteklenmeyen puantaj kodu: {} (tarih: {})",
                period.id, code, date_text
            )));
        }

        let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError(format!(
                "{} döneminde puantaj tarihi YYYY-MM-DD biçiminde geçerli bir tarih olmalıdır: {}",
                period.id, date_text
            ))
        })?;

        if date < start || date > end {
            return Err(DomainError::ValidationError(format!(
                "{} puantaj tarihi {} döneminin {}–{} aralığı dışında.",
                date_text, period.id, period.baslangicTarihi, period.bitisTarihi
            )));
        }
    }

    Ok(())
}

pub fn validate_sick_leave_records(records: &[SickLeaveRecord]) -> Result<()> {
    for record in records {
        let start = NaiveDate::parse_from_str(&record.startDate, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError(format!(
                "Rapor başlangıç tarihi geçersiz: {}.",
                record.startDate
            ))
        })?;
        let end = NaiveDate::parse_from_str(&record.endDate, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError(format!(
                "Rapor bitiş tarihi geçersiz: {}.",
                record.endDate
            ))
        })?;
        if start > end {
            return Err(DomainError::ValidationError(format!(
                "Rapor başlangıç tarihi bitiş tarihinden sonra olamaz: {} > {}.",
                record.startDate, record.endDate
            )));
        }
        if record.personnelId.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "Rapor kaydında personel zorunludur.".into(),
            ));
        }
    }

    for i in 0..records.len() {
        for j in (i + 1)..records.len() {
            let a = &records[i];
            let b = &records[j];
            if a.personnelId == b.personnelId && a.id != b.id {
                let start_a = NaiveDate::parse_from_str(&a.startDate, "%Y-%m-%d").unwrap();
                let end_a = NaiveDate::parse_from_str(&a.endDate, "%Y-%m-%d").unwrap();
                let start_b = NaiveDate::parse_from_str(&b.startDate, "%Y-%m-%d").unwrap();
                let end_b = NaiveDate::parse_from_str(&b.endDate, "%Y-%m-%d").unwrap();
                if start_a <= end_b && end_a >= start_b {
                    return Err(DomainError::ValidationError(format!(
                        "Rapor tarihleri çakışıyor: {}–{} aralığı, {} kaydındaki {}–{} aralığıyla örtüşüyor. Örtüşen raporlar ayrı episode olarak kaydedilemez.",
                        a.startDate, a.endDate, b.id, b.startDate, b.endDate
                    )));
                }
            }
        }
    }

    Ok(())
}

/// A current snapshot may carry the SQLite/replay GV base alongside the rich
/// GV detail.  When both are present they are two representations of one
/// authority and must be identical; sparse legacy records may carry either
/// representation but never an approximate replacement at runtime.
pub fn validate_gv_base_reconciliation(payroll: &BordroKaydi) -> Result<()> {
    if payroll
        .persistedGvBase
        .is_some_and(|value| value < Decimal::ZERO)
    {
        return Err(DomainError::ValidationError(format!(
            "{} persisted GV matrahı negatif olamaz.",
            payroll.accrualId
        )));
    }
    if let (Some(persisted), Some(detail)) = (payroll.persistedGvBase, payroll.gvDetay.as_ref()) {
        if persisted != detail.cariGvMatrahi {
            return Err(DomainError::InvalidData(format!(
                "{} persisted GV matrahı ({}) ile GV snapshot cari matrahı ({}) eşleşmiyor.",
                payroll.accrualId, persisted, detail.cariGvMatrahi
            )));
        }
    }
    Ok(())
}

pub fn validate_payroll_snapshot_authority(payroll: &BordroKaydi) -> Result<()> {
    validate_ordinary_payroll_snapshot(payroll)?;
    validate_gv_base_reconciliation(payroll)
}

/// Strict contract for a current-format authoritative backup. Sparse legacy
/// rows use `validate_payroll_snapshot_authority` plus the explicit migration
/// path; they must not be promoted to this contract merely because their JSON
/// shape can be deserialized as `BordroKaydi`.
pub fn validate_current_payroll_snapshot_authority(payroll: &BordroKaydi) -> Result<()> {
    validate_payroll_snapshot_authority(payroll)?;
    if matches!(
        payroll.status,
        BordroStatus::CALCULATED | BordroStatus::FINALIZED
    ) {
        let persisted = payroll.persistedGvBase.ok_or_else(|| {
            DomainError::InvalidData(
                "Current authoritative snapshot persisted GV matrahını içermiyor.".into(),
            )
        })?;
        let detail = payroll.gvDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(
                "Current authoritative snapshot GV detayını içermiyor.".into(),
            )
        })?;
        if persisted != detail.cariGvMatrahi {
            return Err(DomainError::InvalidData(
                "Current authoritative snapshot persisted GV matrahı ile cari GV matrahı eşleşmiyor."
                    .into(),
            ));
        }
    }
    Ok(())
}

pub fn validate_personnel_for_payroll(personel: &Personel) -> Result<()> {
    if personel.hizmetYili < 0 {
        return Err(DomainError::ValidationError(
            "Hizmet yılı negatif olamaz.".into(),
        ));
    }
    if personel
        .devirKumulatifGvMatrahi
        .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
        || personel
            .devirKumulatifAsgariGvMatrahi
            .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
    {
        return Err(DomainError::ValidationError(
            "Devir kümülatif GV matrahları negatif olamaz.".into(),
        ));
    }

    if personel
        .devirKumulatifGvMatrahiBaslangicAyi
        .is_some_and(|month| !(1..=12).contains(&month))
    {
        return Err(DomainError::ValidationError(
            "GV devir başlangıç ayı 1-12 arasında olmalıdır.".into(),
        ));
    }

    if let Some(k) = personel.kesintiler.as_ref() {
        let monetary = [
            ("sabitSendikaAidati", k.sabitSendikaAidati),
            ("sabitBesTutar", k.sabitBesTutar),
            ("icraTutar", k.icraTutar),
            ("kisiBorcuTutar", k.kisiBorcuTutar),
            (
                "dogumAskerlikBorclanmasiTutar",
                k.dogumAskerlikBorclanmasiTutar,
            ),
            ("hayatSaglikSigortasiTutar", k.hayatSaglikSigortasiTutar),
            ("digerKesintiTutar", k.digerKesintiTutar),
        ];
        for (field, value) in monetary {
            if value.is_some_and(|amount| amount < rust_decimal::Decimal::ZERO) {
                return Err(DomainError::ValidationError(format!(
                    "Personel kesinti tutarı negatif olamaz: {}.",
                    field
                )));
            }
        }

        if k.besUyesi.unwrap_or(false) {
            if let Some(rate) = k.oksOraniYuzde {
                if rate < rust_decimal_macros::dec!(3) || rate > rust_decimal_macros::dec!(100) {
                    return Err(DomainError::ValidationError(
                        "OKS özel oranı, OKS'ye tabi personelde %3-%100 arasında olmalıdır.".into(),
                    ));
                }
            }
        }

        if let Some(gv) = k.gvIndirimleri.as_ref() {
            for (field, value) in [
                (
                    "dogumAskerlikGvIndirimTutar",
                    gv.dogumAskerlikGvIndirimTutar,
                ),
                ("hayatSigortasiPrimiTutar", gv.hayatSigortasiPrimiTutar),
                ("saglikSigortasiPrimiTutar", gv.saglikSigortasiPrimiTutar),
            ] {
                if value.is_some_and(|amount| amount < rust_decimal::Decimal::ZERO) {
                    return Err(DomainError::ValidationError(format!(
                        "GV indirim girdisi negatif olamaz: {}.",
                        field
                    )));
                }
            }
        }
    }

    Ok(())
}

pub fn validate_annual_payroll_parameters(parameters: &AnnualPayrollParameters) -> Result<()> {
    if parameters.year <= 0 || parameters.gelirVergisiDilimleri.is_empty() {
        return Err(DomainError::ValidationError(
            "Yıllık bordro parametresi geçerli bir yıl ve en az bir vergi dilimi içermelidir."
                .into(),
        ));
    }

    if parameters
        .sigortaGvYillikBrutAsgariUcretTavani
        .is_some_and(|value| value <= Decimal::ZERO)
    {
        return Err(DomainError::ValidationError(
            "Sigorta GV yıllık tavanı sıfırdan büyük olmalıdır.".into(),
        ));
    }

    let mut previous_limit = rust_decimal_macros::dec!(0);
    let max_persisted_limit = Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT);
    for TaxBracket { limit, oran } in &parameters.gelirVergisiDilimleri {
        if *limit <= previous_limit
            || *limit > max_persisted_limit
            || *oran < rust_decimal_macros::dec!(0)
            || *oran > rust_decimal_macros::dec!(1)
        {
            return Err(DomainError::ValidationError(
                "Yıllık gelir vergisi dilimleri artan, SQLite'a sığan limitlere ve 0-1 arası oranlara sahip olmalıdır."
                    .into(),
            ));
        }
        previous_limit = *limit;
    }

    // The last bracket is semantically open-ended in the calculation
    // engine. Its persistence boundary must remain finite and safe for the
    // serde-float/SQLite JSON contract; Decimal::MAX is not a valid value.
    let last_limit = parameters
        .gelirVergisiDilimleri
        .last()
        .map(|bracket| bracket.limit)
        .ok_or_else(|| {
            DomainError::ValidationError(
                "Yıllık gelir vergisi parametresinin son dilimi zorunludur.".into(),
            )
        })?;
    if last_limit > max_persisted_limit {
        return Err(DomainError::ValidationError(
            "Yıllık gelir vergisi son dilim sınırı SQLite-safe üst sınırı aşamaz.".into(),
        ));
    }

    Ok(())
}
