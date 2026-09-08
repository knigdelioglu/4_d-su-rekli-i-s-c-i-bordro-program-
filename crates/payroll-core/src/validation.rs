//! Shared financial input invariants for persistence and both calculation runtimes.
use crate::{models::*, DomainError, Result};
use rust_decimal::Decimal;

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
