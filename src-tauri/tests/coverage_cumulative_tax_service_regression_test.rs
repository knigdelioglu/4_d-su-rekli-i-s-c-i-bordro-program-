use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{BordroDonemi, PayrollAccrualInput};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::services::cumulative_tax_service::CumulativeTaxService;

fn january_2027_period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-12".into(),
        donemAdi: "Aralık 2026".into(),
        yil: 2026,
        ay: 12,
        baslangicTarihi: "2026-12-15".into(),
        bitisTarihi: "2027-01-14".into(),
        taxYear: 2027,
        taxMonth: 1,
    }
}

fn accrual(payment_date: &str, accrual_id: &str) -> PayrollAccrualInput {
    PayrollAccrualInput {
        accrualId: accrual_id.into(),
        accrualType: bordro_programi_lib::domain::models::AccrualType::TEDIYE,
        paymentDate: payment_date.into(),
        sequence: 1,
        grossAmount: None,
        description: None,
    }
}

#[test]
fn accrual_tax_chain_rejects_invalid_date_month_and_identity() {
    let conn = create_in_memory_connection().expect("in-memory database");
    let active = january_2027_period();

    let invalid_date = CumulativeTaxService::get_previous_cumulative_gv_for_accrual(
        &conn,
        "p-validation",
        &active,
        &accrual("2027-02-01", "accrual-date"),
    )
    .expect_err("invalid calendar month must be rejected");
    assert!(
        matches!(invalid_date, DomainError::ValidationError(message) if message.contains("aktif vergi yılı/ayı"))
    );

    let malformed_date = CumulativeTaxService::get_previous_cumulative_gv_for_accrual(
        &conn,
        "p-validation",
        &active,
        &accrual("not-a-date", "accrual-malformed"),
    )
    .expect_err("malformed payment date must be rejected");
    assert!(
        matches!(malformed_date, DomainError::ValidationError(message) if message.contains("Tahakkuk ödeme tarihi geçersiz"))
    );

    let empty_id = CumulativeTaxService::get_previous_cumulative_gv_for_accrual(
        &conn,
        "p-validation",
        &active,
        &accrual("2027-01-20", "  "),
    )
    .expect_err("an accrual without an identity cannot enter the tax chain");
    assert!(
        matches!(empty_id, DomainError::ValidationError(message) if message.contains("Tahakkuk kimliği boş olamaz"))
    );
}
