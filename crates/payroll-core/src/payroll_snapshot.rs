//! Typed validation for persisted payroll calculation snapshots.
//!
//! This module validates snapshot identity, versions, coverage, and captured
//! inputs. It does not run a payroll calculation or consult live dataset state.

use crate::models::*;
use crate::{
    calculate_paid_sick_dates_from_records, parse_canonical_work_date, validate_period,
    validate_tax_month_overlap,
};
use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use std::collections::HashSet;
use thiserror::Error;

pub const PAYROLL_SNAPSHOT_SCHEMA_VERSION: u16 = 2;
pub const PAYROLL_PREVIOUS_CALCULATION_VERSION: &str = "compensation-engine-v1";
pub const PAYROLL_CALCULATION_VERSION: &str = "compensation-engine-v2";
pub const PAID_SICK_LEAVE_RULE_VERSION: &str =
    "first-two-days-first-five-episodes-by-start-year-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayrollCalculationSnapshotDisposition {
    /// Missing metadata is the explicit compatibility representation of a
    /// legacy record. No current-engine requirements are inferred.
    Legacy,
    /// Current-engine DRAFT or non-NORMAL records may exist without a snapshot.
    SnapshotNotRequiredYet,
    /// A current NORMAL snapshot passed the typed contract checks.
    CurrentSnapshotValidated,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PayrollCalculationSnapshotError {
    #[error("Current NORMAL CALCULATED/FINALIZED payroll is missing its calculation snapshot.")]
    MissingRequiredSnapshot,
    #[error("Unsupported payroll snapshot schema version: {0}.")]
    UnsupportedSchemaVersion(u16),
    #[error("Unsupported payroll calculation version: {0}.")]
    UnsupportedCalculationVersion(String),
    #[error("Unsupported paid-sick-leave rule version: {0}.")]
    UnsupportedPaidSickLeaveRuleVersion(String),
    #[error("Payroll and period identity do not match: {0}.")]
    IdentityMismatch(String),
    #[error("Invalid payroll period/payment context: {0}.")]
    InvalidContext(String),
    #[error("Invalid compensation segment at index {0}: {1}.")]
    InvalidSegment(usize, String),
    #[error("Compensation segment coverage has a gap before {0}.")]
    SegmentGap(String),
    #[error("Compensation segments overlap at {0}.")]
    SegmentOverlap(String),
    #[error("Compensation segments are out of order at {0}.")]
    SegmentOutOfOrder(String),
    #[error("Compensation segment is outside the service period at {0}.")]
    SegmentOutOfPeriod(String),
    #[error("Required current-engine input is missing or inconsistent: {0}.")]
    MissingRequiredInput(String),
    #[error("Invalid attendance snapshot: {0}.")]
    InvalidAttendance(String),
    #[error("Paid sick-leave dates do not match the frozen episode inputs: {0}.")]
    PaidSickLeaveMismatch(String),
    #[error("Invalid prior payment/tax history: {0}.")]
    InvalidTaxHistory(String),
    #[error("Invalid incoming PEK carry snapshot: {0}.")]
    InvalidPekCarry(String),
    #[error("Canonical financial replay mismatch: {0}.")]
    FinancialProjectionMismatch(String),
}

type SnapshotResult<T> = std::result::Result<T, PayrollCalculationSnapshotError>;

/// Validates one record at its parent period boundary. The BordroKaydi owns
/// personnel/accrual identity; BordroDonemi owns service and tax-period facts.
pub fn validate_payroll_calculation_metadata(
    payroll: &BordroKaydi,
    period: &BordroDonemi,
) -> SnapshotResult<PayrollCalculationSnapshotDisposition> {
    validate_payroll_calculation_metadata_with_context(payroll, period, &[], &[])
}

/// Validates a schema-2 record using the frozen inputs on the record and the
/// saved payment-event rows only for matching event provenance. Amounts used
/// by replay remain those embedded in the NORMAL snapshot.
pub fn validate_payroll_calculation_metadata_with_context(
    payroll: &BordroKaydi,
    period: &BordroDonemi,
    source_payrolls: &[BordroKaydi],
    source_periods: &[BordroDonemi],
) -> SnapshotResult<PayrollCalculationSnapshotDisposition> {
    let disposition = validate_payroll_calculation_metadata_structure(payroll, period)?;
    if disposition == PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
        && matches!(
            payroll.status,
            BordroStatus::CALCULATED | BordroStatus::FINALIZED
        )
    {
        let replayed = crate::payroll_engine::replay_saved_normal_financial_projection(
            payroll,
            period,
            source_payrolls,
            source_periods,
        )
        .map_err(|error| {
            PayrollCalculationSnapshotError::MissingRequiredInput(format!(
                "canonical financial replay source: {error}"
            ))
        })?;
        compare_financial_projection(payroll, &replayed)?;
    }
    Ok(disposition)
}

/// Structural checks used while the engine is creating a new result. Restore,
/// save, and historical replay boundaries must call the public validator.
/// Checks only schema shape, provenance links, and saved-input consistency.
/// This is not an acceptance validator for CALCULATED/FINALIZED records; use
/// [`validate_payroll_calculation_metadata`] at save, import, and restore.
#[doc(hidden)]
pub fn validate_payroll_calculation_metadata_structure(
    payroll: &BordroKaydi,
    period: &BordroDonemi,
) -> SnapshotResult<PayrollCalculationSnapshotDisposition> {
    let Some(metadata) = payroll.calculationMetadata.as_ref() else {
        return Ok(PayrollCalculationSnapshotDisposition::Legacy);
    };

    let Some(snapshot) = metadata.snapshot.as_ref() else {
        if payroll.accrualType == AccrualType::NORMAL
            && matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
        {
            return Err(PayrollCalculationSnapshotError::MissingRequiredSnapshot);
        }
        return Ok(PayrollCalculationSnapshotDisposition::SnapshotNotRequiredYet);
    };

    validate_parent_context(payroll, period)?;
    if payroll.accrualType != AccrualType::NORMAL {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "supported compensation-engine versions currently define NORMAL accruals only".into(),
        ));
    }

    if snapshot.snapshotSchemaVersion != PAYROLL_SNAPSHOT_SCHEMA_VERSION {
        return Err(PayrollCalculationSnapshotError::UnsupportedSchemaVersion(
            snapshot.snapshotSchemaVersion,
        ));
    }
    if snapshot.calculationVersion != PAYROLL_CALCULATION_VERSION
        && snapshot.calculationVersion != PAYROLL_PREVIOUS_CALCULATION_VERSION
    {
        return Err(
            PayrollCalculationSnapshotError::UnsupportedCalculationVersion(
                snapshot.calculationVersion.clone(),
            ),
        );
    }

    validate_compensation_segments(&snapshot.compensation, period)?;
    validate_used_inputs(&snapshot.usedInputs, payroll, period)?;
    validate_resolved_statutory_snapshot(payroll, period)?;

    if matches!(
        payroll.status,
        BordroStatus::CALCULATED | BordroStatus::FINALIZED
    ) {
        for (field, present) in [
            ("isPrimiDetay", payroll.isPrimiDetay.is_some()),
            ("gvDetay", payroll.gvDetay.is_some()),
            ("pekDetay", payroll.pekDetay.is_some()),
            ("damgaDetay", payroll.damgaDetay.is_some()),
            ("persistedGvBase", payroll.persistedGvBase.is_some()),
        ] {
            if !present {
                return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                    format!("current NORMAL output {field}"),
                ));
            }
        }
        crate::validate_current_payroll_snapshot_authority(payroll)
            .map_err(|error| invalid_context("current payroll result", error))?;
    }

    Ok(PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated)
}

