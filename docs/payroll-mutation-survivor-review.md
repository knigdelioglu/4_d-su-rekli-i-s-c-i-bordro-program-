# Mutation Survivor İncelemesi

Bu belge, ikinci ve final kabul edilen full mutation koşusundaki 494
anlamlı missed mutant için conservative triage kaydıdır. Bu sınıflandırma
survivor'ları suppression/exclusion listesine almaz; doğrulanmamış eşdeğerlik
ve ulaşılamazlık iddiaları bilinçli olarak NEEDS_REVIEW altında tutulur.

## Ölçüm kimliği

- CI full workflow: 35013775841
- Ölçülen production/test commit'i: 34198be740843e8ae22ae459e1a4263e7618cf82
- Mutation summary job: 104540908933
- Araç: cargo-mutants 27.1.0
- Shard seti: tam olarak 0, 1, 2, 3, 4, 5, 6, 7
- Kaynak: payroll-core-mutation-summary artifact'ı ve sekiz canonical
  target/cargo-mutants/outcomes.json dosyası

| Sayaç | Değer |
|---|---:|
| Total | 2.147 |
| Caught | 1.349 |
| Missed | 494 |
| Timeout | 0 |
| Unviable | 304 |
| Meaningful | 1.843 |
| Score | %73,20 |

Meaningful değeri 2.147 - 304, skor ise 1.349 / 1.843 olarak hesaplanmıştır.
Unviable mutant'lar survivor değildir ve denominator'dan çıkarılmış olsa da
çıktıda ayrı tutulmuştur.

## Sınıflandırma özeti

| Kategori | Sayı | Uygulama |
|---|---:|---|
| REAL_TEST_GAP | 0 | Bu koşuda production davranışının yanlış olduğu halde testin geçtiği doğrulanmış vaka yok. Bu, gelecekte gap bulunamayacağı iddiası değildir. |
| EQUIVALENT | 2 | Yalnız aşağıda açıklanan, mevcut public calculation path'te gözlenmeyen iki arithmetic mutant. |
| UNREACHABLE | 0 | Public üretim akışından erişilemediği kanıtlanmış survivor yok. |
| NEEDS_REVIEW | 492 | Davranışsal etkisi veya public precondition'ı bu turda kesinleştirilmeyen survivor'lar. |

### EQUIVALENT olarak doğrulanan iki mutant

retro.rs içindeki previous_authoritative_retro_by_period_and_code fonksiyonu
tarafından döndürülen previous_pek değeri mevcut
RetroEntitlementEngine::calculate çağrısında _previous_pek olarak
destructure edilir ve kullanılmaz. Bu nedenle aynı satırdaki previous_pek
toplamına yönelik + -> - ve + -> * mutantları mevcut public sonuçlarda
gözlenebilir fark oluşturmamaktadır. Bu karar cargo-mutants exclusion'ı
değildir; yalnızca inceleme kaydıdır.

## Dosya bazında triage

| Dosya | Missed | REAL_TEST_GAP | EQUIVALENT | UNREACHABLE | NEEDS_REVIEW |
|---|---:|---:|---:|---:|---:|
| calculations.rs | 81 | 0 | 0 | 0 | 81 |
| payroll_engine.rs | 241 | 0 | 0 | 0 | 241 |
| policies.rs | 5 | 0 | 0 | 0 | 5 |
| retro.rs | 141 | 0 | 2 | 0 | 139 |
| validation.rs | 26 | 0 | 0 | 0 | 26 |
| gv_exemption.rs | 0 | 0 | 0 | 0 | 0 |
| **Toplam** | **494** | **0** | **2** | **0** | **492** |

## Yeni regression testlerinin kanıtı

İlk full koşuya göre kalan missed sayısı 525'ten 494'e indi; 31 mutant
caught oldu ve timeout/unviable sayıları değişmedi. En yüksek getirili
kümelerdeki değişim:

