# Golden Payroll Corpus

Bu dizin, `payroll-core` hesaplama sözleşmesinin değişmez referanslarını
barındırır. Her JSON dosyası tek bir bordro isteğini ve exact beklenen mali
sonucu taşır. Bağımsızlık iddiasının denetlenebilir kanıtı kritik fixture'larda
`evidence/Gxxx.md` çalışma kâğıdıdır.

## Sözleşme

- Dosyalar vergi yılı dizini altında tutulur: `2026/G001-*.json`.
- `schemaVersion` şu an `1`dir.
- `legalYear`, `request` içindeki aktif dönemin `taxYear` alanıyla ve kullanılan
  `AnnualPayrollParameters.year` ile aynı olmalıdır.
- `parametersVersion` değişmez bir parametre seti kimliğidir; yeni yasal yıl
  eski fixture'ların üzerine yazılarak değil, yeni yıl diziniyle eklenir.
- `expected.assertionProfile` yalnızca `FULL_FINANCIAL` olabilir.
- `expected.result`, `calculate_payroll_checked()` sonucunun serileştirilmiş
  tam snapshot'ıdır. Böylece yalnız net ödeme değil; gelir kalemleri, PEK,
  işçi/işveren primleri, GV, DV, kesintiler, carry ve payment-event metadata'sı
  exact Decimal değerlerle karşılaştırılır.
- `source.evidence` verilmişse `tests/golden/evidence/` altında repo-relative bir
  Markdown çalışma kâğıdına işaret eder. Loader bu dosyanın varlığını, fixture
  id'sini ve zorunlu hesap bölümlerini kontrol eder.
- `source.verificationStatus` yalnızca `verified` veya `pendingEvidence` olabilir;
  evidence olmayan fixture `verified` olarak işaretlenemez.

## Bağımsızlık kuralı

`expected.result` CI sırasında production motorundan üretilmez. Fixture
beklentisi commit edilmiş bir kanıttır; 33 fixture için bağımsız ve insan
tarafından okunabilir aritmetik `evidence/Gxxx.md` dosyalarında ayrıca gösterilir.
Bu kanıt dış kurum veya mevzuat onayı anlamına gelmez; yasal
parametrelerin doğruluğu ayrıca yetkili bordro/mevzuat incelemesi gerektirir.

Fixture eklerken:

1. Yeni dosyayı doğru `legalYear` dizinine ekleyin.
2. `source.referenceId`, `source.verifiedBy`, `source.verifiedAt`,
   `source.verificationStatus` ve `source.notes` alanlarını doldurun.
3. Kritik bir vaka ise `source.evidence` ile insan tarafından okunabilir
   çalışma kâğıdını bağlayın.
4. Beklenen değerleri production çıktısını kopyalayarak değil, request'ten
   bağımsız aritmetik/kurum bordrosu/reference çalışma kâğıdıyla doğrulayın.
5. `cargo test -p payroll-core --test golden_payroll_corpus` komutunu çalıştırın.

Test loader'ı gerçek `schema.json` kısıtlarını (required, enum, pattern,
additionalProperties ve minLength dahil) uygular; ayrıca dosyaları alfabetik
sırada toplar, duplicate fixture id'lerini, mevzuat yılı/sürüm ayrışmasını ve
zorunlu mali snapshot alanlarını fail-closed olarak kontrol eder. Corpus sayısı
30'un altına düşerse test başarısız olur.

## Mevcut corpus kapsamı

2026 corpus'unda 33 fixture bulunur. Normal ücret, GV dilim sınırları, PEK alt
ve üst sınırı, ücretli/ücretsiz rapor, yemek istisnası, karma puantaj, iş primi,
yardım ve hizmet yılı kalemleri, bağımsız TEDİYE/TİS/ek ödeme event'leri, OKS,
sendika, personel kesintileri, GV açılışı, aynı ay event zinciri ve Decimal
kuruş sınırları temsil edilir.

Bu corpus, mevcut regression/parity testlerinin yerine geçmez. Yıl geçişi,
devreden PEK'in çok aylı ömrü, retro ve FINALIZED/STALE lifecycle davranışları
mevcut testlerde korunmaya devam eder; bunların golden kapsamı sonraki güvence
fazlarında genişletilecektir. Loader tüm 33 fixture için `verified` evidence
bağlantısını fail-closed biçimde doğrular.
