/**
 * GENERATED FILE — run `bun scripts/generate-payroll-contract.mjs --write`.
 * Source of truth: crates/payroll-core/src/models.rs
 */
export const RUST_DECIMAL_KEYS = [
  "accrualGrossAmount",
  "adjustedPek",
  "altSinirTamamlamaFarki",
  "amount",
  "asgariGvCumulativeOpening",
  "asgariUcretGvIstisnasi",
  "asgariUcretGvMatrahi",
  "asgariUcretReferansKumulatifMatrahi",
  "aylikDamgaIstisnaHakki",
  "aylikOncekiPekTuketimi",
  "aylikSonrasiPekTuketimi",
  "ayniAyOncekiKullanilanDamgaIstisnasi",
  "ayniAyOncekiKullanilanGvIstisnasi",
  "bes",
  "besOraniYuzde",
  "birlestirilmisSosyalYardim",
  "brutDamgaVergisi",
  "brutGelirVergisi",
  "cariGvMatrahi",
  "dailyMinimumWage",
  "damgaVergisi",
  "damgaVergisiOraniBinde",
  "deltaAmount",
  "devirKumulatifAsgariGvMatrahi",
  "devirKumulatifGvMatrahi",
  "devredenPekAşanTutar",
  "devredenPekKullanilan",
  "digerGelir",
  "digerGelirVarsayilan",
  "digerKesinti",
  "digerKesintiTutar",
  "dogumAskerlikBorclanmasi",
  "dogumAskerlikBorclanmasiTutar",
  "dogumAskerlikGvIndirimi",
  "dogumAskerlikGvIndirimTutar",
  "ekOdeme",
  "employerLowerBoundDelta",
  "employerLowerBoundPremiumDelta",
  "employerSgkDelta",
  "employerUnemploymentDelta",
  "finalPek",
  "geceCalismaPrimiYuzde",
  "geceCalismasiTatiliUcreti",
  "geceCalismasiUcreti",
  "geceCalismaTatiliPrimiYuzde",
  "gelirToplam",
  "gelirVergisi",
  "gelirVergisiOraniYuzde",
  "giyimYardimi",
  "grossAmount",
  "gunlukAsgariUcret",
  "gunlukIsPrimi",
  "gunlukTabanUcret",
  "gunlukVasitaYol",
  "gunlukYemek",
  "gunlukYemekIstisnasiGV",
  "gunlukYemekIstisnasiSGK",
  "gvCumulativeOpening",
  "gvExemptionUsed",
  "gvReferansGunlukAsgariUcret",
  "gvTaxableBase",
  "gvYemekIstisnasiToplam",
  "hamPek",
  "hayatSaglikSigortasi",
  "hayatSaglikSigortasiTutar",
  "hayatSigortasiPrimiTutar",
  "hesaplananPek",
  "hizmetZammi",
  "hizmetZammiBirimi",
  "icra",
  "icraTutar",
  "insuranceGvDeductionUsed",
  "isciIssizlikPrimi",
  "isciSgkPrimi",
  "isPrimi",
  "isPrimiYuzde",
  "issizlikIsciOraniYuzde",
  "issizlikIsverenOraniYuzde",
  "isverenIssizlikOraniYuzde",
  "isverenIssizlikPrimi",
  "isverenPrimToplami",
  "isverenSgkPrimi",
  "kalanDamgaIstisnasi",
  "kesilenDamgaVergisi",
  "kesilenGelirVergisi",
  "kesintiToplam",
  "kisiBorcu",
  "kisiBorcuTutar",
  "limit",
  "manuelKumulatifGvMatrahi",
  "mealSgkExemption",
  "monthlyGvBase",
  "monthPekConsumptionAfter",
  "nafaka",
  "nafakaTutar",
  "netOdeme",
  "offsetSettlementAmount",
  "oksOraniYuzde",
  "oncekiKumulatifAsgariGvMatrahi",
  "oncekiKumulatifGvMatrahi",
  "oran",
  "originalEmployerLowerBound",
  "originalPek",
  "originalRecognizedAmount",
  "outstandingReceivable",
  "payableSettlementAmount",
  "pekAltSinir",
  "pekAltSinirTamamlamaIsverenPrimi",
  "pekCeiling",
  "pekPrimeBase",
  "pekTavanKatsayisi",
  "pekUstSinir",
  "persistedGvBase",
  "previousAnnualInsuranceGvDeduction",
  "previousAuthoritativeRetroAmount",
  "previousCumulativeGv",
  "previousCumulativeMinimumWageGv",
  "primMatrahi",
  "recoverableAmount",
  "recoveredAmount",
  "retroPekDelta",
  "sabitBesTutar",
  "sabitSendikaAidati",
  "sabitTutar",
  "saglikSigortasiPrimiTutar",
  "samePaymentMonthGvExemptionUsed",
  "samePaymentMonthPekUsed",
  "samePaymentMonthStampExemptionUsed",
  "sendikaAidati",
  "sendikaAidatiYuzde",
  "sgkIsciOraniYuzde",
  "sgkIsverenOraniYuzde",
  "sgkWorkerRatePercent",
  "sgkYemekIstisnasiToplam",
  "sigortaGvAylikLimiti",
  "sigortaGvIndirimAdayi",
  "sigortaGvYillikBrutAsgariUcretTavani",
  "sigortaGvYillikKalanLimiti",
  "stampExemptionUsed",
  "tabanBrutAylik",
  "tahakkukOncesiKalanGvIstisnasi",
  "tahakkukSonrasiKalanGvIstisnasi",
  "targetAmount",
  "targetEmployerLowerBound",
  "tediye",
  "tisIkramiyesi",
  "totalGrossDelta",
  "tutar",
  "unemploymentWorkerRatePercent",
  "uygulanabilirSigortaGvIndirimi",
  "uygulananDamgaIstisnasi",
  "uygulananGvIstisnasi",
  "value",
  "vasitaYol",
  "workerSgkDelta",
  "workerUnemploymentDelta",
  "yemek",
  "yemekIstisnasiTutar",
  "yeniKumulatifGvMatrahi"
] as const;

