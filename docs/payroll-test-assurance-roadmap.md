# Bordro Test Güvence Yol Haritası

**Durum:** Faz 3 altyapısı teslim edildi; coverage ratchet aktif, mutation full-run periyodik/manual akışta ve henüz pending — 2026-09-10

**Hedef dal:** `main`
**Kapsam:** `payroll-core`, native Tauri/SQLite akışı, WASM/browser parity ve CI test güvence katmanları
**İlişkili plan:** `docs/payroll-engine-hardening-plan.md`

---

## 1. Amaç

Bu planın amacı mevcut bordro hesaplama motorunu yeniden yazmak değildir. Amaç, halihazırda güçlü olan regresyon ve invariant testlerini daha yüksek güven seviyesine çıkarmaktır.

Hedeflenen güvence modeli:

1. **Golden Payroll Corpus** — bağımsız doğrulanmış gerçekçi bordrolar değişmez referans olur.
2. **Property-based testing** — elle düşünülmeyen binlerce geçerli/geçersiz senaryo otomatik üretilir.
3. **Independent reference oracle** — kritik mali formüller production kodundan bağımsız ikinci matematiksel modelle karşılaştırılır.
4. **Mutation testing** — testlerin gerçekten yanlış kodu öldürüp öldürmediği ölçülür.
5. **Coverage + CI quality gates** — kapsam ölçülür ve geriye gitmesi engellenir.
6. **Release evidence** — kritik bordro davranışlarının hangi testlerle korunduğu izlenebilir hale getirilir.

Bu plan tamamlandığında hedef yalnızca "çok testimiz var" demek değil, aşağıdaki sorulara kanıtla cevap verebilmektir:

- Bilinen doğru bordrolar hâlâ birebir aynı mı?
- Vergi/PEK/istisna sınırlarında beklenmeyen kombinasyonları yakalıyor muyuz?
- Production formülünde küçük bir hata yapılırsa test gerçekten fail oluyor mu?
- Native, WASM ve browser akışları aynı authoritative sonucu koruyor mu?
- Yeni bir değişiklik test güvence seviyesini düşürüyor mu?

---

## 2. Mevcut başlangıç seviyesi

Depoda zaten korunacak güçlü katmanlar vardır:

- `crates/payroll-core/tests/payroll_engine_regression.rs`
- `crates/payroll-core/tests/retro_regression.rs`
- `crates/payroll-core/tests/independent_audit_*.rs`
- `crates/payroll-core/tests/deep_financial_audit_*.rs`
- `src-tauri/tests/*_regression_test.rs`
- `src-tauri/tests/independent_audit_*.rs`
- `src/services/payrollEngine/decimalBoundary.test.ts`
- browser storage / policy testleri
- Playwright E2E
- Rust/WASM/browser build ve parity kontrolleri
- `.hermes/INVARIANTS.md`
- `ResolvedStatutorySnapshot`, GV/PEK/DV hesap detayları ve FINALIZED koruması

Dolayısıyla bu yol haritası mevcut regresyon testlerini topluca yeniden yazmayacaktır. Yeni katmanlar mevcut test takımının üzerine eklenecektir.

## Uygulama durumu

Üç büyük uygulama fazı yaklaşımında Faz 1 ve Faz 2 tamamlandı. Test güvence
matrisi, 2026 yılı için 33 exact golden fixture (15'i bağımsız evidence çalışma
kâğıdına bağlı), fixture şeması, exact
`calculate_payroll_checked()` replay testi ve `legalYear`/parametre sürümü
fail-closed kontrollerine ek olarak property-based testler ve test-only bağımsız
mali oracle eklendi. Ayrıntılı kapsam
`docs/payroll-test-coverage-matrix.md`, fixture sözleşmesi
`crates/payroll-core/tests/golden/README.md` ve Faz 2 test topolojisi
`crates/payroll-core/tests/property_tests.rs` altında görülebilir.

Bu teslimat, aşağıdaki özgün başlıkları birlikte karşılar:

- Faz 1 — Test envanteri ve güvence matrisi
- Faz 1 — Golden Payroll Corpus
- Faz 1 — Golden/mevzuat sürüm disiplini
- Faz 2 — Property-based testing
- Faz 2 — Independent Reference Oracle