fn compare_projection_value<T: PartialEq + std::fmt::Debug>(
    field: &str,
    saved: &T,
    replayed: &T,
) -> SnapshotResult<()> {
    if saved != replayed {
        return Err(
            PayrollCalculationSnapshotError::FinancialProjectionMismatch(format!(
                "{field}: saved {saved:?}, replayed {replayed:?}"
            )),
        );
    }
    Ok(())
}

fn compare_financial_projection(saved: &BordroKaydi, replayed: &BordroKaydi) -> SnapshotResult<()> {
    macro_rules! same {
        ($path:expr, $saved:expr, $replayed:expr) => {
            compare_projection_value($path, &$saved, &$replayed)?
        };
    }

    for (field, left, right) in [
        (
            "gelirler.tabanBrutAylik",
            saved.gelirler.tabanBrutAylik,
            replayed.gelirler.tabanBrutAylik,
        ),
        (
            "gelirler.tediye",
            saved.gelirler.tediye,
            replayed.gelirler.tediye,
        ),
        (
            "gelirler.tisIkramiyesi",
            saved.gelirler.tisIkramiyesi,
            replayed.gelirler.tisIkramiyesi,
        ),
        (
            "gelirler.ekOdeme",
            saved.gelirler.ekOdeme,
            replayed.gelirler.ekOdeme,
        ),
        (
            "gelirler.yemek",
            saved.gelirler.yemek,
            replayed.gelirler.yemek,
        ),
        (
            "gelirler.birlestirilmisSosyalYardim",
            saved.gelirler.birlestirilmisSosyalYardim,
            replayed.gelirler.birlestirilmisSosyalYardim,
        ),
        (
            "gelirler.vasitaYol",
            saved.gelirler.vasitaYol,
            replayed.gelirler.vasitaYol,
        ),
        (
            "gelirler.giyimYardimi",
            saved.gelirler.giyimYardimi,
            replayed.gelirler.giyimYardimi,
        ),
        (
            "gelirler.isPrimi",
            saved.gelirler.isPrimi,
            replayed.gelirler.isPrimi,
        ),
        (
            "gelirler.geceCalismasiUcreti",
            saved.gelirler.geceCalismasiUcreti,
            replayed.gelirler.geceCalismasiUcreti,
        ),
        (
            "gelirler.geceCalismasiTatiliUcreti",
            saved.gelirler.geceCalismasiTatiliUcreti,
            replayed.gelirler.geceCalismasiTatiliUcreti,
        ),
        (
            "gelirler.hizmetZammi",
            saved.gelirler.hizmetZammi,
            replayed.gelirler.hizmetZammi,
        ),
        (
            "gelirler.digerGelir",
            saved.gelirler.digerGelir,
            replayed.gelirler.digerGelir,
        ),
    ] {
        same!(field, left, right);
    }
    for (field, left, right) in [
        (
            "kesintiler.isciSgkPrimi",
            saved.kesintiler.isciSgkPrimi,
            replayed.kesintiler.isciSgkPrimi,
        ),
        (
            "kesintiler.isciIssizlikPrimi",
            saved.kesintiler.isciIssizlikPrimi,
            replayed.kesintiler.isciIssizlikPrimi,
        ),
        (
            "kesintiler.gelirVergisi",
            saved.kesintiler.gelirVergisi,
            replayed.kesintiler.gelirVergisi,
        ),
        (
            "kesintiler.damgaVergisi",
            saved.kesintiler.damgaVergisi,
            replayed.kesintiler.damgaVergisi,
        ),
        (
            "kesintiler.sendikaAidati",
            saved.kesintiler.sendikaAidati,
            replayed.kesintiler.sendikaAidati,
        ),
        (
            "kesintiler.bes",
            saved.kesintiler.bes,
            replayed.kesintiler.bes,
        ),
        (
            "kesintiler.icra",
            saved.kesintiler.icra,
            replayed.kesintiler.icra,
        ),
        (
            "kesintiler.nafaka",
            saved.kesintiler.nafaka,
            replayed.kesintiler.nafaka,
        ),
        (
            "kesintiler.kisiBorcu",
            saved.kesintiler.kisiBorcu,
            replayed.kesintiler.kisiBorcu,
        ),
        (
            "kesintiler.dogumAskerlikBorclanmasi",
            saved.kesintiler.dogumAskerlikBorclanmasi,
            replayed.kesintiler.dogumAskerlikBorclanmasi,
        ),
        (
            "kesintiler.hayatSaglikSigortasi",
            saved.kesintiler.hayatSaglikSigortasi,
            replayed.kesintiler.hayatSaglikSigortasi,
        ),
        (
            "kesintiler.digerKesinti",
            saved.kesintiler.digerKesinti,
            replayed.kesintiler.digerKesinti,
        ),
    ] {
        same!(field, left, right);
    }
    same!("gelirToplam", saved.gelirToplam, replayed.gelirToplam);
    same!("kesintiToplam", saved.kesintiToplam, replayed.kesintiToplam);
    same!("netOdeme", saved.netOdeme, replayed.netOdeme);
    same!(
        "oncekiKumulatifGvMatrahi",
        saved.oncekiKumulatifGvMatrahi,
        replayed.oncekiKumulatifGvMatrahi
    );
    same!(
        "oncekiKumulatifAsgariGvMatrahi",
        saved.oncekiKumulatifAsgariGvMatrahi,
        replayed.oncekiKumulatifAsgariGvMatrahi
    );
    same!(
        "manuelKumulatifGvMatrahi",
        saved.manuelKumulatifGvMatrahi,
        replayed.manuelKumulatifGvMatrahi
    );
    same!(
        "persistedGvBase",
        saved.persistedGvBase,
        replayed.persistedGvBase
    );
    same!(
        "odenenRaporluGun",
        saved.odenenRaporluGun,
        replayed.odenenRaporluGun
    );
    same!("raporluGun", saved.raporluGun, replayed.raporluGun);
    same!(
        "devredenPekGelen",
        &saved.devredenPekGelen,
        &replayed.devredenPekGelen
    );
    same!(
        "sonrakiDevredenPek",
        &saved.sonrakiDevredenPek,
        &replayed.sonrakiDevredenPek
    );
    same!(
        "statutorySnapshot",
        &saved.statutorySnapshot,
        &replayed.statutorySnapshot
    );

    match (saved.pekDetay.as_ref(), replayed.pekDetay.as_ref()) {
        (Some(left), Some(right)) => {
            same!(
                "pekDetay.hesaplananPek",
                left.hesaplananPek,
                right.hesaplananPek
            );
            same!("pekDetay.hamPek", left.hamPek, right.hamPek);
            same!(
                "pekDetay.devredenPekKullanilan",
                left.devredenPekKullanilan,
                right.devredenPekKullanilan
            );
            same!("pekDetay.primMatrahi", left.primMatrahi, right.primMatrahi);
            same!(
                "pekDetay.aylikOncekiPekTuketimi",
                left.aylikOncekiPekTuketimi,
                right.aylikOncekiPekTuketimi
            );
            same!(
                "pekDetay.aylikSonrasiPekTuketimi",
                left.aylikSonrasiPekTuketimi,
                right.aylikSonrasiPekTuketimi
            );
            same!("pekDetay.finalPek", left.finalPek, right.finalPek);
            same!(
                "pekDetay.devredenPekAsanTutar",
                left.devredenPekAşanTutar,
                right.devredenPekAşanTutar
            );
            same!("pekDetay.pekAltSinir", left.pekAltSinir, right.pekAltSinir);
            same!("pekDetay.pekUstSinir", left.pekUstSinir, right.pekUstSinir);
            same!(
                "pekDetay.altSinirTamamlamaFarki",
                left.altSinirTamamlamaFarki,
                right.altSinirTamamlamaFarki
            );
            same!(
                "pekDetay.fiiliYemekGunu",
                left.fiiliYemekGunu,
                right.fiiliYemekGunu
            );
            same!(
                "pekDetay.yemekIstisnasiTutar",
                left.yemekIstisnasiTutar,
                right.yemekIstisnasiTutar
            );
            same!(
                "pekDetay.isverenSgkPrimi",
                left.isverenSgkPrimi,
                right.isverenSgkPrimi
            );
            same!(
                "pekDetay.isverenIssizlikPrimi",
                left.isverenIssizlikPrimi,
                right.isverenIssizlikPrimi
            );
            same!(
                "pekDetay.pekAltSinirTamamlamaIsverenPrimi",
                left.pekAltSinirTamamlamaIsverenPrimi,
                right.pekAltSinirTamamlamaIsverenPrimi
            );
            same!(
                "pekDetay.isverenPrimToplami",
                left.isverenPrimToplami,
                right.isverenPrimToplami
            );
            same!(
                "pekDetay.sgkIsverenOraniYuzde",
                left.sgkIsverenOraniYuzde,
                right.sgkIsverenOraniYuzde
            );
            same!(
                "pekDetay.isverenIssizlikOraniYuzde",
                left.isverenIssizlikOraniYuzde,
                right.isverenIssizlikOraniYuzde
            );
        }
        (None, None) => {}
        _ => {
            return Err(
                PayrollCalculationSnapshotError::FinancialProjectionMismatch(
                    "pekDetay presence".into(),
                ),
            )
        }
    }
    match (saved.isPrimiDetay.as_ref(), replayed.isPrimiDetay.as_ref()) {
        (Some(left), Some(right)) => {
            same!("isPrimiDetay.grupId", &left.grupId, &right.grupId);
            same!("isPrimiDetay.grupAd", &left.grupAd, &right.grupAd);
            same!("isPrimiDetay.oran", left.oran, right.oran);
            same!("isPrimiDetay.hakGunu", left.hakGunu, right.hakGunu);
            same!(
                "isPrimiDetay.gunlukIsPrimi",
                left.gunlukIsPrimi,
                right.gunlukIsPrimi
            );
            same!("isPrimiDetay.tutar", left.tutar, right.tutar);
        }
        (None, None) => {}
        _ => {
            return Err(
                PayrollCalculationSnapshotError::FinancialProjectionMismatch(
                    "isPrimiDetay presence".into(),
                ),
            )
        }
    }
    match (saved.gvDetay.as_ref(), replayed.gvDetay.as_ref()) {
        (Some(left), Some(right)) => {
            for (field, a, b) in [
                (
                    "gvDetay.oncekiKumulatifGvMatrahi",
                    left.oncekiKumulatifGvMatrahi,
                    right.oncekiKumulatifGvMatrahi,
                ),
                (
                    "gvDetay.cariGvMatrahi",
                    left.cariGvMatrahi,
                    right.cariGvMatrahi,
                ),
                (
                    "gvDetay.yeniKumulatifGvMatrahi",
                    left.yeniKumulatifGvMatrahi,
                    right.yeniKumulatifGvMatrahi,
                ),
                (
                    "gvDetay.brutGelirVergisi",
                    left.brutGelirVergisi,
                    right.brutGelirVergisi,
                ),
                (
                    "gvDetay.asgariUcretGvMatrahi",
                    left.asgariUcretGvMatrahi,
                    right.asgariUcretGvMatrahi,
                ),
                (
                    "gvDetay.asgariUcretReferansKumulatifMatrahi",
                    left.asgariUcretReferansKumulatifMatrahi,
                    right.asgariUcretReferansKumulatifMatrahi,
                ),
                (
                    "gvDetay.asgariUcretGvIstisnasi",
                    left.asgariUcretGvIstisnasi,
                    right.asgariUcretGvIstisnasi,
                ),
                (
                    "gvDetay.ayniAyOncekiKullanilanGvIstisnasi",
                    left.ayniAyOncekiKullanilanGvIstisnasi,
                    right.ayniAyOncekiKullanilanGvIstisnasi,
                ),
                (
                    "gvDetay.tahakkukOncesiKalanGvIstisnasi",
                    left.tahakkukOncesiKalanGvIstisnasi,
                    right.tahakkukOncesiKalanGvIstisnasi,
                ),
                (
                    "gvDetay.uygulananGvIstisnasi",
                    left.uygulananGvIstisnasi,
                    right.uygulananGvIstisnasi,
                ),
                (
                    "gvDetay.tahakkukSonrasiKalanGvIstisnasi",
                    left.tahakkukSonrasiKalanGvIstisnasi,
                    right.tahakkukSonrasiKalanGvIstisnasi,
                ),
                (
                    "gvDetay.kesilenGelirVergisi",
                    left.kesilenGelirVergisi,
                    right.kesilenGelirVergisi,
                ),
                (
                    "gvDetay.dogumAskerlikGvIndirimi",
                    left.dogumAskerlikGvIndirimi,
                    right.dogumAskerlikGvIndirimi,
                ),
                (
                    "gvDetay.sigortaGvIndirimAdayi",
                    left.sigortaGvIndirimAdayi,
                    right.sigortaGvIndirimAdayi,
                ),
                (
                    "gvDetay.sigortaGvAylikLimiti",
                    left.sigortaGvAylikLimiti,
                    right.sigortaGvAylikLimiti,
                ),
                (
                    "gvDetay.sigortaGvYillikKalanLimiti",
                    left.sigortaGvYillikKalanLimiti,
                    right.sigortaGvYillikKalanLimiti,
                ),
                (
                    "gvDetay.uygulanabilirSigortaGvIndirimi",
                    left.uygulanabilirSigortaGvIndirimi,
                    right.uygulanabilirSigortaGvIndirimi,
                ),
            ] {
                same!(field, a, b);
            }
        }
        (None, None) => {}
        _ => {
            return Err(
                PayrollCalculationSnapshotError::FinancialProjectionMismatch(
                    "gvDetay presence".into(),
                ),
            )
        }
    }
    match (saved.damgaDetay.as_ref(), replayed.damgaDetay.as_ref()) {
        (Some(left), Some(right)) => {
            same!(
                "damgaDetay.brutDamgaVergisi",
                left.brutDamgaVergisi,
                right.brutDamgaVergisi
            );
            same!(
                "damgaDetay.aylikDamgaIstisnaHakki",
                left.aylikDamgaIstisnaHakki,
                right.aylikDamgaIstisnaHakki
            );
            same!(
                "damgaDetay.ayniAyOncekiKullanilanDamgaIstisnasi",
                left.ayniAyOncekiKullanilanDamgaIstisnasi,
                right.ayniAyOncekiKullanilanDamgaIstisnasi
            );
            same!(
                "damgaDetay.uygulananDamgaIstisnasi",
                left.uygulananDamgaIstisnasi,
                right.uygulananDamgaIstisnasi
            );
            same!(
                "damgaDetay.kalanDamgaIstisnasi",
                left.kalanDamgaIstisnasi,
                right.kalanDamgaIstisnasi
            );
            same!(
                "damgaDetay.kesilenDamgaVergisi",
                left.kesilenDamgaVergisi,
                right.kesilenDamgaVergisi
            );
        }
        (None, None) => {}
        _ => {
            return Err(
                PayrollCalculationSnapshotError::FinancialProjectionMismatch(
                    "damgaDetay presence".into(),
                ),
            )
        }
    }

    for (field, left, right) in [
        (
            "puantajOzeti.c",
            saved.puantajOzeti.c,
            replayed.puantajOzeti.c,
        ),
        (
            "puantajOzeti.t",
            saved.puantajOzeti.t,
            replayed.puantajOzeti.t,
        ),
        (
            "puantajOzeti.g",
            saved.puantajOzeti.g,
            replayed.puantajOzeti.g,
        ),
        (
            "puantajOzeti.i",
            saved.puantajOzeti.i,
            replayed.puantajOzeti.i,
        ),
        (
            "puantajOzeti.gc",
            saved.puantajOzeti.gc,
            replayed.puantajOzeti.gc,
        ),
        (
            "puantajOzeti.gct",
            saved.puantajOzeti.gct,
            replayed.puantajOzeti.gct,
        ),
        (
            "puantajOzeti.r",
            saved.puantajOzeti.r,
            replayed.puantajOzeti.r,
        ),
    ] {
        same!(field, left, right);
    }
    Ok(())
}