export const RUST_ENUM_VALUES = {
  "AccrualType": [
    "NORMAL",
    "TEDIYE",
    "TIS_IKRAMIYE",
    "SUPPLEMENTAL",
    "RETRO_ADJUSTMENT"
  ],
  "CompensationRevisionReason": [
    "COLLECTIVE_AGREEMENT",
    "ADMINISTRATIVE_DECISION",
    "COURT_DECISION",
    "PAY_CORRECTION",
    "MISSING_ACCRUAL",
    "OTHER"
  ],
  "CompensationRevisionStatus": [
    "DRAFT",
    "CALCULATED",
    "STALE",
    "FINALIZED"
  ],
  "CompensationRevisionScope": [
    "ALL_PERSONNEL",
    "SELECTED_PERSONNEL",
    "PERSONNEL_GROUP"
  ],
  "RetroSettlementStatus": [
    "UNSETTLED",
    "PAID",
    "OVERPAYMENT",
    "SETTLED_BY_OFFSET"
  ],
  "RetroParameterKey": [
    "GUNLUK_TABAN_UCRET",
    "GUNLUK_YEMEK",
    "BIRLESTIRILMIS_SOSYAL_YARDIM",
    "GUNLUK_VASITA_YOL",
    "GIYIM_YARDIMI",
    "HIZMET_ZAMMI_BIRIMI",
    "IS_PRIMI_YUZDE",
    "GECE_CALISMA_PRIMI_YUZDE",
    "GECE_CALISMA_TATILI_PRIMI_YUZDE",
    "EK_ODEME",
    "DIGER_GELIR",
    "TEDIYE",
    "TIS_BONUS"
  ],
  "RetroEarningCode": [
    "BASE_WAGE",
    "NIGHT_WORK",
    "NIGHT_HOLIDAY",
    "WORK_PREMIUM",
    "SOCIAL_AID",
    "MEAL",
    "TRANSPORT",
    "CLOTHING",
    "SERVICE_INCREMENT",
    "TIS_BONUS",
    "TEDIYE",
    "SUPPLEMENTAL",
    "OTHER"
  ],
  "RetroTaxTreatment": [
    "TAXABLE",
    "EXEMPT"
  ],
  "RetroSgkTreatment": [
    "WAGE_SOURCE_MONTH",
    "NON_WAGE_PAYMENT_MONTH",
    "NON_WAGE_CARRY",
    "EXEMPT"
  ],
  "BordroStatus": [
    "DRAFT",
    "CALCULATED",
    "STALE",
    "FINALIZED"
  ],
  "CompensationBoundaryScope": [
    "FULL_EARNING",
    "ROAD_ONLY"
  ],
  "RoadRoundingMode": [
    "PER_COMPENSATION_SLICE",
    "PERIOD_TOTAL"
  ],
  "PayrollCalculationProvenance": [
    "COMPENSATION_POLICY_ENGINE"
  ],
  "PayrollAttendanceCoverage": [
    "COMPLETE",
    "PARTIAL"
  ],
  "PayrollTaxOpeningSource": [
    "EXPLICIT_OPENING",
    "LEGACY_PERSONNEL_VALUE",
    "IMPLICIT_ZERO"
  ],
  "PayrollPekCarrySource": [
    "NONE",
    "PAYMENT_EVENT",
    "RETRO_OVERRIDE"
  ],
  "StatutorySnapshotSource": [
    "ATTENDANCE_BACKED",
    "PROVISIONAL_PAYMENT_MONTH",
    "LEGACY_UNKNOWN"
  ]
} as const;