Uygulama sırası üç büyük faz olarak izlenir:

1. **Faz 1 — Referans temeli:** güvence matrisi, golden corpus ve yasal yıl/sürüm disiplini. **Tamamlandı.**
2. **Faz 2 — Bağımsız doğruluk:** property-based testler ve production'dan bağımsız mali oracle. **Tamamlandı.**
3. **Faz 3 — Sürekli güvence:** mutation, coverage/CI quality gate ve release evidence.
   **Altyapı tamamlandı; mutation infrastructure blocking, quality findings
   advisory ve tam skor periyodik/manual shard koşusundan sonra baseline'a
   işlenecektir.**

---

# 3. Faz 1 — Test envanteri ve güvence matrisi

## Amaç

Her kritik bordro kuralının hangi test katmanlarıyla korunduğunu görünür hale getirmek.

## Eklenecek dosya

`docs/payroll-test-coverage-matrix.md`

## Matris başlıkları

Her kural için en az şu kolonlar bulunmalıdır:

| Kural | Unit/Regression | Golden | Property | Oracle | Native parity | WASM parity | Mutation | Durum |
|---|---|---|---|---|---|---|---|---|

İlk kapsam:

- taban ücret
- puantaj / 15–14 dönem
- ücretli rapor
- SGK prim günü
- PEK alt/üst sınırı
- devreden PEK
- işçi SGK %14
- işçi işsizlik %1
- işveren SGK
- işveren işsizlik
- GV matrahı
- artan oranlı GV
- kümülatif GV
- asgari ücret GV istisnası
- yemek GV istisnası
- damga vergisi / istisnası
- BES/OKS
- sendika/icra/borç/avans kesintileri
- TEDİYE
- TİS ikramiyesi
- supplemental payment event
- retro adjustment
- aynı ay çoklu payment-event sırası
- yıl geçişi
- STALE zinciri
- FINALIZED immutability
- Decimal serialization

## Kabul kriteri

Hiçbir kritik hesap kuralı yalnız dosya adı veya yorum üzerinden "test edilmiş" sayılmamalıdır. Matriste gerçek test adı veya fixture kimliği gösterilmelidir.

---

# 4. Faz 1 — Golden Payroll Corpus

## Amaç

Production motorundan bağımsız olarak doğrulanmış bordroları kalıcı referans haline getirmek.

## Dizin yapısı

```text
crates/payroll-core/tests/golden/
  README.md
  schema.json
  2026/
    G001-normal-full-month.json
    G002-minimum-wage-boundary.json
    G003-pek-floor-completion.json
    G004-mid-range-normal.json
    G005-tax-first-bracket-upper.json
    G006-tax-boundary-below-190000.json
    G007-tax-boundary-above-190000.json
    G008-tax-second-bracket.json
    G009-tax-boundary-below-400000.json
    G010-tax-boundary-above-400000.json
    G011-tax-third-bracket.json
    G012-tax-boundary-above-1500000.json
    G013-pek-ceiling.json
    G014-paid-sick-leave.json
    G015-unpaid-absence.json
    G016-meal-under-exemption.json
    G017-meal-over-exemption.json
    G018-mixed-attendance.json
    G019-work-premium.json
    G020-social-aid.json
    G021-road-and-clothing.json
    G022-service-year-increment.json
    G023-standalone-tediye.json
    G024-standalone-tis.json
    G025-standalone-supplemental.json
    G026-oks-percentage.json
    G027-oks-fixed-and-deductions.json
    G028-union-fixed.json
    G029-tax-opening.json
    G030-tediye-before-normal.json
    G031-normal-before-tis.json
    G032-decimal-boundary.json
    G033-gv-deductions.json
```

İlk hedef **en az 30 golden vaka**, olgun hedef **50+ vaka**dır.

## Golden fixture sözleşmesi

Her fixture şu bölümleri taşımalıdır:

```json
{
  "schemaVersion": 1,
  "id": "G001",
  "description": "...",
  "legalYear": 2026,
  "parametersVersion": "2026-repository-parameters-v1",
  "source": {
    "type": "independent_manual_calculation",
    "verifiedBy": "...",
    "verifiedAt": "YYYY-MM-DD",
    "notes": "..."
  },
  "request": {},
  "expected": {
    "assertionProfile": "FULL_FINANCIAL",
    "result": {}
  }
}
```