fn validate_parent_context(payroll: &BordroKaydi, period: &BordroDonemi) -> SnapshotResult<()> {
    if payroll.donemId != period.id {
        return Err(PayrollCalculationSnapshotError::IdentityMismatch(
            "BordroKaydi.donemId differs from BordroDonemi.id".into(),
        ));
    }
    if payroll.personelId.trim().is_empty() || payroll.accrualId.trim().is_empty() {
        return Err(PayrollCalculationSnapshotError::IdentityMismatch(
            "personelId and accrualId must be non-empty for a current snapshot".into(),
        ));
    }
    if payroll.sequence < 0 {
        return Err(PayrollCalculationSnapshotError::InvalidContext(
            "accrual sequence is negative".into(),
        ));
    }
    validate_period(period).map_err(|error| invalid_context("service period", error))?;
    validate_tax_month_overlap(period).map_err(|error| invalid_context("tax period", error))?;
    let payment_date = parse_canonical_work_date(&payroll.paymentDate, "paymentDate")
        .map_err(|error| invalid_context("paymentDate", error))?;
    if payment_date.year() != period.taxYear || payment_date.month() as i32 != period.taxMonth {
        return Err(PayrollCalculationSnapshotError::InvalidContext(
            "paymentDate must belong to the parent period taxYear/taxMonth".into(),
        ));
    }
    Ok(())
}

