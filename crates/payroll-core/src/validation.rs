//! Shared financial input invariants for persistence and both calculation runtimes.
use crate::{models::*, DomainError, Result};
use rust_decimal::Decimal;

/// Ordinary payroll line items are positive monetary facts.  Signed
/// entitlement/receivable deltas live in the retro ledger and deliberately do
/// not pass through this validator.
pub fn validate_ordinary_payroll_line_items(
    gelirler: &GelirKalemleri,
    kesintiler: &KesintiKalemleri,
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
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(DomainError::ValidationError(format!(
                "Ordinary gelir kalemi negatif olamaz: {}.",
                field
            )));
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
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(DomainError::ValidationError(format!(
                "Ordinary kesinti kalemi negatif olamaz: {}.",
                field
            )));
        }
    }

    Ok(())
}

pub fn validate_ordinary_payroll_snapshot(payroll: &BordroKaydi) -> Result<()> {
    // A RETRO payment may carry a signed source-month premium adjustment in
    // its payment snapshot.  That signed ledger is validated by the retro
    // allocation policy below; ordinary NORMAL/supplementary line items keep
    // the non-negative invariant.
    if payroll.accrualType != AccrualType::RETRO_ADJUSTMENT {
        validate_ordinary_payroll_line_items(&payroll.gelirler, &payroll.kesintiler)?;
    }
    if let Some(pek) = payroll.pekDetay.as_ref() {
        for (field, value) in [
            ("aylikOncekiPekTuketimi", pek.aylikOncekiPekTuketimi),
            ("aylikSonrasiPekTuketimi", pek.aylikSonrasiPekTuketimi),
        ] {
            if value.is_some_and(|amount| amount < Decimal::ZERO) {
                return Err(DomainError::ValidationError(format!(
                    "{} monthly PEK state'i negatif olamaz.",
                    field
                )));
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