## Kritik kural

Golden `expected` sonucu **production `calculate_payroll_checked()` çağrısından otomatik üretilmemelidir**.

Production motorunun sonucunu fixture'a yazan generator yalnız taşıma/format aracı olabilir; doğrulama kaynağı olamaz.

Golden değerler şu kaynaklardan biriyle doğrulanmalıdır:

1. elle hazırlanmış bağımsız kuruş hesabı,
2. mevzuata göre ayrı hazırlanmış reference calculator,
3. güvenilir kurum bordrosu / doğrulanmış gerçek bordro örneği,
4. iki bağımsız yöntemin aynı sonucu vermesi.

Kişisel gerçek bordrolar kullanılacaksa fixture'lar anonimleştirilmelidir.

## Assertion seviyeleri

`FULL_FINANCIAL` fixture'larında yalnız `netOdeme` kontrol edilmez. En az:

- gelir kalemleri
- gelir toplamı
- PEK
- işçi primleri
- işveren primleri
- GV matrahı
- önceki/yeni kümülatif GV
- brüt GV
- uygulanan GV istisnası
- kesilen GV
- DV matrahı
- DV istisnası
- kesintiler
- kesinti toplamı
- net ödeme
- devreden PEK
- payment-event metadata

birebir doğrulanır.

## Uygulama

Yeni test:

`crates/payroll-core/tests/golden_payroll_corpus.rs`

Test tüm fixture'ları yükler, `calculate_payroll_checked()` çalıştırır ve beklenen mali alanları exact `Decimal` olarak karşılaştırır.

## Kabul kriteri

- En az 30 exact golden fixture. **Tamamlandı: 33 fixture; 15 kritik fixture
  repo içi bağımsız evidence çalışma kâğıdına bağlı, 18 fixture pendingEvidence.**
- Production kodundan otomatik expected üretimi yok.
- Tüm golden fixture'lar CI'da çalışıyor. **Tamamlandı: ayrı blocking CI adımı eklendi.**
- Fixture değişikliği kod değişikliğinden ayrı ve incelemeye açık diff üretir.

---

# 5. Faz 2 — Property-based testing

## Amaç

Elle yazılmış örneklerin kapsamadığı kombinasyonları otomatik üretmek ve invariant ihlallerini bulmak.

## Teknoloji

Rust tarafında `proptest` yalnız `dev-dependencies` altında kullanılmalıdır.

Önerilen:

```toml
[dev-dependencies]
proptest = "1"
```

Production binary bağımlılığı yapılmamalıdır.

## Yeni test dosyaları

```text
crates/payroll-core/tests/property/
  mod.rs
  payroll_properties.rs
  tax_properties.rs
  pek_properties.rs
  payment_event_properties.rs
  retro_properties.rs
  validation_properties.rs
crates/payroll-core/tests/reference/
  mod.rs
  tax_oracle.rs
  pek_oracle.rs
  exemptions_oracle.rs
  deductions_oracle.rs
  normal_payroll_oracle.rs
crates/payroll-core/tests/property_tests.rs
```

Cargo integration-test yapısına göre gerekirse tek üst seviye `property_tests.rs` modülü altında organize edilebilir.

## İlk zorunlu property seti

### Finansal korunum

- `netOdeme == gelirToplam - kesintiToplam`
- kesintiler geçerli normal senaryoda negatif olamaz
- `finalPek <= pekUstSinir`
- PEK kapasitesini aşan uygun ücret dışı kazanç yalnız mevzuat izin veriyorsa devreden kayda dönüşür
- kullanılan + kalan devreden PEK tutarı kaynak tutarla tutarlı olmalıdır

### Vergi

- cari GV matrahı negatif olamaz
- aynı parametrelerle gelir artarken brüt GV düşemez
- uygulanan asgari GV istisnası kalan aylık hakkı aşamaz
- kesilen GV negatif olamaz
- yeni vergi yılında önceki yıl GV kümülatifi taşınamaz

### Payment-event