fn validate_compensation_segments(
    snapshot: &ResolvedCompensationSnapshot,
    period: &BordroDonemi,
) -> SnapshotResult<()> {
    if snapshot.segments.is_empty() {
        return Err(PayrollCalculationSnapshotError::SegmentGap(
            period.baslangicTarihi.clone(),
        ));
    }
    let period_start = parse_canonical_work_date(&period.baslangicTarihi, "period start")
        .map_err(|error| invalid_context("period start", error))?;
    let period_end = parse_canonical_work_date(&period.bitisTarihi, "period end")
        .map_err(|error| invalid_context("period end", error))?;
    let mut previous_start: Option<NaiveDate> = None;
    let mut previous_end: Option<NaiveDate> = None;
    let mut previous_state: Option<&CompensationPolicyState> = None;
    let mut road_mode: Option<RoadRoundingMode> = None;
    let mut version_ids = HashSet::new();

    for (index, segment) in snapshot.segments.iter().enumerate() {
        if segment.versionId.trim().is_empty() || segment.versionId != segment.versionId.trim() {
            return Err(PayrollCalculationSnapshotError::InvalidSegment(
                index,
                "versionId is empty or non-canonical".into(),
            ));
        }
        let start =
            parse_canonical_work_date(&segment.startDate, "segment start").map_err(|error| {
                PayrollCalculationSnapshotError::InvalidSegment(index, error.to_string())
            })?;
        let end = parse_canonical_work_date(&segment.endDate, "segment end").map_err(|error| {
            PayrollCalculationSnapshotError::InvalidSegment(index, error.to_string())
        })?;
        if start > end {
            return Err(PayrollCalculationSnapshotError::InvalidSegment(
                index,
                "startDate is after endDate".into(),
            ));
        }
        if start < period_start || end > period_end {
            return Err(PayrollCalculationSnapshotError::SegmentOutOfPeriod(
                segment.startDate.clone(),
            ));
        }
        if let Some(previous) = previous_start {
            if start < previous {
                return Err(PayrollCalculationSnapshotError::SegmentOutOfOrder(
                    segment.startDate.clone(),
                ));
            }
        }
        if let Some(previous) = previous_end {
            if start <= previous {
                return Err(PayrollCalculationSnapshotError::SegmentOverlap(
                    segment.startDate.clone(),
                ));
            }
            if previous
                .checked_add_signed(Duration::days(1))
                .is_none_or(|next| next < start)
            {
                return Err(PayrollCalculationSnapshotError::SegmentGap(
                    segment.startDate.clone(),
                ));
            }
        } else if start > period_start {
            return Err(PayrollCalculationSnapshotError::SegmentGap(
                segment.startDate.clone(),
            ));
        }
        crate::validate_compensation_policy_state(&segment.state).map_err(|error| {
            PayrollCalculationSnapshotError::InvalidSegment(index, error.to_string())
        })?;
        if let Some(previous_mode) = road_mode {
            if previous_mode != segment.roadRoundingMode {
                return Err(PayrollCalculationSnapshotError::InvalidSegment(
                    index,
                    "roadRoundingMode changes within one service period".into(),
                ));
            }
        } else {
            road_mode = Some(segment.roadRoundingMode);
        }
        if index > 0
            && segment.boundaryScope == CompensationBoundaryScope::RoadOnly
            && !crate::compensation_states_equal_except_road(
                previous_state.expect("a non-baseline segment has a previous state"),
                &segment.state,
            )
        {
            return Err(PayrollCalculationSnapshotError::InvalidSegment(
                index,
                "ROAD_ONLY may change only gunlukVasitaYol".into(),
            ));
        }
        if !version_ids.insert(segment.versionId.as_str()) {
            return Err(PayrollCalculationSnapshotError::IdentityMismatch(format!(
                "compensation version {} appears more than once in the resolved period",
                segment.versionId
            )));
        }
        previous_state = Some(&segment.state);
        previous_start = Some(start);
        previous_end = Some(end);
    }

    if previous_end != Some(period_end) {
        return Err(PayrollCalculationSnapshotError::SegmentGap(
            period.bitisTarihi.clone(),
        ));
    }
    Ok(())
}