| Küme | İlk yeni koşu | Final koşu | Değişim |
|---|---:|---:|---:|
| policies.rs toplam missed | 16 | 5 | -11 |
| retro.rs toplam missed | 161 | 141 | -20 |
| retro.rs validate_batch_ledger | 28 | 23 | -5 |
| retro.rs apply_source_month_sgk | 12 | 2 | -10 |
| retro.rs RetroEntitlementEngine::calculate | 11 | 10 | -1 |
| retro.rs previous_source_retro_state | 7 | 3 | -4 |
| policies.rs affected_by_mutation | 8 | 3 | -5 |
| policies.rs retro_batch_affected_by_mutation | 6 | 1 | -5 |
| policies.rs retro_batch_requires_source_carry_replay | 1 | 0 | -1 |

Bu azalmanın başlıca regression kapsamı chained authoritative retro delta,
source/payment month ayrımı, source-month SGK, allocation ledger sınırları,
negative settlement ve policy scope/carry replay davranışlarıdır.

## Sonraki inceleme sınırı

Kalan NEEDS_REVIEW kümeleri özellikle payroll_engine.rs para akışı,
retro.rs ledger/settlement ve hesaplama/validation sınırlarında tutuldu.
Sırf mutation skorunu yükseltmek için private fonksiyon public yapılmadı,
dead branch eklenmedi, test skip edilmedi ve production hesaplama algoritması
değiştirilmedi. Gerçek bir davranış gap'i doğrulanırsa önce failing regression
testi, ardından yalnızca gerekli minimal production fix'i eklenecektir.

## Payroll Engine Quality Round 2

Bu tur yalnızca `crates/payroll-core/src/payroll_engine.rs` için, mevcut
full baseline survivor'larının davranışsal incelemesine ve gerçek test açığı
olan public payroll yollarına odaklandı. Production bordro hesaplama kodu
değiştirilmedi.

### Başlangıç ve ölçüm kimliği

- Başlangıç payroll-engine baseline: **781 total, 408 caught, 241 missed,
  0 timeout, 132 unviable, 649 meaningful, %62,87**.
- Ölçülen kod commit'i: `c9d0e9b838fbf2c6ae1a9fc1f1b07eaba11ea906`.
- Normal CI / verify run: `35439156749` — PASS.
- Full 8-shard run: `35439833175` — PASS; summary job
  `105891611784`.
- Araç: cargo-mutants `27.1.0`; full run shard kümesi `0..7`.

### Başlangıç survivor kümeleri ve risk sınıfı

Aşağıdaki sayılar baseline `missedMutants` listesinden gerçek olarak
çıkarılmıştır; tahmini değildir.