- aynı event sırası ve aynı dataset deterministik sonuç üretir
- aynı ay önceki authoritative event'ler sıralamaya göre kümülatif state'e katılır
- STALE dependency sessizce atlanamaz
- event kimliği/sequence metadata'sı yeniden hesaplamada değişmez

### Puantaj / rapor

- dönem dışı tarih kabul edilmez
- geçersiz tarih kabul edilmez
- rapor overlap invariantı korunur
- tam katılımda SGK gün normalizasyonu sözleşmeye uygun kalır

### Decimal

- serde round-trip mali Decimal değerlerini değiştirmez
- WASM sınırı için geçerli Decimal string'leri scientific notation'a dönüşmez

## Generator stratejisi

Generator'lar doğrudan tamamen rastgele JSON üretmemelidir. Önce geçerli domain objeleri üreten composable strategy'ler kurulmalıdır:

- valid period
- valid attendance
- valid annual parameters
- valid personnel
- valid deduction set
- valid payment-event chain
- valid retro revision

Ayrıca ayrı invalid strategy'ler validation fail-closed davranışını test etmelidir.

## Reproducibility

CI failure çıktısında `proptest` minimal failing case ve seed/case bilgisi korunmalıdır. Bu
suite `failure_persistence = None` ile koşuyu kaynak ağacına yerel regresyon dosyası
yazmadan ve sabit case bütçeleriyle yürütür; başarısızlık çıktısındaki küçültülmüş vaka
CI log'unda görünür. Aynı girdiyi yeniden çalıştırmak için proptest'in raporladığı
seed/case bilgisi kullanılabilir. Bulunan her production bug fixinden sonra
küçültülmüş örnek ayrıca normal regresyon testine dönüştürülmelidir.

## Kabul kriteri

- Kritik alanların tamamında property testleri. **Tamamlandı: vergi, PEK/carry,
  normal finansal korunum, payment-event, retro ve validation/Decimal setleri.**
- Her CI koşusunda replay edilebilir failure bilgisi. **Tamamlandı:
  `property_tests` blocking test adımı, persistence kapalı sabit case bütçeleri ve
  shrink çıktısı.**
- Property testinin bulduğu her gerçek hata için kalıcı regression testi. **Faz 2
  koşusunda üretim hatası bulunmadı; yakalanan kapasite/uygulanan istisna ayrımı
  oracle modeline kalıcı test beklentisi olarak işlendi.**

---

# 6. Faz 2 — Independent Reference Oracle

## Amaç

Production bordro motorunun kendi hesabını yine kendisiyle doğrulama riskini azaltmak.

## Mimari kural

Oracle production `payroll_engine`, `calculations`, `policies` helper'larını kullanmamalıdır.

Oracle test-only kod olmalıdır:

```text
crates/payroll-core/tests/reference/
  tax_oracle.rs
  pek_oracle.rs
  exemptions_oracle.rs
  deductions_oracle.rs
  normal_payroll_oracle.rs
```

## Yaklaşım

Production kodundaki optimizasyonları veya fonksiyon yapısını kopyalamak yerine daha basit, okunabilir ve doğrudan mevzuat formülünü ifade eden algoritmalar yazılmalıdır.

Örnek:

- artan oranlı GV: dilim aralıklarını tek tek kesişim hesabıyla çöz
- PEK: taban/tavan ve carry kapasitesini doğrudan matematiksel parçalarla hesapla
- aylık asgari ücret GV hakkı: ayrı referans fonksiyon
- DV: matrah × oran − uygulanabilir istisna

## Differential katmanları

### A. Formula differential

`production_formula(input) == independent_oracle(input)`

Bu karşılaştırma property-generated yüzlerce/binlerce girdi üzerinde çalıştırılır.

### B. Full normal payroll differential

İlk etapta normal bordronun kritik mali alanları oracle tarafından bağımsız hesaplanır:

- brüt gelir
- PEK
- SGK/işsizlik
- GV matrahı
- GV
- GV istisnası
- DV
- kesinti toplamı
- net

Retro/payment-event gibi karmaşık akışlar ikinci aşamada eklenir.

### C. Adapter parity

Mevcut native/WASM/browser parity testleri korunur:

```text
reference oracle
       ↓
payroll-core
   ↙       ↘
native    WASM/browser
```

