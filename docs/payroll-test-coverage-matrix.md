# Bordro Test Güvence Matrisi

Bu matris, kritik bordro kurallarını gerçek test/fixture kimlikleriyle
eşleştirir. `Golden` sütunundaki kimlikler
`crates/payroll-core/tests/golden/2026/` altındaki exact golden fixture'ları
(bunların 15'i bağımsız `evidence/Gxxx.md` çalışma kâğıdına bağlıdır),
`Property`/`Oracle` sütunları Faz 2'nin generated differential testlerini,
diğer sütunlar mevcut regression/parity testlerini gösterir. Faz 3 mutation
kapsamı ve coverage ratchet'ı tablonun altındaki kalite kanıtı bölümünde
izlenir.

Durumlar:

- `Kapsamlı`: Golden ve mevcut test katmanları birlikte koruyor.
- `Mevcut test`: Golden kapsamı henüz yok; mevcut regression/parity testi korunuyor.
- `Kısmi`: Golden bazı varyantları koruyor, çoklu dönem veya lifecycle kapsamı mevcut testlerde.

| Kural | Unit/Regression | Golden | Property | Oracle | Native parity | WASM/browser parity | Durum |
|---|---|---|---|---|---|---|---|
| Taban ücret / gelir toplamı | `payroll_engine_regression::calculation_uses_shared_engine_and_preserves_manual_decimal_values` | G001, G002, G004 | `payroll_properties::generated_normal_payroll_matches_independent_oracle` | `normal_payroll_oracle::calculate` | `domain_tests::test_a_save_attendance_then_calculate_same_personnel_period` | `e2e/payroll.browser.playwright.ts::browser WASM calculation persists in IndexedDB and survives reload` | Kapsamlı |
| Puantaj / 15–14 dönem | `independent_audit_20260908::audit_calendar_and_earning_formula_matrix` | G001, G014, G015, G018 | `validation_properties::period_start_outside_fifteenth_is_rejected` | — | `domain_tests::test_c_attendance_bound_to_period_id_not_date_range` | `e2e/payroll.browser.playwright.ts::normal payroll table keeps core columns and moves secondary details to the payslip` | Kapsamlı |
| Ücretli rapor | `independent_audit_20260908::audit_sick_leave_quota_dedup_and_unpaid_days` | G014 | `validation_properties::overlapping_sick_episodes_are_rejected` | — | `domain_tests::test_raporlu_gun_persistence_and_finalized_reload` | `e2e/payroll.browser.playwright.ts::browser WASM calculation persists in IndexedDB and survives reload` | Kapsamlı |
| SGK prim günü normalizasyonu | `standalone_audit_regression_test::test_audit_february_28_days_with_1_unpaid_day_results_in_27_prim_days` | G014, G015, G018 | — | — | `payroll_stress_hardening_regression_test::unpaid_r_is_not_sgk_day_but_paid_r_is` | `src/utils/payrollPresentation.test.ts` readiness kontrolleri | Kapsamlı |
| PEK alt sınırı / işveren tamamlama | `deep_financial_audit_20260909::audit_lower_bound_completion_with_earlier_same_month_event` | G003 | `pek_properties::no_carry_pek_matches_independent_oracle` | `pek_oracle::normal` | `domain_tests::test_pek_alt_sinir_tamamlama_isveren_prim_ayrimi` | `src/utils/employerPremiums.test.ts` | Kapsamlı |
| PEK üst sınırı | `independent_audit_20260908::audit_pek_ceiling_and_nonwage_carry_conservation` | G013 | `pek_properties::no_carry_pek_matches_independent_oracle` | `pek_oracle::normal` | `payroll_stress_hardening_regression_test` PEK testleri | `src/utils/employerPremiums.test.ts` | Kapsamlı |
| Devreden PEK | `payroll_engine_regression::devreden_pek_ages_once_per_tax_month_not_once_per_accrual` | — | `pek_properties::incoming_carry_is_conserved_and_never_exceeds_pek_capacity` | — | `payroll_stress_hardening_regression_test` devreden PEK testleri | `src/utils/cumulativeGv.test.ts` | Mevcut test |
| İşçi SGK %14 / işsizlik %1 | `independent_audit_20260908::audit_meal_is_exempt_from_stamp_as_well_as_income_tax` | G001, G003, G013 | `payroll_properties::generated_normal_payroll_matches_independent_oracle` | `deductions_oracle::contributions` | `domain_tests::test_isveren_sgk_primi_test_c` | WASM adapter parity tests | Kapsamlı |
| İşveren SGK / işsizlik | `domain_tests::test_toplam_isveren_primi_test_e` | G001, G003, G013 | `payroll_properties::generated_normal_payroll_matches_independent_oracle` | `deductions_oracle::contributions` | `financial_integrity_regression_test::strict_restore_reconciles_gv_damga_and_pek_snapshots` | `src/utils/employerPremiums.test.ts` | Kapsamlı |
| GV matrahı | `standalone_audit_regression_test::test_audit_sendika_aidati_deducted_from_gv_matrah` | G001, G016, G017, G028, G033 | `payroll_properties::generated_normal_payroll_matches_independent_oracle` | `normal_payroll_oracle::calculate` | `gv_matrah_regression_test` | `src/utils/cumulativeGv.test.ts` | Kapsamlı |
| Artan oranlı GV / dilim sınırları | `independent_audit_20260908::audit_progressive_tax_against_independent_piecewise_oracle` | G005–G012 | `tax_properties::progressive_total_tax_matches_independent_piecewise_oracle` | `tax_oracle::total_progressive_tax` | `domain_tests::test_gv_istisna_gercek_kumulatif_bagimsiz` | `src/utils/cumulativeGv.test.ts` | Kapsamlı |
| Kümülatif GV / açılış | `independent_audit_20260909::audit_opening_zero_nonzero_and_stale_history` | G029 | `payment_event_properties::same_month_authoritative_events_extend_tax_and_pek_state` | — | `cumulative_gv_stale_chain_regression_test` | `src/hooks/usePayrollMutationController.test.ts` | Kısmi |
| Asgari ücret GV istisnası | `independent_audit_20260909::audit_monthly_exemption_and_cumulative_event_chain` | G001, G016, G017, G030, G031 | `tax_properties::minimum_wage_gv_reference_matches_oracle_and_is_monotone` | `tax_oracle::monthly_minimum_gv_base` | `domain_tests::test_gv_istisna_ocak` | `src/utils/cumulativeGv.test.ts` | Kapsamlı |
| Yemek GV/SGK/DV istisnası | `payroll_engine_regression::normal_payroll_uses_segmented_meal_exemptions_for_sgk_gv_and_stamp_tax` | G016, G017 | `payroll_properties::generated_normal_payroll_matches_independent_oracle` | `exemptions_oracle::meal_exemption` | `domain_tests::test_yemek_istisnasi_hesabi_test_a` | `src/utils/payrollPresentation.test.ts` | Kapsamlı |
| BES/OKS | `independent_audit_20260908::audit_zero_personal_fixed_bes_means_percentage_on_every_extra_kind` | G026, G027 | — | — | `statutory_helper_authority_regression_test::zero_gross_with_used_deferred_pek_still_accrues_worker_premiums_and_oks` | `src/utils/payrollPresentation.test.ts` | Kapsamlı |
| Sendika / icra / borç / avans / diğer kesintiler | `standalone_audit_regression_test::test_audit_direct_statutory_deductions_union_fee_deduction` | G027, G028 | — | — | `financial_integrity_regression_test` strict restore testleri | Browser payroll store invariant testleri | Kapsamlı |
| TEDİYE | `payroll_engine_regression::supplementary_is_independent_and_only_prior_events_must_be_authoritative` | G023, G030 | `payment_event_properties::supplementary_event_is_deterministic_for_identical_input` | — | `multi_accrual_regression_test::payment_event_backdated_insert_delete_and_finalized_protection` | `src/utils/manualTediyeTis.test.ts` | Kapsamlı |
| TİS ikramiyesi | `payroll_engine_regression::payment_event_chain_shares_snapshots_and_preserves_full_normal_income` | G024, G031 | — | — | `manual_tediye_tis_regression_test` | `src/utils/manualTediyeTis.test.ts` | Kapsamlı |
| Supplemental payment-event | `payroll_engine_regression::supplementary_events_calculate_without_attendance_but_normal_still_fails_closed` | G025 | `payment_event_properties::supplementary_event_is_deterministic_for_identical_input` | — | `multi_accrual_regression_test` | `src/services/payrollEngine/paymentEventOrder.test.ts` | Kapsamlı |
| Aynı ay event sırası / metadata | `payroll_engine_regression::same_month_event_order_reconciles_to_the_same_final_pek_state` | G030, G031 | `payment_event_properties::same_month_authoritative_events_extend_tax_and_pek_state` | — | `multi_accrual_regression_test::native_multi_accrual_keeps_finalized_normal_immutable_and_continues_into_october` | `src/services/payrollEngine/paymentEventOrder.test.ts` | Kapsamlı |
| Retro adjustment | `retro_regression::retro_replays_historical_periods_and_keeps_original_payrolls_immutable` | — | `retro_properties::generated_retro_replay_is_deterministic_and_conservative` | — | `retro_native_regression_test` | Browser retro parity kapsamı | Mevcut test |
| Yıl geçişi | `independent_audit_20260909::audit_new_tax_year_resets_gv_but_preserves_pek_carry` | — | — | — | `cumulative_gv_stale_chain_regression_test::vergi_yili_degisiminde_onceki_yilin_stale_kaydi_yeni_yili_kirletmez` | `src/utils/cumulativeGv.test.ts` | Mevcut test |
| STALE zinciri | `payroll_engine_regression::strict_preflight_fails_closed_when_tax_chain_is_incomplete` | — | — | — | `payroll_dependency_state_regression_test` | `browserPayrollPolicies.test.ts` | Mevcut test |
| FINALIZED immutability | `payroll_engine_regression::finalized_existing_payroll_is_immutable` | — | — | — | `financial_integrity_regression_test::strict_restore_accepts_valid_payment_events_and_retro_source_deltas` | `e2e/payroll.browser.playwright.ts::browser finalization uses WASM, persists FINALIZED, and rejects a finalized mutation` | Mevcut test |
| Decimal serde / WASM sınırı | `payroll_engine_regression::decimal_json_boundary_accepts_exact_strings` | G032 | `validation_properties::decimal_json_round_trip_preserves_exact_money` | — | `independent_audit_20260909::audit_serialized_calculation_is_exact_and_repeatable` | `src/services/payrollEngine/decimalBoundary.test.ts` | Kapsamlı |

## Faz 1 kanıtı

- Golden loader: `crates/payroll-core/tests/golden_payroll_corpus.rs`
- Fixture sözleşmesi: `crates/payroll-core/tests/golden/schema.json`
- Fixture kullanım ve ekleme kuralları: `crates/payroll-core/tests/golden/README.md`
- Minimum corpus kapısı: 30 fixture; mevcut sayı: 33.
- Evidence durumu: 15 `verified`, 18 `pendingEvidence`; kritik loader bağlantı
  ve çalışma kâğıdı bölümlerini kontrol eder.

## 2026 statutory parameter kanıtı

- Bağımsız snapshot: `crates/payroll-core/tests/statutory/2026.json`.
- Karşılaştırma testi: `annual_parameters_regression::production_2026_parameters_match_independent_statutory_reference`.
- Yıllık GV dilimleri ve yıllık sigorta GV tavanı production default'tan
  generate edilmez; JSON içindeki bağımsız sabitlerle exact karşılaştırılır.
- SGK yemek istisnası alanı mevcut 4/D uygulama değeri ile resmi 4/a değeri
  arasındaki kapsam farkı nedeniyle `needs_authoritative_verification` durumunu
  açıkça taşır.

## Faz 2 kanıtı

- Property test girişi: `crates/payroll-core/tests/property_tests.rs`.
- Generated test modülleri: `crates/payroll-core/tests/property/`.
- Bağımsız oracle modülleri: `crates/payroll-core/tests/reference/`.
- Blocking CI adımı: `cargo test -p payroll-core --test property_tests`.
- Son koşu: 19 test geçti; normal bordro, GV, PEK/carry, payment-event, retro,
  validation ve Decimal sınırları temsil edildi.
- `Oracle` sütunundaki `—`, ilgili satırın karmaşık event/lifecycle davranışı
  için ayrıca bağımsız formül modeli olmadığını gösterir; mevcut regression ve
  property testleri yine korunur.

## Faz 3 kalite kanıtı

- Mutation scope: `calculations.rs`, `gv_exemption.rs`, `payroll_engine.rs`,
  `policies.rs`, `retro.rs`, `validation.rs`.
- Mutation aday listesi: **2.147**; ayrıntılı makine manifesti
  `docs/payroll-mutation-baseline.json` içindedir.
- Yerel smoke: `gv_exemption.rs` için 6/6 anlamlı mutant caught, 1 unviable,
  0 missed. Tam skor haftalık/manual 8 shard CI artifact'larından alınır.
- Canonical shard evidence yolu `target/cargo-mutants/outcomes.json`'dır.
  Eksik baseline/outcomes/shard veya parse edilemeyen JSON infrastructure
  failure'dır; missed/timeout sonucu full baseline oluşana kadar advisory'dir.
- Aggregate collector tam 8 shard bekler, duplicate mutant ve denominator/file
  scope uyuşmazlığında fail olur ve tek machine-readable summary üretir.
- Coverage baseline: `docs/payroll-coverage-baseline.json`.
- Ratchet uygulayıcısı: `scripts/check-rust-coverage.mjs`; kritik dosyaların
  lines/functions/regions metrikleri baseline'ın altına inemez. Weekly/manual
  ölçüm `PROPTEST_RNG_SEED=2026091001` ile çalışır; PR property keşfi sabitlenmez.

Golden sütunundaki
`—` işareti mevcut regression kapsamının zayıf olduğu anlamına gelmez; yalnızca
bu ilk corpus'un o davranışı henüz fixture olarak taşımadığını gösterir.