fn validate_used_inputs(
    inputs: &PayrollUsedInputSnapshot,
    payroll: &BordroKaydi,
    period: &BordroDonemi,
) -> SnapshotResult<()> {
    if inputs.personnelGroup.trim().is_empty() || inputs.serviceYear < 0 {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "personnelGroup or non-negative serviceYear".into(),
        ));
    }
    validate_personal_deduction_inputs(inputs.personalDeductions.as_ref())?;
    validate_manual_income(&inputs.manualIncome)?;

    let attendance = inputs.attendance.as_ref().ok_or_else(|| {
        PayrollCalculationSnapshotError::MissingRequiredInput("daily attendance snapshot".into())
    })?;
    validate_attendance(
        attendance,
        period,
        payroll.status == BordroStatus::FINALIZED,
    )?;
    let paid_sick_leave = inputs.paidSickLeave.as_ref().ok_or_else(|| {
        PayrollCalculationSnapshotError::MissingRequiredInput(
            "paid-sick-leave episode/date snapshot".into(),
        )
    })?;
    validate_paid_sick_leave(paid_sick_leave, attendance, period, &payroll.personelId)?;

    validate_institution_deduction_settings(&inputs.institutionDeductionSettings)?;
    validate_statutory_inputs(&inputs.statutoryParameters, period)?;
    validate_annual_parameters(&inputs.annualParameters, period)?;
    validate_tax_history(&inputs.taxHistory, period, payroll)?;
    validate_incoming_pek_carry(&inputs.incomingPekCarry, period)?;

    if inputs
        .accrualGrossAmount
        .is_some_and(|amount| amount < Decimal::ZERO)
    {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "non-negative accrual grossAmount".into(),
        ));
    }
    Ok(())
}

fn validate_personal_deduction_inputs(
    deductions: Option<&PayrollPersonalDeductionInputsSnapshot>,
) -> SnapshotResult<()> {
    let Some(deductions) = deductions else {
        return Ok(());
    };
    for (field, value) in [
        ("sabitSendikaAidati", deductions.sabitSendikaAidati),
        ("sabitBesTutar", deductions.sabitBesTutar),
        ("icraTutar", deductions.icraTutar),
        ("nafakaTutar", deductions.nafakaTutar),
        ("kisiBorcuTutar", deductions.kisiBorcuTutar),
        (
            "dogumAskerlikBorclanmasiTutar",
            deductions.dogumAskerlikBorclanmasiTutar,
        ),
        (
            "hayatSaglikSigortasiTutar",
            deductions.hayatSaglikSigortasiTutar,
        ),
        ("digerKesintiTutar", deductions.digerKesintiTutar),
    ] {
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                format!("non-negative personal deduction {field}"),
            ));
        }
    }
    if deductions
        .oksOraniYuzde
        .is_some_and(|rate| rate < Decimal::ZERO || rate > Decimal::from(100))
    {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "personal OKS percent in 0..=100".into(),
        ));
    }
    if let Some(gv) = deductions.gvIndirimleri.as_ref() {
        for (field, value) in [
            (
                "dogumAskerlikGvIndirimTutar",
                gv.dogumAskerlikGvIndirimTutar,
            ),
            ("hayatSigortasiPrimiTutar", gv.hayatSigortasiPrimiTutar),
            ("saglikSigortasiPrimiTutar", gv.saglikSigortasiPrimiTutar),
        ] {
            if value.is_some_and(|amount| amount < Decimal::ZERO) {
                return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                    format!("non-negative personal GV input {field}"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_manual_income(input: &PayrollManualIncomeInputSnapshot) -> SnapshotResult<()> {
    if input
        .tediye
        .into_iter()
        .chain(input.tisIkramiyesi)
        .any(|amount| amount < Decimal::ZERO)
    {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "non-negative manual payroll income".into(),
        ));
    }
    if !input.supplied && (input.tediye.is_some() || input.tisIkramiyesi.is_some()) {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "manual income values must be null when supplied is false".into(),
        ));
    }
    Ok(())
}

fn validate_attendance(
    attendance: &PayrollAttendanceInputSnapshot,
    period: &BordroDonemi,
    require_complete: bool,
) -> SnapshotResult<()> {
    let start = parse_canonical_work_date(&period.baslangicTarihi, "period start")
        .map_err(|error| invalid_context("period start", error))?;
    let end = parse_canonical_work_date(&period.bitisTarihi, "period end")
        .map_err(|error| invalid_context("period end", error))?;
    if attendance.days.is_empty() {
        return Err(PayrollCalculationSnapshotError::InvalidAttendance(
            "NORMAL snapshot has no daily attendance".into(),
        ));
    }

    let mut previous: Option<NaiveDate> = None;
    let mut all_days = true;
    let mut current = start;
    for day in &attendance.days {
        let date = parse_canonical_work_date(&day.date, "attendance date").map_err(|error| {
            PayrollCalculationSnapshotError::InvalidAttendance(error.to_string())
        })?;
        if date < start || date > end {
            return Err(PayrollCalculationSnapshotError::InvalidAttendance(format!(
                "{} is outside the parent service period",
                day.date
            )));
        }
        if !crate::validation::VALID_ATTENDANCE_CODES.contains(&day.code.as_str()) {
            return Err(PayrollCalculationSnapshotError::InvalidAttendance(format!(
                "unsupported attendance code {} at {}",
                day.code, day.date
            )));
        }
        if let Some(previous) = previous {
            if date <= previous {
                return Err(PayrollCalculationSnapshotError::InvalidAttendance(
                    "daily attendance dates must be strictly ordered and unique".into(),
                ));
            }
        }
        if date != current {
            all_days = false;
        }
        current = date.checked_add_signed(Duration::days(1)).ok_or_else(|| {
            PayrollCalculationSnapshotError::InvalidAttendance("date overflow".into())
        })?;
        previous = Some(date);
    }
    all_days &= attendance
        .days
        .first()
        .is_some_and(|day| day.date == period.baslangicTarihi)
        && attendance
            .days
            .last()
            .is_some_and(|day| day.date == period.bitisTarihi);
    let declared_complete = attendance.coverage == PayrollAttendanceCoverage::COMPLETE;
    if declared_complete != all_days {
        return Err(PayrollCalculationSnapshotError::InvalidAttendance(
            "declared attendance coverage does not match the daily date set".into(),
        ));
    }
    if require_complete && !declared_complete {
        return Err(PayrollCalculationSnapshotError::InvalidAttendance(
            "FINALIZED NORMAL snapshot requires full period attendance coverage".into(),
        ));
    }
    Ok(())
}