Native ↔ core veya WASM ↔ core karşılaştırması tek başına bağımsız oracle sayılmayacaktır; ancak adapter parity güvencesi olarak devam edecektir.

## Kabul kriteri

- GV, PEK, DV ve temel normal bordro için bağımsız oracle. **Tamamlandı.**
- Oracle production helper import etmez. **Tamamlandı: reference modülleri
  yalnız test binary'si altında derlenir ve production hesap helper'larını
  çağırmaz.**
- Property-generated girdiler üzerinde differential test çalışır. **Tamamlandı:
  normal bordro ile GV/PEK formülleri property suite içinde karşılaştırılır.**
- Bir production bug'ı oracle'daki aynı kod kopyası nedeniyle gizlenmemelidir.
  **Tamamlandı: oracle daha basit parça/dilim hesapları ve ayrı test-only
  yuvarlama fonksiyonları kullanır.**

---

# 7. Faz 3 — Mutation Testing

## Amaç

Test sayısını değil, yanlış implementasyonu yakalama gücünü ölçmek.

## Teknoloji

Rust için `cargo-mutants` kullanılacaktır.

İlk kapsam tüm workspace değil, kritik hesap modülleridir:

```text
crates/payroll-core/src/calculations.rs
crates/payroll-core/src/gv_exemption.rs
crates/payroll-core/src/payroll_engine.rs
crates/payroll-core/src/policies.rs
crates/payroll-core/src/retro.rs
crates/payroll-core/src/validation.rs
```

### Uygulama durumu

`.cargo/mutants.toml` bu altı dosyayı ve yalnız `payroll-core` test paketini
ölçecek şekilde sabitler. Lokal aday listesi **2.147 mutant** üretmiştir;
`gv_exemption.rs` smoke koşusunda 7 mutantın 6'sı caught, 1'i unviable ve
0'ı missed olmuştur. Tam kapsam, her haftalık/manual çalışmada 8 shard halinde
`target/cargo-mutants/outcomes.json` artifact'ı olarak saklanır. cargo-mutants
27.1.0'un tool-native `target/mutants.out` çıktısı CI'da tek seferde canonical
dizine taşınır; collector nested/fallback path aramaz. Unviable sonuçlar
sessiz exclusion değildir; sonuç sınıflandırmasında ayrı tutulur.

## Uygulama sırası

1. Lokal mutation baseline çıkar.
2. Surviving mutant'lar sınıflandırılır.
3. Gerçek test açığı olan her mutant için test eklenir.
4. Equivalent/unreachable mutant'lar ancak ayrı kanıt ve inceleme ile
   belgelenir; blanket survivor exclusion kullanılmaz.
5. CI'ya infrastructure failure'ı blocking, missed/timeout kalite sonucunu
   advisory olarak raporlayan job olarak girer.
6. Baseline oturduktan sonra mutation score gerilemesi blocking hale gelir.

## Hedef

İlk koşuda keyfi bir yüzdeyi zorlamak yerine baseline ölçülür.

Sonraki kalite kapısı:

- kritik mali modüllerde mutation score gerileyemez,
- yeni/değiştirilen kritik kodda surviving mutant gerekçesiz bırakılamaz,
- uzun vadeli hedef kritik mali modüllerde **%90+ öldürülen anlamlı mutant**tır.

Mutation testi her normal push'ta çalışmak zorunda değildir. Maliyet nedeniyle:

- PR etiketi/manual dispatch,
- nightly/weekly,
- release öncesi

akışlarından biri kullanılabilir.

## Kabul kriteri

"Test geçti" durumunun yanında "kritik formülde yapılan sentetik yanlışlık test tarafından yakalandı" kanıtı bulunur.

---

# 8. Faz 3 — Coverage ve CI quality gates

## Amaç

Test kapsamının görünür olması ve fark edilmeden gerilememesi.

### Uygulama durumu

`docs/payroll-coverage-baseline.json` altı kritik modül için lines/functions/
regions baseline'ını taşır. `scripts/check-rust-coverage.mjs` her metriği dosya
bazında karşılaştırır ve düşüşte fail olur. Bu kontrol haftalık/manual CI
çalışmasında blocking'dir; ilk baseline'da toplam repository için keyfi bir
minimum yüzde yoktur. Coverage job ölçümünde sabit `PROPTEST_RNG_SEED=2026091001`
kullanır; normal PR property suite keşif için seed'siz kalır.

