use payroll_core::{
    normalize_turkish_iban, validate_personnel_for_payroll, Personel, PersonelKesintileri,
};
use rust_decimal_macros::dec;

const VALID_IBAN: &str = "TR330006100519786457841326";

fn person() -> Personel {
    Personel {
        id: "worker-1".into(),
        tcNo: "11111111110".into(),
        ad: "Ayşe".into(),
        soyad: "Yılmaz".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: String::new(),
        iban: VALID_IBAN.into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri::default()),
    }
}

fn valid_iban_with_bban(bban: &str) -> String {
    let rearranged = format!("{bban}TR00");
    let mut remainder = 0_u32;
    for character in rearranged.bytes() {
        if character.is_ascii_digit() {
            remainder = (remainder * 10 + u32::from(character - b'0')) % 97;
        } else {
            let numeric_value = u32::from(character - b'A') + 10;
            remainder = (remainder * 100 + numeric_value) % 97;
        }
    }
    format!("TR{:02}{bban}", 98 - remainder)
}

#[test]
fn turkish_iban_accepts_valid_grouped_input_and_preserves_account_characters() {
    assert_eq!(
        normalize_turkish_iban(" tr33 0006 1005 1978 6457 8413 26 ").unwrap(),
        VALID_IBAN
    );
    let alpha_bban = format!("000610{}", "00000000ABCDEF12");
    let alpha_account = valid_iban_with_bban(&alpha_bban);
    assert_eq!(
        normalize_turkish_iban(&alpha_account).unwrap(),
        alpha_account
    );
}

#[test]
fn turkish_iban_rejects_missing_wrong_length_wrong_layout_and_wrong_check_digits() {
    let invalid = [
        "TR",
        "TR33000610051978645784132",
        "TR3300061005197864578413260",
        "TR3A0006100519786457841326",
        "TR3300061100519786457841326",
        "TR330006100519786457841325",
        "",
    ];
    for iban in invalid {
        assert!(normalize_turkish_iban(iban).is_err(), "accepted {iban}");
    }
}

#[test]
fn personnel_domain_validation_rejects_negative_deduction_amounts_without_sign_conversion() {
    let mut worker = person();
    for amount in [dec!(-1), dec!(-0.01)] {
        worker.kesintiler.as_mut().unwrap().icraTutar = Some(amount);
        assert!(validate_personnel_for_payroll(&worker).is_err());
        assert_eq!(worker.kesintiler.as_ref().unwrap().icraTutar, Some(amount));
    }

    for amount in [dec!(0), dec!(0.01)] {
        worker.kesintiler.as_mut().unwrap().icraTutar = Some(amount);
        assert!(validate_personnel_for_payroll(&worker).is_ok());
    }
}