fn validate_paid_sick_leave(
    paid_leave: &PayrollPaidSickLeaveInputSnapshot,
    attendance: &PayrollAttendanceInputSnapshot,
    period: &BordroDonemi,
    personnel_id: &str,
) -> SnapshotResult<()> {
    if paid_leave.ruleVersion != PAID_SICK_LEAVE_RULE_VERSION {
        return Err(
            PayrollCalculationSnapshotError::UnsupportedPaidSickLeaveRuleVersion(
                paid_leave.ruleVersion.clone(),
            ),
        );
    }
    let mut ids = HashSet::new();
    let records = paid_leave
        .sourceEpisodes
        .iter()
        .map(|episode| {
            if episode.id.trim().is_empty() || !ids.insert(episode.id.as_str()) {
                return Err(PayrollCalculationSnapshotError::PaidSickLeaveMismatch(
                    "source episode ids must be non-empty and unique".into(),
                ));
            }
            parse_canonical_work_date(&episode.startDate, "sick episode start").map_err(
                |error| PayrollCalculationSnapshotError::PaidSickLeaveMismatch(error.to_string()),
            )?;
            parse_canonical_work_date(&episode.endDate, "sick episode end").map_err(|error| {
                PayrollCalculationSnapshotError::PaidSickLeaveMismatch(error.to_string())
            })?;
            Ok(SickLeaveRecord {
                id: episode.id.clone(),
                personnelId: personnel_id.to_string(),
                startDate: episode.startDate.clone(),
                endDate: episode.endDate.clone(),
                createdAt: None,
                updatedAt: None,
            })
        })
        .collect::<SnapshotResult<Vec<_>>>()?;
    let resolved = calculate_paid_sick_dates_from_records(&records, period)
        .map_err(|error| PayrollCalculationSnapshotError::PaidSickLeaveMismatch(error.to_string()))?
        .into_iter()
        .map(|date| date.format("%Y-%m-%d").to_string())
        .collect::<Vec<_>>();
    if resolved != paid_leave.paidDates {
        return Err(PayrollCalculationSnapshotError::PaidSickLeaveMismatch(
            "paidDates differ from source episodes and the supported rule".into(),
        ));
    }

    let attendance_by_date = attendance
        .days
        .iter()
        .map(|day| (day.date.as_str(), day.code.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let mut previous: Option<NaiveDate> = None;
    for date_text in &paid_leave.paidDates {
        let date = parse_canonical_work_date(date_text, "paid sick date").map_err(|error| {
            PayrollCalculationSnapshotError::PaidSickLeaveMismatch(error.to_string())
        })?;
        if previous.is_some_and(|prior| date <= prior) {
            return Err(PayrollCalculationSnapshotError::PaidSickLeaveMismatch(
                "paidDates must be strictly ordered and unique".into(),
            ));
        }
        if attendance_by_date.get(date_text.as_str()) != Some(&"R") {
            return Err(PayrollCalculationSnapshotError::PaidSickLeaveMismatch(
                format!("paid sick date {date_text} is absent from attendance or is not coded R"),
            ));
        }
        previous = Some(date);
    }
    Ok(())
}

fn validate_institution_deduction_settings(
    settings: &PayrollInstitutionDeductionInputsSnapshot,
) -> SnapshotResult<()> {
    for (field, value) in [
        ("damgaVergisiOraniBinde", settings.damgaVergisiOraniBinde),
        ("sabitSendikaAidati", settings.sabitSendikaAidati),
        ("sabitBesTutar", settings.sabitBesTutar),
    ] {
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                format!("non-negative institution setting {field}"),
            ));
        }
    }
    for (field, value) in [
        ("sendikaAidatiYuzde", settings.sendikaAidatiYuzde),
        ("besOraniYuzde", settings.besOraniYuzde),
    ] {
        if value.is_some_and(|rate| rate < Decimal::ZERO || rate > Decimal::from(100)) {
            return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                format!("institution setting {field} in 0..=100"),
            ));
        }
    }
    Ok(())
}

fn validate_statutory_inputs(
    statutory: &PayrollStatutoryInputsSnapshot,
    period: &BordroDonemi,
) -> SnapshotResult<()> {
    if statutory.gunlukAsgariUcret <= Decimal::ZERO
        || statutory.pekTavanKatsayisi < Decimal::ONE
        || statutory.gunlukYemekIstisnasiSGK < Decimal::ZERO
        || statutory.gunlukYemekIstisnasiGV < Decimal::ZERO
    {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "valid current statutory amounts".into(),
        ));
    }
    for (field, value) in [
        ("sgkIsciOraniYuzde", Some(statutory.sgkIsciOraniYuzde)),
        (
            "issizlikIsciOraniYuzde",
            Some(statutory.issizlikIsciOraniYuzde),
        ),
        ("sgkIsverenOraniYuzde", statutory.sgkIsverenOraniYuzde),
        (
            "issizlikIsverenOraniYuzde",
            statutory.issizlikIsverenOraniYuzde,
        ),
    ] {
        if value.is_some_and(|rate| rate < Decimal::ZERO || rate > Decimal::from(100)) {
            return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                format!("statutory rate {field} in 0..=100"),
            ));
        }
    }
    let start = parse_canonical_work_date(&period.baslangicTarihi, "period start")
        .map_err(|error| invalid_context("period start", error))?;
    let end = parse_canonical_work_date(&period.bitisTarihi, "period end")
        .map_err(|error| invalid_context("period end", error))?;
    let mut previous = None;
    for segment in &statutory.statutoryParameterSegments {
        let date = parse_canonical_work_date(&segment.effectiveFrom, "statutory effectiveFrom")
            .map_err(|error| PayrollCalculationSnapshotError::InvalidContext(error.to_string()))?;
        if date < start || date > end || previous.is_some_and(|prior| date <= prior) {
            return Err(PayrollCalculationSnapshotError::InvalidContext(
                "statutory parameter segments must be ordered, unique, and in-period".into(),
            ));
        }
        for (field, value) in [
            ("gunlukAsgariUcret", segment.gunlukAsgariUcret),
            ("pekTavanKatsayisi", segment.pekTavanKatsayisi),
            ("gunlukYemekIstisnasiSGK", segment.gunlukYemekIstisnasiSGK),
            ("gunlukYemekIstisnasiGV", segment.gunlukYemekIstisnasiGV),
        ] {
            if value.is_some_and(|amount| amount < Decimal::ZERO) {
                return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                    format!("non-negative statutory segment {field}"),
                ));
            }
        }
        previous = Some(date);
    }
    Ok(())
}

fn validate_annual_parameters(
    parameters: &PayrollAnnualParametersSnapshot,
    period: &BordroDonemi,
) -> SnapshotResult<()> {
    if parameters.year != period.taxYear {
        return Err(PayrollCalculationSnapshotError::IdentityMismatch(
            "annual tariff year differs from period taxYear".into(),
        ));
    }
    if parameters.gelirVergisiDilimleri.is_empty()
        || parameters
            .sigortaGvYillikBrutAsgariUcretTavani
            .is_none_or(|cap| cap <= Decimal::ZERO)
    {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "annual tax brackets and positive insurance GV annual cap".into(),
        ));
    }
    let mut previous_limit = Decimal::ZERO;
    for bracket in &parameters.gelirVergisiDilimleri {
        if bracket.limit <= previous_limit
            || bracket.oran < Decimal::ZERO
            || bracket.oran > Decimal::ONE
        {
            return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
                "strictly increasing annual tax brackets with rates in 0..=1".into(),
            ));
        }
        previous_limit = bracket.limit;
    }
    Ok(())
}