## Rust coverage

`cargo-llvm-cov` ile baseline alınır.

İlk çalışmada minimum yüzde dayatılmaz. Önce şu modüller ayrı raporlanır:

- calculations
- payroll_engine
- gv_exemption
- policies
- retro
- validation

Sonra **ratchet policy** uygulanır:

> Mevcut coverage düşemez; hedef zaman içinde artırılır.

Uzun vadeli hedefler:

- kritik saf hesap modüllerinde yüksek branch/line coverage,
- yalnız toplam repository yüzdesine güvenmeme,
- kritik hesap fonksiyonlarının her branch'inin en az bir semantic testle ilişkilendirilmesi.

## Frontend/WASM

TypeScript coverage yalnız authoritative finansal adapter/storage katmanında anlamlı kalite kapısı olarak kullanılmalıdır. Sunum komponentlerinde yüksek coverage uğruna düşük değerli snapshot testleri üretilmemelidir.

Öncelik:

- Decimal boundary
- storage schema
- browser payroll policies
- payment-event ordering
- export financial mapping

## CI katmanları

### Her PR

- cargo check
- payroll-core unit/regression
- golden corpus
- property tests
- native integration
- WASM tests
- Bun tests
- E2E smoke
- lint/clippy/fmt
- generated contract/WASM freshness

### Daha ağır periyodik/release gate

- mutation testing
- tam coverage raporu
- geniş property case count
- full golden corpus + differential oracle

## Kabul kriteri

CI yalnız "derleniyor ve testler geçiyor" kapısı değil, bordro doğruluk güvencesi kapısı haline gelir.

---

# 9. Faz 1 — Golden/mevzuat sürüm disiplini

## Amaç

Yasal parametre değişikliklerinin test verisini sessizce geçersizleştirmesini önlemek.

## Kural

Golden fixture'ın `legalYear` ve kullandığı yıllık parametre seti açık olmalıdır.

2026 fixture'ı, ileride 2027 parametreleri geldiğinde otomatik olarak 2027'ye taşınmamalıdır.

Yeni yıl süreci:

1. yeni `AnnualPayrollParameters` eklenir,
2. validation testleri eklenir,
3. yeni yıl için boundary fixture'ları oluşturulur,
4. en az temel normal bordro / vergi dilimi / istisna / PEK golden senaryoları bağımsız doğrulanır,
5. eski golden fixture'lar değişmeden kalır.

Bu, uygulamanın kapanmış bordroları yeniden hesaplayan genel tarihsel mevzuat motoruna dönüşmesi anlamına gelmez. Golden corpus test kanıtıdır; production geçmiş-mevzuat arşivi değildir.

---

# 10. Faz 3 — Release Evidence / test manifesti

## Amaç

Bir release'in hangi finansal güvence setinden geçtiğini tek yerden görebilmek.

## Eklenecek belge

`docs/payroll-assurance-status.md`

Belge eklendi. Güncel manifest golden/property/oracle sayısını, coverage
baseline'ını, mutation kapsamını, doğrulama SHA'sını ve bilinen boşlukları
taşır; tam mutation skoru CI artifact'ı oluşmadan uydurulmaz.

İçeriği:

- golden fixture sayısı
- property suite listesi
- oracle kapsamı
- mutation baseline / son skor
- coverage baseline / son durum
- kritik invariant listesi
- son tam doğrulama commit SHA'sı
- bilinen test boşlukları

Bu belge otomatik veya yarı otomatik güncellenebilir; ancak sayılar CI çıktısından gelmelidir, elle uydurulmamalıdır.

---

# 11. Auditability sınırı

Mevcut `GvHesapDetayi`, `DamgaVergisiHesapDetayi`, PEK detayları ve `ResolvedStatutorySnapshot` bordro hesabının neden o sonucu verdiğini incelemek için korunacaktır.

Bu plan bunların yerine yeni bir calculation engine kurmaz.

Ayrı bir "kim, hangi alanı, ne zaman değiştirdi" append-only kullanıcı işlem günlüğü istenirse bu ayrıca ürün/persistence özelliği olarak tasarlanmalıdır. Böyle bir ledger test güvence planının zorunlu ön koşulu değildir.

