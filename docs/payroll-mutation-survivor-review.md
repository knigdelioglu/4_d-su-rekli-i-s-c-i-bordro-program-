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