fn validate_tax_history(
    history: &PayrollTaxHistoryInputsSnapshot,
    period: &BordroDonemi,
    payroll: &BordroKaydi,
) -> SnapshotResult<()> {
    validate_tax_opening(&history.normalGvOpening, period, "normal GV")?;
    validate_tax_opening(&history.minimumWageGvOpening, period, "minimum-wage GV")?;
    for (field, value) in [
        ("previousCumulativeGv", history.previousCumulativeGv),
        (
            "previousCumulativeMinimumWageGv",
            history.previousCumulativeMinimumWageGv,
        ),
        (
            "previousAnnualInsuranceGvDeduction",
            history.previousAnnualInsuranceGvDeduction,
        ),
        ("samePaymentMonthPekUsed", history.samePaymentMonthPekUsed),
        (
            "samePaymentMonthGvExemptionUsed",
            history.samePaymentMonthGvExemptionUsed,
        ),
        (
            "samePaymentMonthStampExemptionUsed",
            history.samePaymentMonthStampExemptionUsed,
        ),
    ] {
        if value < Decimal::ZERO {
            return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(format!(
                "{field} is negative"
            )));
        }
    }

    let current_order = (
        period.taxYear,
        period.taxMonth,
        payroll.paymentDate.as_str(),
        payroll.sequence,
        payroll.accrualId.as_str(),
    );
    let mut seen = HashSet::new();
    let mut previous_order = None;
    let mut previous_gv = history.normalGvOpening.amount;
    let mut previous_insurance = Decimal::ZERO;
    let mut same_month_pek = Decimal::ZERO;
    let mut same_month_gv_exemption = Decimal::ZERO;
    let mut same_month_stamp_exemption = Decimal::ZERO;
    for event in &history.previousPaymentEvents {
        if event.periodId.trim().is_empty()
            || event.accrualId.trim().is_empty()
            || event.sequence < 0
            || !seen.insert((event.periodId.as_str(), event.accrualId.as_str()))
        {
            return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
                "prior payment event identity must be non-empty and unique".into(),
            ));
        }
        let date = parse_canonical_work_date(&event.paymentDate, "prior paymentDate").map_err(
            |error| PayrollCalculationSnapshotError::InvalidTaxHistory(error.to_string()),
        )?;
        if date.year() != event.taxYear || date.month() as i32 != event.taxMonth {
            return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
                "prior payment date and tax year/month disagree".into(),
            ));
        }
        if event.taxYear != period.taxYear || event.taxMonth > period.taxMonth {
            return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
                "prior tax event is outside the current tax year/month boundary".into(),
            ));
        }
        let order = (
            event.taxYear,
            event.taxMonth,
            event.paymentDate.as_str(),
            event.sequence,
            event.accrualId.as_str(),
        );
        if order >= current_order || previous_order.is_some_and(|prior| order <= prior) {
            return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
                "prior payment events must be strictly ordered before the current accrual".into(),
            ));
        }
        validate_prior_payment_event(event)?;
        previous_order = Some(order);
        previous_insurance = add_tax_history_amount(
            previous_insurance,
            event.insuranceGvDeductionUsed,
            "annual insurance GV deductions",
        )?;
        if event.taxMonth >= history.normalGvOpening.effectiveFromTaxMonth {
            previous_gv =
                add_tax_history_amount(previous_gv, event.gvTaxableBase, "normal cumulative GV")?;
        }
        if event.taxMonth == period.taxMonth {
            same_month_gv_exemption = round_tax_total(add_tax_history_amount(
                same_month_gv_exemption,
                event.gvExemptionUsed,
                "same-month GV exemption",
            )?);
            same_month_stamp_exemption = round_tax_total(add_tax_history_amount(
                same_month_stamp_exemption,
                event.stampExemptionUsed,
                "same-month stamp exemption",
            )?);
            let month_pek_after = event.monthPekConsumptionAfter.map(Ok).unwrap_or_else(|| {
                add_tax_history_amount(
                    same_month_pek,
                    event.pekPrimeBase,
                    "same-month PEK consumption",
                )
            })?;
            same_month_pek = round_tax_total(month_pek_after);
        }
    }
    if round_tax_total(previous_gv) != history.previousCumulativeGv
        || round_tax_total(previous_insurance) != history.previousAnnualInsuranceGvDeduction
        || same_month_pek != history.samePaymentMonthPekUsed
        || same_month_gv_exemption != history.samePaymentMonthGvExemptionUsed
        || same_month_stamp_exemption != history.samePaymentMonthStampExemptionUsed
    {
        return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
            format!(
                "captured cumulative or same-payment-month values do not reconcile to prior events (GV {} != {}, insurance {} != {}, PEK {} != {}, GV exemption {} != {}, stamp exemption {} != {})",
                history.previousCumulativeGv,
                round_tax_total(previous_gv),
                history.previousAnnualInsuranceGvDeduction,
                round_tax_total(previous_insurance),
                history.samePaymentMonthPekUsed,
                same_month_pek,
                history.samePaymentMonthGvExemptionUsed,
                same_month_gv_exemption,
                history.samePaymentMonthStampExemptionUsed,
                same_month_stamp_exemption,
            ),
        ));
    }

    let mut previous_minimum_gv = history.minimumWageGvOpening.amount;
    let expected_reference_count =
        (period.taxMonth - history.minimumWageGvOpening.effectiveFromTaxMonth) as usize;
    if history.minimumWageMonthlyReferences.len() != expected_reference_count {
        return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
            "minimum-wage monthly references must cover every tax month from the opening through the month before the current payment".into(),
        ));
    }
    for (index, reference) in history.minimumWageMonthlyReferences.iter().enumerate() {
        let expected_month = history.minimumWageGvOpening.effectiveFromTaxMonth + index as i32;
        if reference.periodId.trim().is_empty()
            || reference.periodId != reference.periodId.trim()
            || reference.taxYear != period.taxYear
            || reference.taxMonth != expected_month
            || reference.dailyMinimumWage <= Decimal::ZERO
            || reference.sgkWorkerRatePercent < Decimal::ZERO
            || reference.sgkWorkerRatePercent > Decimal::from(100)
            || reference.unemploymentWorkerRatePercent < Decimal::ZERO
            || reference.unemploymentWorkerRatePercent > Decimal::from(100)
            || reference.monthlyGvBase < Decimal::ZERO
        {
            return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
                "minimum-wage monthly references must be valid, ordered, and unique".into(),
            ));
        }
        let expected_monthly_gv = crate::calculate_aylik_asgari_ucret_gv_matrahi(
            reference.dailyMinimumWage,
            reference.sgkWorkerRatePercent / Decimal::from(100),
            reference.unemploymentWorkerRatePercent / Decimal::from(100),
        );
        if reference.monthlyGvBase != expected_monthly_gv {
            return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(format!(
                "minimum-wage monthly GV reference for tax month {} does not match the captured daily minimum wage and worker rates",
                reference.taxMonth
            )));
        }
        previous_minimum_gv = add_tax_history_amount(
            previous_minimum_gv,
            reference.monthlyGvBase,
            "minimum-wage cumulative GV",
        )?;
    }
    if round_tax_total(previous_minimum_gv) != history.previousCumulativeMinimumWageGv {
        return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
            "minimum-wage opening plus monthly references does not equal the used cumulative input"
                .into(),
        ));
    }
    Ok(())
}

fn round_tax_total(value: Decimal) -> Decimal {
    crate::round_currency_amount(value)
}

fn add_tax_history_amount(total: Decimal, amount: Decimal, field: &str) -> SnapshotResult<Decimal> {
    total.checked_add(amount).ok_or_else(|| {
        PayrollCalculationSnapshotError::InvalidTaxHistory(format!("{field} overflowed"))
    })
}