---

# 12. Öncelik ve uygulama sırası

## P0 — En yüksek değer

1. Faz 1 — Test güvence matrisi
2. Faz 1 — Golden Payroll Corpus
3. Faz 1 — Mevzuat/golden sürüm disiplini

Bu temel güvence katmanları tamamlanmadan yeni yüzlerce klasik regression testi
yazmak öncelik değildir.

## P1 — Bağımsız doğruluk

4. Faz 2 — Property-based testing
5. Faz 2 — Independent Reference Oracle

## P2 — Sürekli kalite kapısı

6. Faz 3 — Mutation Testing
7. Faz 3 — Coverage/CI ratchet
8. Faz 3 — Release evidence

---

# 13. Definition of Done

Bu yol haritası aşağıdaki koşullar sağlandığında tamamlanmış kabul edilir:

- [x] Kritik bordro kuralları için test güvence matrisi mevcut.
- [x] En az 30 exact golden bordro mevcut; 33 fixture korunuyor.
- [x] En az 10–15 kritik golden fixture bağımsız repo içi evidence'a bağlı; mevcut kapsam 15/15.
- [ ] Kalan 18 golden fixture bağımsız evidence çalışma kâğıtlarına bağlandı.
- [x] Golden expected değerleri production motorundan otomatik üretilmiyor.
- [x] GV, PEK, payment-event, puantaj/rapor ve Decimal için property testleri mevcut.
- [x] Property failure'ları replay/shrink edilebilir.
- [x] GV, PEK, DV ve temel normal bordro için production'dan bağımsız oracle mevcut.
- [x] Oracle ile production motoru generated input'larda differential test ediliyor.
- [ ] Native ve WASM/browser adapter parity testleri korunuyor.
- [x] Kritik Rust mali modülleri cargo-mutants ile ölçülüyor.
- [ ] Mutation score baseline kaydedilmiş ve gerileme politikası uygulanıyor.
- [x] Rust coverage baseline ölçülmüş ve ratchet policy uygulanıyor.
- [x] Golden + property testleri normal PR CI akışında blocking.
- [x] Mutation/coverage suite için release veya periyodik/manual workflow yapılandırıldı; mutation full 2.147 sonucu pending.
- [x] Production 2026 statutory parameter değerleri bağımsız reference fixture ile karşılaştırılıyor.
- [ ] Her yeni gerçek hesap hatası kalıcı regresyon vakasına dönüştürülüyor.
- [x] `docs/payroll-assurance-status.md` güncel güvence durumunu gösteriyor.

---

# 14. Uygulama prensipleri

## GitHub branch protection

Repository içinden GitHub branch protection/ruleset değiştirilemez; bu ayar
manuel yapılmalıdır. Required check önerisi: `CI / verify`.

1. **Tek production motoru korunur.** Test için ikinci production engine yaratılmaz.
2. **Oracle bağımsızdır ama test-only'dir.** Production akışına çağrılmaz.
3. **Exact Decimal korunur.** Golden/oracle karşılaştırmalarında float toleransı kullanılmaz.
4. **Net ödeme tek başına yeterli assertion değildir.** Ara matrah ve vergiler de doğrulanır.
5. **Bulunan bug test olur.** Her gerçek hata minimal kalıcı regression case'e dönüştürülür.
6. **Fixture update kolaylaştırılmaz.** Golden değişiklik bilinçli review gerektirir.
7. **Coverage hedef değil sinyaldir.** Yüksek coverage düşük kaliteli testin yerine geçmez.
8. **Mutation score test gücünü ölçer.** Equivalent mutant'lar gerekçesiz suppression ile gizlenmez.
9. **Mevzuat parametresi ile test oracle'ı aynı kaynaktan körlemesine türetilmez.** Aksi halde aynı hata iki tarafa da taşınabilir.
10. **CI deterministik olmalıdır.** Random/property test failure'ı yeniden üretilebilir olmalıdır.

Bu yaklaşım mevcut regresyon/invariant altyapısını koruyarak bordro motorunu sıradan yüksek test sayısından, ölçülebilir finansal doğruluk güvencesi seviyesine taşımayı hedefler.