| Fonksiyon | Başlangıç missed | Round 2 full missed | Risk sınıfı |
|---|---:|---:|---|
| `calculate_payroll_with_index` | 39 | 23 | CRITICAL_FINANCIAL |
| `validate_devreden_pek_gap` | 24 | 21 | VALIDATION_BOUNDARY |
| `previous_insurance_gv` | 24 | 12 | CRITICAL_FINANCIAL |
| `reconcile_same_month_pek` | 17 | 3 | CRITICAL_FINANCIAL |
| `previous_gv` | 16 | 13 | CRITICAL_FINANCIAL |
| `validate_statutory_segments_for_period` | 14 | 0 | VALIDATION_BOUNDARY |
| `merge_is_primi_details` | 12 | 1 | CLASSIFICATION |
| `hakedis_gun` | 10 | 0 | DATE_PERIOD_LOGIC |
| `resolve_accrual_input` | 8 | 0 | STRUCTURAL |
| `retro_source_carry_overrides_for_event` | 8 | 3 | STRUCTURAL |
| `validate_retro_source_carry` | 7 | 0 | VALIDATION_BOUNDARY |
| `incoming_devreden_pek` | 7 | 7 | CRITICAL_FINANCIAL |
| `validate_period` | 6 | 2 | VALIDATION_BOUNDARY |
| `validate_tax_chronology` | 5 | 5 | VALIDATION_BOUNDARY |
| `find_previous_work_period` | 5 | 5 | DATE_PERIOD_LOGIC |
| `same_month_stamp_exemption_used` | 4 | 0 | CRITICAL_FINANCIAL |
| `validate_tax_month_overlap` | 2 | 0 | VALIDATION_BOUNDARY |
| `validate_statutory_tax_month_reference` | 3 | 3 | VALIDATION_BOUNDARY |
| `incoming_from_retro_source_carry` | 3 | 1 | CRITICAL_FINANCIAL |
| `carry_override_for_source_period` | 2 | 2 | STRUCTURAL |
| `days_in_month` | 2 | 2 | DATE_PERIOD_LOGIC |
| `find_zam_tarihi` | 2 | 2 | DATE_PERIOD_LOGIC |
| `incoming_devreden_pek_for_replay` | 2 | 2 | CRITICAL_FINANCIAL |
| `retro_event_contains_carry_override` | 2 | 2 | STRUCTURAL |
| `tax_month_distance` | 2 | 0 | DATE_PERIOD_LOGIC |
| `validate_payroll_finalization_request_with_index` | 2 | 2 | VALIDATION_BOUNDARY |
| `add_paid_sick_wage` | 1 | 0 | CRITICAL_FINANCIAL |
| `attendance_has_full_calendar_coverage` | 1 | 1 | VALIDATION_BOUNDARY |
| `attendance_missing_calendar_days` | 1 | 0 | VALIDATION_BOUNDARY |
| `calculate_normal_meal_exemptions` | 1 | 1 | CRITICAL_FINANCIAL |
| `calculate_paid_sick_dates_from_records` | 1 | 1 | DATE_PERIOD_LOGIC |
| `canonical_event_pek_components` | 1 | 0 | CRITICAL_FINANCIAL |
| `full_period_sgk_day_weight` | 1 | 1 | CRITICAL_FINANCIAL |
| `is_prim_bearing_code` | 1 | 0 | CLASSIFICATION |
| `ordered_prior_payment_events` | 1 | 0 | STRUCTURAL |
| `resolve_legacy_effective_tax_month` | 1 | 1 | VALIDATION_BOUNDARY |
| `same_month_pek_used` | 1 | 0 | CRITICAL_FINANCIAL |
| `validate_payroll_request_with_index` | 1 | 0 | VALIDATION_BOUNDARY |
| `validate_prior_accruals_finalized` | 1 | 0 | VALIDATION_BOUNDARY |

Risk sınıfı mutation operatorünün adına göre otomatik verilmedi; fonksiyonun
public payroll sonucuna veya fail-closed validation davranışına etkisi
incelenerek verildi. Özellikle `+/-/*`, karşılaştırma ve boolean operatorleri
CRITICAL_FINANCIAL veya VALIDATION_BOUNDARY kümelerinde önceliklendirildi.

### Eklenen testler ve hedefleri

- Gün/hakediş ve SGK gün ağırlığı: 28/29 Şubat, artık yıl, 30/31 günlük ay,
  tam/eksik gün, ücretsiz izin, rapor, GC/GÇT, 15–14 dönem, ay/yıl geçişi
  ve bağımsız exact Decimal beklentileri.
- `skipped_tax_month_expires_a_two_month_pek_carry_exactly`: tax month
  distance ve iki aylık carry sonlanma sınırını public payroll API üzerinden
  doğrular.
- Period/finalization sınırları: eşit başlangıç-bitiş, ters dönem, tax year/
  month uyuşmazlığı, komşu ay/yıl geçişi, segment boundary/overlap/gap ve
  finalized olmayan prior event ile eksik attendance reddi.