fn validate_tax_opening(
    opening: &PayrollTaxOpeningSnapshot,
    period: &BordroDonemi,
    name: &str,
) -> SnapshotResult<()> {
    if opening.effectiveFromTaxYear != period.taxYear
        || !(1..=period.taxMonth).contains(&opening.effectiveFromTaxMonth)
        || opening.amount < Decimal::ZERO
    {
        return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(format!(
            "{name} opening has an invalid amount or tax anchor"
        )));
    }
    match opening.source {
        PayrollTaxOpeningSource::EXPLICIT_OPENING => {
            if opening.openingId.as_deref().is_none_or(str::is_empty) {
                return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(format!(
                    "{name} explicit opening is missing its source id"
                )));
            }
        }
        PayrollTaxOpeningSource::LEGACY_PERSONNEL_VALUE => {
            if opening.openingId.is_some() {
                return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(format!(
                    "{name} legacy personnel opening cannot carry an explicit row id"
                )));
            }
        }
        PayrollTaxOpeningSource::IMPLICIT_ZERO => {
            if opening.openingId.is_some()
                || opening.effectiveFromTaxMonth != 1
                || opening.amount != Decimal::ZERO
            {
                return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(format!(
                    "{name} implicit opening must be zero from tax month 1"
                )));
            }
        }
    }
    Ok(())
}

fn validate_prior_payment_event(event: &PayrollPriorPaymentEventSnapshot) -> SnapshotResult<()> {
    if event.gvTaxableBase < Decimal::ZERO
        || event.insuranceGvDeductionUsed < Decimal::ZERO
        || event.gvExemptionUsed < Decimal::ZERO
        || event.stampExemptionUsed < Decimal::ZERO
        || event.pekPrimeBase < Decimal::ZERO
        || event
            .monthPekConsumptionAfter
            .is_some_and(|value| value < Decimal::ZERO)
        || event.pekCeiling < Decimal::ZERO
        || event.mealSgkExemption < Decimal::ZERO
        || event
            .outgoingPekCarry
            .iter()
            .any(|carry| carry.tutar < Decimal::ZERO || carry.kalanAySayisi < 0)
    {
        return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
            "prior event contains negative tax, PEK, exemption, or carry input".into(),
        ));
    }
    let income_values = [
        event.income.tabanBrutAylik,
        event.income.tediye,
        event.income.tisIkramiyesi,
        event.income.ekOdeme,
        event.income.yemek,
        event.income.birlestirilmisSosyalYardim,
        event.income.vasitaYol,
        event.income.giyimYardimi,
        event.income.isPrimi,
        event.income.geceCalismasiUcreti,
        event.income.geceCalismasiTatiliUcreti,
        event.income.hizmetZammi,
        event.income.digerGelir,
    ];
    if income_values
        .into_iter()
        .flatten()
        .any(|amount| amount < Decimal::ZERO)
    {
        return Err(PayrollCalculationSnapshotError::InvalidTaxHistory(
            "prior event contains a negative ordinary income input".into(),
        ));
    }
    Ok(())
}

fn validate_incoming_pek_carry(
    carry: &PayrollIncomingPekCarrySnapshot,
    period: &BordroDonemi,
) -> SnapshotResult<()> {
    if carry.taxMonthsElapsed < 0
        || carry
            .records
            .iter()
            .any(|record| record.tutar < Decimal::ZERO || record.kalanAySayisi < 0)
    {
        return Err(PayrollCalculationSnapshotError::InvalidPekCarry(
            "negative elapsed month, carry amount, or remaining month count".into(),
        ));
    }
    let source_fields_present = [
        carry.sourceId.is_some(),
        carry.sourcePeriodId.is_some(),
        carry.sourceTaxYear.is_some(),
        carry.sourceTaxMonth.is_some(),
    ];
    if carry.source == PayrollPekCarrySource::NONE {
        if source_fields_present.into_iter().any(|present| present)
            || !carry.records.is_empty()
            || carry.taxMonthsElapsed != 0
        {
            return Err(PayrollCalculationSnapshotError::InvalidPekCarry(
                "NONE carry requires empty records, no source identity/anchor, and zero elapsed months".into(),
            ));
        }
        return Ok(());
    }

    if source_fields_present.into_iter().any(|present| !present) {
        return Err(PayrollCalculationSnapshotError::InvalidPekCarry(
            "sourced carry requires source id, source period id, tax year, and tax month".into(),
        ));
    }
    let source_id = carry.sourceId.as_deref().expect("presence checked above");
    let source_period_id = carry
        .sourcePeriodId
        .as_deref()
        .expect("presence checked above");
    let source_year = carry.sourceTaxYear.expect("presence checked above");
    let source_month = carry.sourceTaxMonth.expect("presence checked above");
    if source_id.trim().is_empty()
        || source_id != source_id.trim()
        || source_period_id.trim().is_empty()
        || source_period_id != source_period_id.trim()
        || source_year <= 0
        || !(1..=12).contains(&source_month)
        || source_year > period.taxYear
        || (source_year == period.taxYear && source_month > period.taxMonth)
    {
        return Err(PayrollCalculationSnapshotError::InvalidPekCarry(
            "sourced carry identity or tax anchor is invalid or non-canonical".into(),
        ));
    }
    let months_elapsed = (i64::from(period.taxYear) - i64::from(source_year)) * 12
        + i64::from(period.taxMonth - source_month);
    if months_elapsed != i64::from(carry.taxMonthsElapsed) {
        return Err(PayrollCalculationSnapshotError::InvalidPekCarry(
            "taxMonthsElapsed disagrees with the carry source tax anchor".into(),
        ));
    }
    Ok(())
}

fn validate_resolved_statutory_snapshot(
    payroll: &BordroKaydi,
    period: &BordroDonemi,
) -> SnapshotResult<()> {
    let statutory = payroll.statutorySnapshot.as_ref().ok_or_else(|| {
        PayrollCalculationSnapshotError::MissingRequiredInput(
            "resolved statutorySnapshot output".into(),
        )
    })?;
    if statutory.source != StatutorySnapshotSource::AttendanceBacked {
        return Err(PayrollCalculationSnapshotError::MissingRequiredInput(
            "attendance-backed statutorySnapshot".into(),
        ));
    }
    if statutory.segments.is_empty() {
        return Err(PayrollCalculationSnapshotError::InvalidContext(
            "resolved statutorySnapshot has no segments".into(),
        ));
    }
    let period_start = parse_canonical_work_date(&period.baslangicTarihi, "period start")
        .map_err(|error| invalid_context("period start", error))?;
    let period_end = parse_canonical_work_date(&period.bitisTarihi, "period end")
        .map_err(|error| invalid_context("period end", error))?;
    let mut prior_end = None;
    for (index, segment) in statutory.segments.iter().enumerate() {
        let start = parse_canonical_work_date(&segment.effectiveFrom, "statutory segment start")
            .map_err(|error| invalid_context("statutory segment start", error))?;
        let end = parse_canonical_work_date(&segment.effectiveTo, "statutory segment end")
            .map_err(|error| invalid_context("statutory segment end", error))?;
        if start > end
            || start < period_start
            || end > period_end
            || (index == 0 && start != period_start)
            || prior_end.is_some_and(|previous: NaiveDate| {
                previous.checked_add_signed(Duration::days(1)) != Some(start)
            })
        {
            return Err(PayrollCalculationSnapshotError::InvalidContext(
                "resolved statutory segments must exactly and contiguously cover the period".into(),
            ));
        }
        prior_end = Some(end);
    }
    if prior_end != Some(period_end) {
        return Err(PayrollCalculationSnapshotError::InvalidContext(
            "resolved statutory segments do not reach the period end".into(),
        ));
    }
    Ok(())
}

fn invalid_context(field: &str, error: impl std::fmt::Display) -> PayrollCalculationSnapshotError {
    PayrollCalculationSnapshotError::InvalidContext(format!("{field}: {error}"))
}