export const RUST_STRUCT_CONTRACT = {
  "GvIndirimGirdileri": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "dogumAskerlikGvIndirimTutar": {
        "rustName": "dogumAskerlikGvIndirimTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "hayatSigortasiPrimiTutar": {
        "rustName": "hayatSigortasiPrimiTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "saglikSigortasiPrimiTutar": {
        "rustName": "saglikSigortasiPrimiTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PersonelKesintileri": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "sendikaUyesi": {
        "rustName": "sendikaUyesi",
        "rustType": "Option<bool>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sabitSendikaAidati": {
        "rustName": "sabitSendikaAidati",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "besUyesi": {
        "rustName": "besUyesi",
        "rustType": "Option<bool>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "oksOraniYuzde": {
        "rustName": "oksOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitBesTutar": {
        "rustName": "sabitBesTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "icraTutar": {
        "rustName": "icraTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "nafakaTutar": {
        "rustName": "nafakaTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "kisiBorcuTutar": {
        "rustName": "kisiBorcuTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "dogumAskerlikBorclanmasiTutar": {
        "rustName": "dogumAskerlikBorclanmasiTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "hayatSaglikSigortasiTutar": {
        "rustName": "hayatSaglikSigortasiTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "digerKesintiTutar": {
        "rustName": "digerKesintiTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gvIndirimleri": {
        "rustName": "gvIndirimleri",
        "rustType": "Option<GvIndirimGirdileri>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "GvIndirimGirdileri"
        ]
      }
    }
  },
  "Personel": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "tcNo": {
        "rustName": "tcNo",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "ad": {
        "rustName": "ad",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "soyad": {
        "rustName": "soyad",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "grup": {
        "rustName": "grup",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "unvan": {
        "rustName": "unvan",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sgkSicilNo": {
        "rustName": "sgkSicilNo",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "iban": {
        "rustName": "iban",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "hizmetYili": {
        "rustName": "hizmetYili",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "aciklama": {
        "rustName": "aciklama",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "devirKumulatifGvMatrahi": {
        "rustName": "devirKumulatifGvMatrahi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "devirKumulatifGvMatrahiYili": {
        "rustName": "devirKumulatifGvMatrahiYili",
        "rustType": "Option<i32>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "devirKumulatifGvMatrahiBaslangicAyi": {
        "rustName": "devirKumulatifGvMatrahiBaslangicAyi",
        "rustType": "Option<i32>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "devirKumulatifAsgariGvMatrahi": {
        "rustName": "devirKumulatifAsgariGvMatrahi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "devirKumulatifAsgariGvMatrahiYili": {
        "rustName": "devirKumulatifAsgariGvMatrahiYili",
        "rustType": "Option<i32>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "kesintiler": {
        "rustName": "kesintiler",
        "rustType": "Option<PersonelKesintileri>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PersonelKesintileri"
        ]
      }
    }
  },
  "BordroDonemi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "yil": {
        "rustName": "yil",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "ay": {
        "rustName": "ay",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "baslangicTarihi": {
        "rustName": "baslangicTarihi",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "bitisTarihi": {
        "rustName": "bitisTarihi",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "donemAdi": {
        "rustName": "donemAdi",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "taxYear": {
        "rustName": "taxYear",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "taxMonth": {
        "rustName": "taxMonth",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PersonelTaxOpening": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personnelId": {
        "rustName": "personnelId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "year": {
        "rustName": "year",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gvCumulativeOpening": {
        "rustName": "gvCumulativeOpening",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "effectiveFromPeriodId": {
        "rustName": "effectiveFromPeriodId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "asgariGvCumulativeOpening": {
        "rustName": "asgariGvCumulativeOpening",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "asgariGvEffectiveFromPeriodId": {
        "rustName": "asgariGvEffectiveFromPeriodId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "createdAt": {
        "rustName": "createdAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "updatedAt": {
        "rustName": "updatedAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "TaxBracket": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "limit": {
        "rustName": "limit",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "oran": {
        "rustName": "oran",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "AnnualPayrollParameters": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "year": {
        "rustName": "year",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gelirVergisiDilimleri": {
        "rustName": "gelirVergisiDilimleri",
        "rustType": "Vec<TaxBracket>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "TaxBracket"
        ]
      },
      "sigortaGvYillikBrutAsgariUcretTavani": {
        "rustName": "sigortaGvYillikBrutAsgariUcretTavani",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "updatedAt": {
        "rustName": "updatedAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PersonelPuantaj": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personelId": {
        "rustName": "personelId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "donemId": {
        "rustName": "donemId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunler": {
        "rustName": "gunler",
        "rustType": "HashMap<String, String>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PuantajOzeti": {
    "renameAll": null,
    "default": true,
    "fields": {
      "Ç": {
        "rustName": "c",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "T": {
        "rustName": "t",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "G": {
        "rustName": "g",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "İ": {
        "rustName": "i",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "GÇ": {
        "rustName": "gc",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "GÇT": {
        "rustName": "gct",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "R": {
        "rustName": "r",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "GelirKalemleri": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "tabanBrutAylik": {
        "rustName": "tabanBrutAylik",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "tediye": {
        "rustName": "tediye",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "tisIkramiyesi": {
        "rustName": "tisIkramiyesi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "ekOdeme": {
        "rustName": "ekOdeme",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "yemek": {
        "rustName": "yemek",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "birlestirilmisSosyalYardim": {
        "rustName": "birlestirilmisSosyalYardim",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "vasitaYol": {
        "rustName": "vasitaYol",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "giyimYardimi": {
        "rustName": "giyimYardimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isPrimi": {
        "rustName": "isPrimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "geceCalismasiUcreti": {
        "rustName": "geceCalismasiUcreti",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "geceCalismasiTatiliUcreti": {
        "rustName": "geceCalismasiTatiliUcreti",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "hizmetZammi": {
        "rustName": "hizmetZammi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "digerGelir": {
        "rustName": "digerGelir",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "ManualPayrollIncomeInput": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "tediye": {
        "rustName": "tediye",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "tisIkramiyesi": {
        "rustName": "tisIkramiyesi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "CompensationRevision": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "reason": {
        "rustName": "reason",
        "rustType": "CompensationRevisionReason",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationRevisionReason"
        ]
      },
      "title": {
        "rustName": "title",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "effectiveFrom": {
        "rustName": "effectiveFrom",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "effectiveTo": {
        "rustName": "effectiveTo",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "decisionDate": {
        "rustName": "decisionDate",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "signedAt": {
        "rustName": "signedAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "description": {
        "rustName": "description",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "status": {
        "rustName": "status",
        "rustType": "CompensationRevisionStatus",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationRevisionStatus"
        ]
      },
      "scope": {
        "rustName": "scope",
        "rustType": "CompensationRevisionScope",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationRevisionScope"
        ]
      },
      "personnelIds": {
        "rustName": "personnelIds",
        "rustType": "Vec<String>",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personnelGroup": {
        "rustName": "personnelGroup",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "createdAt": {
        "rustName": "createdAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "updatedAt": {
        "rustName": "updatedAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "CompensationRevisionOverride": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "revisionId": {
        "rustName": "revisionId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "parameter": {
        "rustName": "parameter",
        "rustType": "RetroParameterKey",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RetroParameterKey"
        ]
      },
      "value": {
        "rustName": "value",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "personnelId": {
        "rustName": "personnelId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "RetroAdjustmentBatch": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "revisionId": {
        "rustName": "revisionId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personnelId": {
        "rustName": "personnelId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "paymentDate": {
        "rustName": "paymentDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "status": {
        "rustName": "status",
        "rustType": "CompensationRevisionStatus",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationRevisionStatus"
        ]
      },
      "settlementStatus": {
        "rustName": "settlementStatus",
        "rustType": "RetroSettlementStatus",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RetroSettlementStatus"
        ]
      },
      "totalGrossDelta": {
        "rustName": "totalGrossDelta",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "payableSettlementAmount": {
        "rustName": "payableSettlementAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "offsetSettlementAmount": {
        "rustName": "offsetSettlementAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "recoveredAmount": {
        "rustName": "recoveredAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "recoverableAmount": {
        "rustName": "recoverableAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "outstandingReceivable": {
        "rustName": "outstandingReceivable",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "description": {
        "rustName": "description",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "createdAt": {
        "rustName": "createdAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "calculatedAt": {
        "rustName": "calculatedAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "finalizedAt": {
        "rustName": "finalizedAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "RetroAllocation": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "batchId": {
        "rustName": "batchId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personnelId": {
        "rustName": "personnelId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sourcePeriodId": {
        "rustName": "sourcePeriodId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "earningCode": {
        "rustName": "earningCode",
        "rustType": "RetroEarningCode",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RetroEarningCode"
        ]
      },
      "originalRecognizedAmount": {
        "rustName": "originalRecognizedAmount",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "previousAuthoritativeRetroAmount": {
        "rustName": "previousAuthoritativeRetroAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "targetAmount": {
        "rustName": "targetAmount",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "deltaAmount": {
        "rustName": "deltaAmount",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkTreatment": {
        "rustName": "sgkTreatment",
        "rustType": "RetroSgkTreatment",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RetroSgkTreatment"
        ]
      },
      "incomeTaxTreatment": {
        "rustName": "incomeTaxTreatment",
        "rustType": "RetroTaxTreatment",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RetroTaxTreatment"
        ]
      },
      "stampTaxTreatment": {
        "rustName": "stampTaxTreatment",
        "rustType": "RetroTaxTreatment",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RetroTaxTreatment"
        ]
      },
      "originalPek": {
        "rustName": "originalPek",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "retroPekDelta": {
        "rustName": "retroPekDelta",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "adjustedPek": {
        "rustName": "adjustedPek",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "workerSgkDelta": {
        "rustName": "workerSgkDelta",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "workerUnemploymentDelta": {
        "rustName": "workerUnemploymentDelta",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "employerSgkDelta": {
        "rustName": "employerSgkDelta",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "employerUnemploymentDelta": {
        "rustName": "employerUnemploymentDelta",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "originalEmployerLowerBound": {
        "rustName": "originalEmployerLowerBound",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "targetEmployerLowerBound": {
        "rustName": "targetEmployerLowerBound",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "employerLowerBoundDelta": {
        "rustName": "employerLowerBoundDelta",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "employerLowerBoundPremiumDelta": {
        "rustName": "employerLowerBoundPremiumDelta",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "originalSourceCarry": {
        "rustName": "originalSourceCarry",
        "rustType": "Option<Vec<DevredenPekKaydi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "DevredenPekKaydi"
        ]
      },
      "targetSourceCarry": {
        "rustName": "targetSourceCarry",
        "rustType": "Option<Vec<DevredenPekKaydi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "DevredenPekKaydi"
        ]
      },
      "payableSettlementAmount": {
        "rustName": "payableSettlementAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "offsetSettlementAmount": {
        "rustName": "offsetSettlementAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "recoverableAmount": {
        "rustName": "recoverableAmount",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "metadata": {
        "rustName": "metadata",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PayrollAccrualInput": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "accrualId": {
        "rustName": "accrualId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "accrualType": {
        "rustName": "accrualType",
        "rustType": "AccrualType",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "AccrualType"
        ]
      },
      "paymentDate": {
        "rustName": "paymentDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sequence": {
        "rustName": "sequence",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "grossAmount": {
        "rustName": "grossAmount",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "description": {
        "rustName": "description",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "KesintiKalemleri": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "isciSgkPrimi": {
        "rustName": "isciSgkPrimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isciIssizlikPrimi": {
        "rustName": "isciIssizlikPrimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gelirVergisi": {
        "rustName": "gelirVergisi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "damgaVergisi": {
        "rustName": "damgaVergisi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sendikaAidati": {
        "rustName": "sendikaAidati",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "bes": {
        "rustName": "bes",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "icra": {
        "rustName": "icra",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "nafaka": {
        "rustName": "nafaka",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "kisiBorcu": {
        "rustName": "kisiBorcu",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "dogumAskerlikBorclanmasi": {
        "rustName": "dogumAskerlikBorclanmasi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "hayatSaglikSigortasi": {
        "rustName": "hayatSaglikSigortasi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "digerKesinti": {
        "rustName": "digerKesinti",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "SickLeaveRecord": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personnelId": {
        "rustName": "personnelId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "startDate": {
        "rustName": "startDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "endDate": {
        "rustName": "endDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "createdAt": {
        "rustName": "createdAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "updatedAt": {
        "rustName": "updatedAt",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "DevredenPekKaydi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "tutar": {
        "rustName": "tutar",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "kalanAySayisi": {
        "rustName": "kalanAySayisi",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "kaynakDonemId": {
        "rustName": "kaynakDonemId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PekDetayi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "hesaplananPek": {
        "rustName": "hesaplananPek",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "hamPek": {
        "rustName": "hamPek",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "devredenPekKullanilan": {
        "rustName": "devredenPekKullanilan",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "primMatrahi": {
        "rustName": "primMatrahi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "aylikOncekiPekTuketimi": {
        "rustName": "aylikOncekiPekTuketimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "aylikSonrasiPekTuketimi": {
        "rustName": "aylikSonrasiPekTuketimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "finalPek": {
        "rustName": "finalPek",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "devredenPekAşanTutar": {
        "rustName": "devredenPekAşanTutar",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "pekAltSinir": {
        "rustName": "pekAltSinir",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "pekUstSinir": {
        "rustName": "pekUstSinir",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "altSinirTamamlamaFarki": {
        "rustName": "altSinirTamamlamaFarki",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "fiiliYemekGunu": {
        "rustName": "fiiliYemekGunu",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "yemekIstisnasiTutar": {
        "rustName": "yemekIstisnasiTutar",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "isverenSgkPrimi": {
        "rustName": "isverenSgkPrimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isverenIssizlikPrimi": {
        "rustName": "isverenIssizlikPrimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "pekAltSinirTamamlamaIsverenPrimi": {
        "rustName": "pekAltSinirTamamlamaIsverenPrimi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isverenPrimToplami": {
        "rustName": "isverenPrimToplami",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkIsverenOraniYuzde": {
        "rustName": "sgkIsverenOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isverenIssizlikOraniYuzde": {
        "rustName": "isverenIssizlikOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollIncomeItem": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "payrollId": {
        "rustName": "payrollId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "itemType": {
        "rustName": "itemType",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "description": {
        "rustName": "description",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "amount": {
        "rustName": "amount",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "source": {
        "rustName": "source",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PayrollDeductionItem": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "payrollId": {
        "rustName": "payrollId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "itemType": {
        "rustName": "itemType",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "description": {
        "rustName": "description",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "amount": {
        "rustName": "amount",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "source": {
        "rustName": "source",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "BordroKaydi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personelId": {
        "rustName": "personelId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "donemId": {
        "rustName": "donemId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "accrualId": {
        "rustName": "accrualId",
        "rustType": "String",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "accrualType": {
        "rustName": "accrualType",
        "rustType": "AccrualType",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "AccrualType"
        ]
      },
      "paymentDate": {
        "rustName": "paymentDate",
        "rustType": "String",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sequence": {
        "rustName": "sequence",
        "rustType": "i32",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "accrualDescription": {
        "rustName": "accrualDescription",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "puantajOzeti": {
        "rustName": "puantajOzeti",
        "rustType": "PuantajOzeti",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PuantajOzeti"
        ]
      },
      "gelirler": {
        "rustName": "gelirler",
        "rustType": "GelirKalemleri",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "GelirKalemleri"
        ]
      },
      "gelirToplam": {
        "rustName": "gelirToplam",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "kesintiler": {
        "rustName": "kesintiler",
        "rustType": "KesintiKalemleri",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "KesintiKalemleri"
        ]
      },
      "kesintiToplam": {
        "rustName": "kesintiToplam",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "netOdeme": {
        "rustName": "netOdeme",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "status": {
        "rustName": "status",
        "rustType": "BordroStatus",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "BordroStatus"
        ]
      },
      "olusturulmaTarihi": {
        "rustName": "olusturulmaTarihi",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sonGuncellemeTarihi": {
        "rustName": "sonGuncellemeTarihi",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "notlar": {
        "rustName": "notlar",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "oncekiKumulatifGvMatrahi": {
        "rustName": "oncekiKumulatifGvMatrahi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "oncekiKumulatifAsgariGvMatrahi": {
        "rustName": "oncekiKumulatifAsgariGvMatrahi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "manuelKumulatifGvMatrahi": {
        "rustName": "manuelKumulatifGvMatrahi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "devredenPekGelen": {
        "rustName": "devredenPekGelen",
        "rustType": "Option<Vec<DevredenPekKaydi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "DevredenPekKaydi"
        ]
      },
      "sonrakiDevredenPek": {
        "rustName": "sonrakiDevredenPek",
        "rustType": "Option<Vec<DevredenPekKaydi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "DevredenPekKaydi"
        ]
      },
      "pekDetay": {
        "rustName": "pekDetay",
        "rustType": "Option<PekDetayi>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PekDetayi"
        ]
      },
      "isPrimiDetay": {
        "rustName": "isPrimiDetay",
        "rustType": "Option<IsPrimiHesapDetayi>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "IsPrimiHesapDetayi"
        ]
      },
      "gvDetay": {
        "rustName": "gvDetay",
        "rustType": "Option<GvHesapDetayi>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "GvHesapDetayi"
        ]
      },
      "persistedGvBase": {
        "rustName": "persistedGvBase",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "damgaDetay": {
        "rustName": "damgaDetay",
        "rustType": "Option<DamgaVergisiHesapDetayi>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "DamgaVergisiHesapDetayi"
        ]
      },
      "statutorySnapshot": {
        "rustName": "statutorySnapshot",
        "rustType": "Option<ResolvedStatutorySnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "ResolvedStatutorySnapshot"
        ]
      },
      "odenenRaporluGun": {
        "rustName": "odenenRaporluGun",
        "rustType": "Option<i32>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "raporluGun": {
        "rustName": "raporluGun",
        "rustType": "Option<i32>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "calculationMetadata": {
        "rustName": "calculationMetadata",
        "rustType": "Option<PayrollCalculationMetadata>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PayrollCalculationMetadata"
        ]
      }
    }
  },
  "TediyeKalemi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "ad": {
        "rustName": "ad",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "odemeAyi": {
        "rustName": "odemeAyi",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunSayisi": {
        "rustName": "gunSayisi",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "aktifDonemdeOdensin": {
        "rustName": "aktifDonemdeOdensin",
        "rustType": "bool",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sabitTutar": {
        "rustName": "sabitTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "TisIkramiyeKalemi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "ad": {
        "rustName": "ad",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "odemeAyi": {
        "rustName": "odemeAyi",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunSayisi": {
        "rustName": "gunSayisi",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "aktifDonemdeOdensin": {
        "rustName": "aktifDonemdeOdensin",
        "rustType": "bool",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sabitTutar": {
        "rustName": "sabitTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "IsPrimiGrupItem": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "ad": {
        "rustName": "ad",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "oran": {
        "rustName": "oran",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "aktif": {
        "rustName": "aktif",
        "rustType": "bool",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "CompensationPolicyState": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "gunlukTabanUcret": {
        "rustName": "gunlukTabanUcret",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemek": {
        "rustName": "gunlukYemek",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "birlestirilmisSosyalYardim": {
        "rustName": "birlestirilmisSosyalYardim",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukVasitaYol": {
        "rustName": "gunlukVasitaYol",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "giyimYardimi": {
        "rustName": "giyimYardimi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "hizmetZammiBirimi": {
        "rustName": "hizmetZammiBirimi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "isPrimiYuzde": {
        "rustName": "isPrimiYuzde",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isPrimiGruplari": {
        "rustName": "isPrimiGruplari",
        "rustType": "Vec<CompensationPolicyGroup>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationPolicyGroup"
        ]
      },
      "geceCalismaPrimiYuzde": {
        "rustName": "geceCalismaPrimiYuzde",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "geceCalismaTatiliPrimiYuzde": {
        "rustName": "geceCalismaTatiliPrimiYuzde",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "ekOdeme": {
        "rustName": "ekOdeme",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "digerGelirVarsayilan": {
        "rustName": "digerGelirVarsayilan",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "CompensationPolicyGroup": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "ad": {
        "rustName": "ad",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "oran": {
        "rustName": "oran",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "aktif": {
        "rustName": "aktif",
        "rustType": "bool",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "CompensationPolicyVersion": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "effectiveFrom": {
        "rustName": "effectiveFrom",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "state": {
        "rustName": "state",
        "rustType": "CompensationPolicyState",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationPolicyState"
        ]
      },
      "boundaryScope": {
        "rustName": "boundaryScope",
        "rustType": "CompensationBoundaryScope",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationBoundaryScope"
        ]
      },
      "roadRoundingMode": {
        "rustName": "roadRoundingMode",
        "rustType": "RoadRoundingMode",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RoadRoundingMode"
        ]
      },
      "createdAt": {
        "rustName": "createdAt",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "source": {
        "rustName": "source",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "reason": {
        "rustName": "reason",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "supersedesVersionId": {
        "rustName": "supersedesVersionId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "ResolvedCompensationSegment": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "versionId": {
        "rustName": "versionId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "startDate": {
        "rustName": "startDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "endDate": {
        "rustName": "endDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "state": {
        "rustName": "state",
        "rustType": "CompensationPolicyState",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationPolicyState"
        ]
      },
      "boundaryScope": {
        "rustName": "boundaryScope",
        "rustType": "CompensationBoundaryScope",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "CompensationBoundaryScope"
        ]
      },
      "roadRoundingMode": {
        "rustName": "roadRoundingMode",
        "rustType": "RoadRoundingMode",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "RoadRoundingMode"
        ]
      }
    }
  },
  "PayrollCalculationMetadata": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "provenance": {
        "rustName": "provenance",
        "rustType": "PayrollCalculationProvenance",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollCalculationProvenance"
        ]
      },
      "snapshot": {
        "rustName": "snapshot",
        "rustType": "Option<PayrollCalculationSnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PayrollCalculationSnapshot"
        ]
      }
    }
  },
  "PayrollCalculationSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "snapshotSchemaVersion": {
        "rustName": "snapshotSchemaVersion",
        "rustType": "u16",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "calculationVersion": {
        "rustName": "calculationVersion",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "compensation": {
        "rustName": "compensation",
        "rustType": "ResolvedCompensationSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "ResolvedCompensationSnapshot"
        ]
      },
      "usedInputs": {
        "rustName": "usedInputs",
        "rustType": "PayrollUsedInputSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollUsedInputSnapshot"
        ]
      }
    }
  },
  "ResolvedCompensationSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "segments": {
        "rustName": "segments",
        "rustType": "Vec<ResolvedCompensationSegment>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "ResolvedCompensationSegment"
        ]
      }
    }
  },
  "PayrollUsedInputSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "personnelGroup": {
        "rustName": "personnelGroup",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "serviceYear": {
        "rustName": "serviceYear",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "personalDeductions": {
        "rustName": "personalDeductions",
        "rustType": "Option<PayrollPersonalDeductionInputsSnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PayrollPersonalDeductionInputsSnapshot"
        ]
      },
      "attendance": {
        "rustName": "attendance",
        "rustType": "Option<PayrollAttendanceInputSnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PayrollAttendanceInputSnapshot"
        ]
      },
      "paidSickLeave": {
        "rustName": "paidSickLeave",
        "rustType": "Option<PayrollPaidSickLeaveInputSnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PayrollPaidSickLeaveInputSnapshot"
        ]
      },
      "manualIncome": {
        "rustName": "manualIncome",
        "rustType": "PayrollManualIncomeInputSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollManualIncomeInputSnapshot"
        ]
      },
      "accrualGrossAmount": {
        "rustName": "accrualGrossAmount",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "institutionDeductionSettings": {
        "rustName": "institutionDeductionSettings",
        "rustType": "PayrollInstitutionDeductionInputsSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollInstitutionDeductionInputsSnapshot"
        ]
      },
      "statutoryParameters": {
        "rustName": "statutoryParameters",
        "rustType": "PayrollStatutoryInputsSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollStatutoryInputsSnapshot"
        ]
      },
      "annualParameters": {
        "rustName": "annualParameters",
        "rustType": "PayrollAnnualParametersSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollAnnualParametersSnapshot"
        ]
      },
      "taxHistory": {
        "rustName": "taxHistory",
        "rustType": "PayrollTaxHistoryInputsSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollTaxHistoryInputsSnapshot"
        ]
      },
      "incomingPekCarry": {
        "rustName": "incomingPekCarry",
        "rustType": "PayrollIncomingPekCarrySnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollIncomingPekCarrySnapshot"
        ]
      }
    }
  },
  "PayrollPersonalDeductionInputsSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "sendikaUyesi": {
        "rustName": "sendikaUyesi",
        "rustType": "Option<bool>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sabitSendikaAidati": {
        "rustName": "sabitSendikaAidati",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "besUyesi": {
        "rustName": "besUyesi",
        "rustType": "Option<bool>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "oksOraniYuzde": {
        "rustName": "oksOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitBesTutar": {
        "rustName": "sabitBesTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "icraTutar": {
        "rustName": "icraTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "nafakaTutar": {
        "rustName": "nafakaTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "kisiBorcuTutar": {
        "rustName": "kisiBorcuTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "dogumAskerlikBorclanmasiTutar": {
        "rustName": "dogumAskerlikBorclanmasiTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "hayatSaglikSigortasiTutar": {
        "rustName": "hayatSaglikSigortasiTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "digerKesintiTutar": {
        "rustName": "digerKesintiTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gvIndirimleri": {
        "rustName": "gvIndirimleri",
        "rustType": "Option<PayrollGvDeductionInputsSnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "PayrollGvDeductionInputsSnapshot"
        ]
      }
    }
  },
  "PayrollGvDeductionInputsSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "dogumAskerlikGvIndirimTutar": {
        "rustName": "dogumAskerlikGvIndirimTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "hayatSigortasiPrimiTutar": {
        "rustName": "hayatSigortasiPrimiTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "saglikSigortasiPrimiTutar": {
        "rustName": "saglikSigortasiPrimiTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollAttendanceInputSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "coverage": {
        "rustName": "coverage",
        "rustType": "PayrollAttendanceCoverage",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollAttendanceCoverage"
        ]
      },
      "days": {
        "rustName": "days",
        "rustType": "Vec<PayrollAttendanceDaySnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollAttendanceDaySnapshot"
        ]
      }
    }
  },
  "PayrollAttendanceDaySnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "date": {
        "rustName": "date",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "code": {
        "rustName": "code",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PayrollPaidSickLeaveInputSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "ruleVersion": {
        "rustName": "ruleVersion",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sourceEpisodes": {
        "rustName": "sourceEpisodes",
        "rustType": "Vec<PayrollSickLeaveEpisodeSnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollSickLeaveEpisodeSnapshot"
        ]
      },
      "paidDates": {
        "rustName": "paidDates",
        "rustType": "Vec<String>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PayrollSickLeaveEpisodeSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "id": {
        "rustName": "id",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "startDate": {
        "rustName": "startDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "endDate": {
        "rustName": "endDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "PayrollManualIncomeInputSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "supplied": {
        "rustName": "supplied",
        "rustType": "bool",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "tediye": {
        "rustName": "tediye",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "tisIkramiyesi": {
        "rustName": "tisIkramiyesi",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollInstitutionDeductionInputsSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "yemekTamGvIstisnaDamgaTabi": {
        "rustName": "yemekTamGvIstisnaDamgaTabi",
        "rustType": "bool",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "damgaVergisiOraniBinde": {
        "rustName": "damgaVergisiOraniBinde",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sendikaAidatiYuzde": {
        "rustName": "sendikaAidatiYuzde",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitSendikaAidati": {
        "rustName": "sabitSendikaAidati",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "besOraniYuzde": {
        "rustName": "besOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitBesTutar": {
        "rustName": "sabitBesTutar",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollStatutoryInputsSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "gunlukAsgariUcret": {
        "rustName": "gunlukAsgariUcret",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkIsciOraniYuzde": {
        "rustName": "sgkIsciOraniYuzde",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsciOraniYuzde": {
        "rustName": "issizlikIsciOraniYuzde",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "pekTavanKatsayisi": {
        "rustName": "pekTavanKatsayisi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiSGK": {
        "rustName": "gunlukYemekIstisnasiSGK",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiGV": {
        "rustName": "gunlukYemekIstisnasiGV",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkIsverenOraniYuzde": {
        "rustName": "sgkIsverenOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsverenOraniYuzde": {
        "rustName": "issizlikIsverenOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "statutoryParameterSegments": {
        "rustName": "statutoryParameterSegments",
        "rustType": "Vec<PayrollStatutoryInputSegmentSnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollStatutoryInputSegmentSnapshot"
        ]
      }
    }
  },
  "PayrollStatutoryInputSegmentSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "effectiveFrom": {
        "rustName": "effectiveFrom",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunlukAsgariUcret": {
        "rustName": "gunlukAsgariUcret",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "pekTavanKatsayisi": {
        "rustName": "pekTavanKatsayisi",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiSGK": {
        "rustName": "gunlukYemekIstisnasiSGK",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiGV": {
        "rustName": "gunlukYemekIstisnasiGV",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollAnnualParametersSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "year": {
        "rustName": "year",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gelirVergisiDilimleri": {
        "rustName": "gelirVergisiDilimleri",
        "rustType": "Vec<PayrollTaxBracketSnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollTaxBracketSnapshot"
        ]
      },
      "sigortaGvYillikBrutAsgariUcretTavani": {
        "rustName": "sigortaGvYillikBrutAsgariUcretTavani",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollTaxBracketSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "limit": {
        "rustName": "limit",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "oran": {
        "rustName": "oran",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollTaxOpeningSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "source": {
        "rustName": "source",
        "rustType": "PayrollTaxOpeningSource",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollTaxOpeningSource"
        ]
      },
      "openingId": {
        "rustName": "openingId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "effectiveFromTaxYear": {
        "rustName": "effectiveFromTaxYear",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "effectiveFromTaxMonth": {
        "rustName": "effectiveFromTaxMonth",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "amount": {
        "rustName": "amount",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollTaxHistoryInputsSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "normalGvOpening": {
        "rustName": "normalGvOpening",
        "rustType": "PayrollTaxOpeningSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollTaxOpeningSnapshot"
        ]
      },
      "minimumWageGvOpening": {
        "rustName": "minimumWageGvOpening",
        "rustType": "PayrollTaxOpeningSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollTaxOpeningSnapshot"
        ]
      },
      "previousCumulativeGv": {
        "rustName": "previousCumulativeGv",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "previousCumulativeMinimumWageGv": {
        "rustName": "previousCumulativeMinimumWageGv",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "previousAnnualInsuranceGvDeduction": {
        "rustName": "previousAnnualInsuranceGvDeduction",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "samePaymentMonthPekUsed": {
        "rustName": "samePaymentMonthPekUsed",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "samePaymentMonthGvExemptionUsed": {
        "rustName": "samePaymentMonthGvExemptionUsed",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "samePaymentMonthStampExemptionUsed": {
        "rustName": "samePaymentMonthStampExemptionUsed",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "previousPaymentEvents": {
        "rustName": "previousPaymentEvents",
        "rustType": "Vec<PayrollPriorPaymentEventSnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollPriorPaymentEventSnapshot"
        ]
      },
      "minimumWageMonthlyReferences": {
        "rustName": "minimumWageMonthlyReferences",
        "rustType": "Vec<PayrollMinimumWageMonthInputSnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollMinimumWageMonthInputSnapshot"
        ]
      }
    }
  },
  "PayrollPriorPaymentEventSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "periodId": {
        "rustName": "periodId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "accrualId": {
        "rustName": "accrualId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "accrualType": {
        "rustName": "accrualType",
        "rustType": "AccrualType",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "AccrualType"
        ]
      },
      "taxYear": {
        "rustName": "taxYear",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "taxMonth": {
        "rustName": "taxMonth",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "paymentDate": {
        "rustName": "paymentDate",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sequence": {
        "rustName": "sequence",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gvTaxableBase": {
        "rustName": "gvTaxableBase",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "insuranceGvDeductionUsed": {
        "rustName": "insuranceGvDeductionUsed",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gvExemptionUsed": {
        "rustName": "gvExemptionUsed",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "stampExemptionUsed": {
        "rustName": "stampExemptionUsed",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "pekPrimeBase": {
        "rustName": "pekPrimeBase",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "monthPekConsumptionAfter": {
        "rustName": "monthPekConsumptionAfter",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "pekCeiling": {
        "rustName": "pekCeiling",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "mealSgkExemption": {
        "rustName": "mealSgkExemption",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "income": {
        "rustName": "income",
        "rustType": "PayrollIncomeInputsSnapshot",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollIncomeInputsSnapshot"
        ]
      },
      "outgoingPekCarry": {
        "rustName": "outgoingPekCarry",
        "rustType": "Vec<PayrollPekCarrySnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollPekCarrySnapshot"
        ]
      }
    }
  },
  "PayrollIncomeInputsSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "tabanBrutAylik": {
        "rustName": "tabanBrutAylik",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "tediye": {
        "rustName": "tediye",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "tisIkramiyesi": {
        "rustName": "tisIkramiyesi",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "ekOdeme": {
        "rustName": "ekOdeme",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "yemek": {
        "rustName": "yemek",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "birlestirilmisSosyalYardim": {
        "rustName": "birlestirilmisSosyalYardim",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "vasitaYol": {
        "rustName": "vasitaYol",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "giyimYardimi": {
        "rustName": "giyimYardimi",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isPrimi": {
        "rustName": "isPrimi",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "geceCalismasiUcreti": {
        "rustName": "geceCalismasiUcreti",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "geceCalismasiTatiliUcreti": {
        "rustName": "geceCalismasiTatiliUcreti",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "hizmetZammi": {
        "rustName": "hizmetZammi",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "digerGelir": {
        "rustName": "digerGelir",
        "rustType": "Option<Decimal>",
        "required": true,
        "optional": false,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollMinimumWageMonthInputSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "periodId": {
        "rustName": "periodId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "taxYear": {
        "rustName": "taxYear",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "taxMonth": {
        "rustName": "taxMonth",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "dailyMinimumWage": {
        "rustName": "dailyMinimumWage",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkWorkerRatePercent": {
        "rustName": "sgkWorkerRatePercent",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "unemploymentWorkerRatePercent": {
        "rustName": "unemploymentWorkerRatePercent",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "monthlyGvBase": {
        "rustName": "monthlyGvBase",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollIncomingPekCarrySnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "source": {
        "rustName": "source",
        "rustType": "PayrollPekCarrySource",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollPekCarrySource"
        ]
      },
      "sourceId": {
        "rustName": "sourceId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sourcePeriodId": {
        "rustName": "sourcePeriodId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sourceTaxYear": {
        "rustName": "sourceTaxYear",
        "rustType": "Option<i32>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sourceTaxMonth": {
        "rustName": "sourceTaxMonth",
        "rustType": "Option<i32>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "taxMonthsElapsed": {
        "rustName": "taxMonthsElapsed",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "records": {
        "rustName": "records",
        "rustType": "Vec<PayrollPekCarrySnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "PayrollPekCarrySnapshot"
        ]
      }
    }
  },
  "PayrollPekCarrySnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "tutar": {
        "rustName": "tutar",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "kalanAySayisi": {
        "rustName": "kalanAySayisi",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "kaynakDonemId": {
        "rustName": "kaynakDonemId",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      }
    }
  },
  "IsPrimiHesapDetayi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "grupId": {
        "rustName": "grupId",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "grupAd": {
        "rustName": "grupAd",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "oran": {
        "rustName": "oran",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "hakGunu": {
        "rustName": "hakGunu",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunlukIsPrimi": {
        "rustName": "gunlukIsPrimi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "tutar": {
        "rustName": "tutar",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "GvHesapDetayi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "oncekiKumulatifGvMatrahi": {
        "rustName": "oncekiKumulatifGvMatrahi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "cariGvMatrahi": {
        "rustName": "cariGvMatrahi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "yeniKumulatifGvMatrahi": {
        "rustName": "yeniKumulatifGvMatrahi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "brutGelirVergisi": {
        "rustName": "brutGelirVergisi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "asgariUcretGvMatrahi": {
        "rustName": "asgariUcretGvMatrahi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "asgariUcretReferansKumulatifMatrahi": {
        "rustName": "asgariUcretReferansKumulatifMatrahi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "asgariUcretGvIstisnasi": {
        "rustName": "asgariUcretGvIstisnasi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "ayniAyOncekiKullanilanGvIstisnasi": {
        "rustName": "ayniAyOncekiKullanilanGvIstisnasi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "tahakkukOncesiKalanGvIstisnasi": {
        "rustName": "tahakkukOncesiKalanGvIstisnasi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "uygulananGvIstisnasi": {
        "rustName": "uygulananGvIstisnasi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "tahakkukSonrasiKalanGvIstisnasi": {
        "rustName": "tahakkukSonrasiKalanGvIstisnasi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "kesilenGelirVergisi": {
        "rustName": "kesilenGelirVergisi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "dogumAskerlikGvIndirimi": {
        "rustName": "dogumAskerlikGvIndirimi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sigortaGvIndirimAdayi": {
        "rustName": "sigortaGvIndirimAdayi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sigortaGvAylikLimiti": {
        "rustName": "sigortaGvAylikLimiti",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sigortaGvYillikKalanLimiti": {
        "rustName": "sigortaGvYillikKalanLimiti",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "uygulanabilirSigortaGvIndirimi": {
        "rustName": "uygulanabilirSigortaGvIndirimi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "DamgaVergisiHesapDetayi": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "brutDamgaVergisi": {
        "rustName": "brutDamgaVergisi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "aylikDamgaIstisnaHakki": {
        "rustName": "aylikDamgaIstisnaHakki",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "ayniAyOncekiKullanilanDamgaIstisnasi": {
        "rustName": "ayniAyOncekiKullanilanDamgaIstisnasi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "uygulananDamgaIstisnasi": {
        "rustName": "uygulananDamgaIstisnasi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "kalanDamgaIstisnasi": {
        "rustName": "kalanDamgaIstisnasi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "kesilenDamgaVergisi": {
        "rustName": "kesilenDamgaVergisi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "StatutoryParameterSegment": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "effectiveFrom": {
        "rustName": "effectiveFrom",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunlukAsgariUcret": {
        "rustName": "gunlukAsgariUcret",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "pekTavanKatsayisi": {
        "rustName": "pekTavanKatsayisi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiSGK": {
        "rustName": "gunlukYemekIstisnasiSGK",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiGV": {
        "rustName": "gunlukYemekIstisnasiGV",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "ResolvedStatutorySegmentSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "effectiveFrom": {
        "rustName": "effectiveFrom",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "effectiveTo": {
        "rustName": "effectiveTo",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "sgkPrimGunSayisi": {
        "rustName": "sgkPrimGunSayisi",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "fiiliYemekGunu": {
        "rustName": "fiiliYemekGunu",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunlukAsgariUcret": {
        "rustName": "gunlukAsgariUcret",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "pekTavanKatsayisi": {
        "rustName": "pekTavanKatsayisi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiSGK": {
        "rustName": "gunlukYemekIstisnasiSGK",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiGV": {
        "rustName": "gunlukYemekIstisnasiGV",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "ResolvedStatutorySnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "source": {
        "rustName": "source",
        "rustType": "StatutorySnapshotSource",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "StatutorySnapshotSource"
        ]
      },
      "segments": {
        "rustName": "segments",
        "rustType": "Vec<ResolvedStatutorySegmentSnapshot>",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "ResolvedStatutorySegmentSnapshot"
        ]
      },
      "sgkPrimGunSayisi": {
        "rustName": "sgkPrimGunSayisi",
        "rustType": "i32",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "pekAltSinir": {
        "rustName": "pekAltSinir",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "pekUstSinir": {
        "rustName": "pekUstSinir",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkYemekIstisnasiToplam": {
        "rustName": "sgkYemekIstisnasiToplam",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gvYemekIstisnasiToplam": {
        "rustName": "gvYemekIstisnasiToplam",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gvReferansGunlukAsgariUcret": {
        "rustName": "gvReferansGunlukAsgariUcret",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkIsciOraniYuzde": {
        "rustName": "sgkIsciOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsciOraniYuzde": {
        "rustName": "issizlikIsciOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "StatutoryParameterSnapshot": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "gunlukAsgariUcret": {
        "rustName": "gunlukAsgariUcret",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkIsciOraniYuzde": {
        "rustName": "sgkIsciOraniYuzde",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsciOraniYuzde": {
        "rustName": "issizlikIsciOraniYuzde",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "pekTavanKatsayisi": {
        "rustName": "pekTavanKatsayisi",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiSGK": {
        "rustName": "gunlukYemekIstisnasiSGK",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiGV": {
        "rustName": "gunlukYemekIstisnasiGV",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "statutoryParameterSegments": {
        "rustName": "statutoryParameterSegments",
        "rustType": "Vec<StatutoryParameterSegment>",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": [
          "StatutoryParameterSegment"
        ]
      }
    }
  },
  "TransportRateChange": {
    "renameAll": "camelCase",
    "default": false,
    "fields": {
      "effectiveFrom": {
        "rustName": "effectiveFrom",
        "rustType": "String",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunlukVasitaYol": {
        "rustName": "gunlukVasitaYol",
        "rustType": "Decimal",
        "required": true,
        "optional": false,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "DonemselKurumDegerleri": {
    "renameAll": "camelCase",
    "default": true,
    "fields": {
      "donemId": {
        "rustName": "donemId",
        "rustType": "String",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "gunlukTabanUcret": {
        "rustName": "gunlukTabanUcret",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemek": {
        "rustName": "gunlukYemek",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "yemekTamGvIstisnaDamgaTabi": {
        "rustName": "yemekTamGvIstisnaDamgaTabi",
        "rustType": "bool",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "birlestirilmisSosyalYardim": {
        "rustName": "birlestirilmisSosyalYardim",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukVasitaYol": {
        "rustName": "gunlukVasitaYol",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "transportRateChanges": {
        "rustName": "transportRateChanges",
        "rustType": "Option<Vec<TransportRateChange>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "TransportRateChange"
        ]
      },
      "giyimYardimi": {
        "rustName": "giyimYardimi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "hizmetZammiBirimi": {
        "rustName": "hizmetZammiBirimi",
        "rustType": "Decimal",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": true,
        "nestedTypes": []
      },
      "isPrimiYuzde": {
        "rustName": "isPrimiYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "isPrimiGruplari": {
        "rustName": "isPrimiGruplari",
        "rustType": "Option<Vec<IsPrimiGrupItem>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "IsPrimiGrupItem"
        ]
      },
      "geceCalismaPrimiYuzde": {
        "rustName": "geceCalismaPrimiYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "geceCalismaTatiliPrimiYuzde": {
        "rustName": "geceCalismaTatiliPrimiYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "ekOdeme": {
        "rustName": "ekOdeme",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "digerGelirVarsayilan": {
        "rustName": "digerGelirVarsayilan",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "tediyeListesi": {
        "rustName": "tediyeListesi",
        "rustType": "Option<Vec<TediyeKalemi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "TediyeKalemi"
        ]
      },
      "tisIkramiyeListesi": {
        "rustName": "tisIkramiyeListesi",
        "rustType": "Option<Vec<TisIkramiyeKalemi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "TisIkramiyeKalemi"
        ]
      },
      "tediyeTisNotu": {
        "rustName": "tediyeTisNotu",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sgkIsciOraniYuzde": {
        "rustName": "sgkIsciOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsciOraniYuzde": {
        "rustName": "issizlikIsciOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gelirVergisiOraniYuzde": {
        "rustName": "gelirVergisiOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "damgaVergisiOraniBinde": {
        "rustName": "damgaVergisiOraniBinde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sendikaAidatiYuzde": {
        "rustName": "sendikaAidatiYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitSendikaAidati": {
        "rustName": "sabitSendikaAidati",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "besOraniYuzde": {
        "rustName": "besOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitBesTutar": {
        "rustName": "sabitBesTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiSGK": {
        "rustName": "gunlukYemekIstisnasiSGK",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiGV": {
        "rustName": "gunlukYemekIstisnasiGV",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "statutoryParameterSegments": {
        "rustName": "statutoryParameterSegments",
        "rustType": "Option<Vec<StatutoryParameterSegment>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "StatutoryParameterSegment"
        ]
      },
      "statutoryParameterSnapshot": {
        "rustName": "statutoryParameterSnapshot",
        "rustType": "Option<StatutoryParameterSnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "StatutoryParameterSnapshot"
        ]
      },
      "pekTavanKatsayisi": {
        "rustName": "pekTavanKatsayisi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukAsgariUcret": {
        "rustName": "gunlukAsgariUcret",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkIsverenOraniYuzde": {
        "rustName": "sgkIsverenOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsverenOraniYuzde": {
        "rustName": "issizlikIsverenOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  },
  "PayrollPeriodInstitutionSettings": {
    "renameAll": "camelCase",
    "default": true,
    "fields": {
      "donemId": {
        "rustName": "donemId",
        "rustType": "String",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "yemekTamGvIstisnaDamgaTabi": {
        "rustName": "yemekTamGvIstisnaDamgaTabi",
        "rustType": "bool",
        "required": false,
        "optional": true,
        "nullable": false,
        "decimal": false,
        "nestedTypes": []
      },
      "tediyeListesi": {
        "rustName": "tediyeListesi",
        "rustType": "Option<Vec<TediyeKalemi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "TediyeKalemi"
        ]
      },
      "tisIkramiyeListesi": {
        "rustName": "tisIkramiyeListesi",
        "rustType": "Option<Vec<TisIkramiyeKalemi>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "TisIkramiyeKalemi"
        ]
      },
      "tediyeTisNotu": {
        "rustName": "tediyeTisNotu",
        "rustType": "Option<String>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": []
      },
      "sgkIsciOraniYuzde": {
        "rustName": "sgkIsciOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsciOraniYuzde": {
        "rustName": "issizlikIsciOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gelirVergisiOraniYuzde": {
        "rustName": "gelirVergisiOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "damgaVergisiOraniBinde": {
        "rustName": "damgaVergisiOraniBinde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sendikaAidatiYuzde": {
        "rustName": "sendikaAidatiYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitSendikaAidati": {
        "rustName": "sabitSendikaAidati",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "besOraniYuzde": {
        "rustName": "besOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sabitBesTutar": {
        "rustName": "sabitBesTutar",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiSGK": {
        "rustName": "gunlukYemekIstisnasiSGK",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukYemekIstisnasiGV": {
        "rustName": "gunlukYemekIstisnasiGV",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "statutoryParameterSegments": {
        "rustName": "statutoryParameterSegments",
        "rustType": "Option<Vec<StatutoryParameterSegment>>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "StatutoryParameterSegment"
        ]
      },
      "statutoryParameterSnapshot": {
        "rustName": "statutoryParameterSnapshot",
        "rustType": "Option<StatutoryParameterSnapshot>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": false,
        "nestedTypes": [
          "StatutoryParameterSnapshot"
        ]
      },
      "pekTavanKatsayisi": {
        "rustName": "pekTavanKatsayisi",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "gunlukAsgariUcret": {
        "rustName": "gunlukAsgariUcret",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "sgkIsverenOraniYuzde": {
        "rustName": "sgkIsverenOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      },
      "issizlikIsverenOraniYuzde": {
        "rustName": "issizlikIsverenOraniYuzde",
        "rustType": "Option<Decimal>",
        "required": false,
        "optional": true,
        "nullable": true,
        "decimal": true,
        "nestedTypes": []
      }
    }
  }
} as const;

export const RUST_DOMAIN_CONTRACT = {
  enums: RUST_ENUM_VALUES,
  structs: RUST_STRUCT_CONTRACT,
} as const;