- Accrual ve retro carry sınırları: negatif/colliding input, matching tax
  month/payment date, yanlış tax component, source-period carry ve
  authoritative aynı-ay PEK state.
- Aynı-ay PEK ve prim-bearing davranışı: exact wage/carry önceliği, attendance
  kapasite sınırı, normal olayın provisional'a yükseltilmemesi, legacy
  snapshot reconstruct davranışı ve SGK prim sınıflandırması.
- Insurance GV public yolu: önceki tax month kullanımı, aynı ay/future ay ve
  önceki yıl kayıtlarının doğru dışlanması.

Testler public calculation/validation API'lerini kullandı; private fonksiyon
mutation'ı öldürmek için public yapılmadı. Expected değerler Decimal ve
bağımsız basit aritmetik ile üretildi; production calculator expected oracle
olarak kullanılmadı.

### Targeted mutation sonucu

Targeted komut payroll-engine source scope'u, cargo-mutants `27.1.0`, mevcut
test komutu, timeout policy ve `--no-shuffle` semantiği korunarak çalıştırıldı.

| Sayaç | Başlangıç | Targeted Round 2 |
|---|---:|---:|
| Total | 781 | 781 |
| Caught | 408 | 526 |
| Missed | 241 | 123 |
| Timeout | 0 | 0 |
| Unviable | 132 | 132 |
| Meaningful | 649 | 649 |
| Score | %62,87 | %81,05 |

Targeted koşuda 118 survivor azaldı. Full CI aggregate aynı ölçülen commit
üzerinde 7 function-return mutantını da yakaladığı için baseline kaydı için
source-of-truth olan full sonuç kullanıldı; targeted ve full scope/runner
execution path arasındaki bu 7 mutant farkı ayrıca bırakılmadı.

### Full mutation sonucu ve final triage

Full summary sonucu:

| Sayaç | Round 2 full |
|---|---:|
| Total | 2.147 |
| Caught | 1.483 |
| Missed | 360 |
| Timeout | 0 |
| Unviable | 304 |
| Meaningful | 1.843 |
| Score | %80,47 |

Payroll-engine final aggregate:

| Sayaç | Round 2 full |
|---|---:|
| Total | 781 |
| Caught | 533 |
| Missed | 116 |
| Timeout | 0 |
| Unviable | 132 |
| Meaningful | 649 |
| Score | %82,13 |

Payroll-engine missed `241 -> 116`; full aggregate'e göre 125 mutant caught
oldu. Kalan 116 survivor'ın hiçbirisi bu turda doğrulanmış
`REAL_TEST_GAP` olarak işaretlenmedi. `EQUIVALENT` veya `UNREACHABLE` kararı
kanıtlanmadan verilmedi; kalanların tamamı conservative `NEEDS_REVIEW` olarak
korundu. Özellikle `previous_gv` ve `previous_insurance_gv` içindeki
DRAFT/STALE ikinci-loop survivor'ları public fail-closed precondition nedeniyle
erişilebilirlik adayıdır, ancak bu turda kesin UNREACHABLE sayılmadı.

| Final sınıflandırma | Sayı |
|---|---:|
| REAL_TEST_GAP | 0 |
| EQUIVALENT | 0 |
| UNREACHABLE | 0 |
| NEEDS_REVIEW | 116 |

Kalan yüksek yoğunluklu kümeler: `calculate_payroll_with_index` 23,
`validate_devreden_pek_gap` 21, `previous_gv` 13,
`previous_insurance_gv` 12, `incoming_devreden_pek` 7,
`validate_tax_chronology` 5 ve `find_previous_work_period` 5. Bu kümeler
financially meaningful adaylar olarak bir sonraki review turuna bırakıldı.

**No production payroll bug was found in this round.** Production dosyalarında
değişiklik yoktur; yalnız regression/property/table-driven test paketi
eklenmiştir. Mutation exclusion, blanket ignore, timeout artırımı veya test
skip uygulanmadı.
