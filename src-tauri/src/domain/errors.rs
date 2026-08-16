use rust_decimal::Decimal;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum DomainError {
    #[error("{0}")]
    ValidationError(String),

    #[error("Devir matrahı çakışması: {0}")]
    TaxOpeningConflict(String),

    #[error("Kayıt bulunamadı: {0}")]
    NotFound(String),

    #[error("Bordro kesinleştirilmiş: {0}")]
    PayrollFinalized(String),

    #[error("Veritabanı hatası: {0}")]
    DatabaseError(String),

    #[error("Geçersiz veri: {0}")]
    InvalidData(String),

    #[error("Kesintiler toplam geliri aşıyor: gelir={gelir}, kesinti={kesinti}, fark={fark}")]
    NegativeNetPayment {
        gelir: Decimal,
        kesinti: Decimal,
        fark: Decimal,
    },
}

pub type Result<T> = std::result::Result<T, DomainError>;
