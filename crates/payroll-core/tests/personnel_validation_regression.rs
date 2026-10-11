use payroll_core::{
    normalize_turkish_iban, validate_attendance_for_period, validate_personnel_for_payroll,
    validate_sick_leave_records, BordroDonemi, Personel, PersonelKesintileri, PersonelPuantaj,
    SickLeaveRecord,
};
use rust_decimal_macros::dec;
use std::collections::HashMap;

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

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-01".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-01".into(),
        bitisTarihi: "2026-01-03".into(),
        donemAdi: "January 2026".into(),
        taxYear: 2026,
        taxMonth: 1,
    }
}

fn sick_leave(id: &str, personnel_id: &str, start: &str, end: &str) -> SickLeaveRecord {
    SickLeaveRecord {
        id: id.into(),
        personnelId: personnel_id.into(),
        startDate: start.into(),
        endDate: end.into(),
        createdAt: None,
        updatedAt: None,
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
    assert_eq!(
        normalize_turkish_iban(&alpha_account.to_lowercase()).unwrap(),
        alpha_account
    );
}

#[test]
fn turkish_iban_rejects_missing_wrong_length_wrong_layout_and_wrong_check_digits() {
    let invalid = [
        "TR",
        "TR33000610051978645784132",
        "TR3300061005197864578413260",
        "XY330006100519786457841326",
        "TR3A0006100519786457841326",
        "TR330006111519786457841326",
        "TR3300061100519786457841326",
        "TR330006100519786457841325",
        "TR33000610051978645784132!",
        "",
    ];
    for iban in invalid {
        assert!(normalize_turkish_iban(iban).is_err(), "accepted {iban}");
    }
}

#[test]
fn attendance_validation_rejects_invalid_periods_and_out_of_range_entries() {
    let period = period();
    let valid_attendance = PersonelPuantaj {
        id: "worker-1_2026-01".into(),
        personelId: "worker-1".into(),
        donemId: period.id.clone(),
        gunler: HashMap::from([("2026-01-01".into(), "Ç".into())]),
    };
    assert!(validate_attendance_for_period(&valid_attendance, &period).is_ok());

    let mut legacy_attendance = valid_attendance.clone();
    legacy_attendance.donemId.clear();
    assert!(validate_attendance_for_period(&legacy_attendance, &period).is_ok());

    let mut mismatched_id = valid_attendance.clone();
    mismatched_id.donemId = "2026-02".into();
    assert!(validate_attendance_for_period(&mismatched_id, &period).is_err());

    let mut invalid_start = period.clone();
    invalid_start.baslangicTarihi = "not-a-date".into();
    assert!(validate_attendance_for_period(&valid_attendance, &invalid_start).is_err());

    let mut invalid_end = period.clone();
    invalid_end.bitisTarihi = "not-a-date".into();
    assert!(validate_attendance_for_period(&valid_attendance, &invalid_end).is_err());

    let mut reversed = period.clone();
    reversed.baslangicTarihi = "2026-01-04".into();
    assert!(validate_attendance_for_period(&valid_attendance, &reversed).is_err());

    let too_many_days = PersonelPuantaj {
        gunler: HashMap::from([
            ("2026-01-01".into(), "Ç".into()),
            ("2026-01-02".into(), "Ç".into()),
            ("2026-01-03".into(), "Ç".into()),
            ("2026-01-04".into(), "Ç".into()),
        ]),
        ..valid_attendance.clone()
    };
    assert!(validate_attendance_for_period(&too_many_days, &period).is_err());

    let unknown_code = PersonelPuantaj {
        gunler: HashMap::from([("2026-01-01".into(), "?".into())]),
        ..valid_attendance.clone()
    };
    assert!(validate_attendance_for_period(&unknown_code, &period).is_err());

    let invalid_date = PersonelPuantaj {
        gunler: HashMap::from([("2026-02-30".into(), "Ç".into())]),
        ..valid_attendance.clone()
    };
    assert!(validate_attendance_for_period(&invalid_date, &period).is_err());

    let outside_date = PersonelPuantaj {
        gunler: HashMap::from([("2025-12-31".into(), "Ç".into())]),
        ..valid_attendance
    };
    assert!(validate_attendance_for_period(&outside_date, &period).is_err());
}

#[test]
fn sick_leave_validation_checks_dates_personnel_and_overlap() {
    assert!(validate_sick_leave_records(&[]).is_ok());
    assert!(validate_sick_leave_records(&[sick_leave(
        "episode-1",
        "worker-1",
        "2026-01-01",
        "2026-01-02"
    )])
    .is_ok());

    assert!(validate_sick_leave_records(&[sick_leave(
        "bad-start",
        "worker-1",
        "not-a-date",
        "2026-01-02"
    )])
    .is_err());
    assert!(validate_sick_leave_records(&[sick_leave(
        "bad-end",
        "worker-1",
        "2026-01-01",
        "not-a-date"
    )])
    .is_err());
    assert!(validate_sick_leave_records(&[sick_leave(
        "reversed",
        "worker-1",
        "2026-01-03",
        "2026-01-01"
    )])
    .is_err());
    assert!(validate_sick_leave_records(&[sick_leave(
        "missing-personnel",
        "  ",
        "2026-01-01",
        "2026-01-02"
    )])
    .is_err());

    let non_overlapping = [
        sick_leave("episode-1", "worker-1", "2026-01-01", "2026-01-02"),
        sick_leave("episode-2", "worker-1", "2026-01-03", "2026-01-04"),
    ];
    assert!(validate_sick_leave_records(&non_overlapping).is_ok());

    let overlapping = [
        sick_leave("episode-1", "worker-1", "2026-01-01", "2026-01-03"),
        sick_leave("episode-2", "worker-1", "2026-01-03", "2026-01-04"),
    ];
    assert!(validate_sick_leave_records(&overlapping).is_err());
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
