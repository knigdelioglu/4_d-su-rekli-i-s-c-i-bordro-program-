# Görev 4 — Shadow engine parity kanıtı

> **Aşama 2 güncellemesi:** Bu dosya, Görev 4 sırasında geçerli olan parity
> yuvarlama sonuçlarını tarihsel kayıt olarak içerir. Üretim parasal sonuçları
> için Aşama 2'de bulunan bağımsız dayanaklı yuvarlama kuralı ve yemek
> istisnası birleştirmesi [finansal denetim raporunda](payroll-financial-audit-stage2.md)
> yürürlüktedir. `10.005` örneklerindeki aşağıdaki eski kuruş sonuçları artık
> güncel beklenen değer değildir.

## Proof plan

| Gereksinim | Sahip akış | Kabul kanıtı |
|---|---|---|
| Shadow yalnız açık tam-state policy timeline'ı kullanır; legacy oracle değişmez | `resolve_compensation_segments` → private `cfg(test)` seam → ortak PEK/GV/DV/kesinti/carry orkestrasyonu | Aynı fixture'da tüm serileştirilen `BordroKaydi` eşitliği; production core derlemesi |
| Tam Decimal parity kayıtlı earning sınırlarını korur | Typed `FullEarning` boundary günlük base, meal, road, premium ve night earning slice'ını başlatır; parasal kuruş yuvarlaması Aşama 2'de ortak half-up oracle ile doğrulanır | Fractional single-field, automatic/explicit indexing, consecutive raise ve paid-sickness fixture'ları |
| Statutory yemek sınırı tek earning tutarını ikinci kez yuvarlamaz | Yemek earning compensation diliminde hesaplanır; aynı dilimdeki statutory kesişimleri önce toplanır | Tek compensation slice içindeki iki `10.005` gün, statutory tarih tek veya iki parçaya ayrılsa da `20.01` |
| Road-only dated boundary diğer kazançları bölmez | Tek full-state timeline içindeki `RoadOnly` typed metadata'sı yalnız `gunlukVasitaYol` değişimine izin verir ve earning slice'ını bölmez | Fractional road `None` / `Some([])` / `Some(changes)` ile bağımsız statutory ve compensation sınırları; eksik/çelişkili metadata reddi |
| Policy hataları fail-closed | Shadow giriş noktası mevcut full-state validator/resolver'ını çağırır | Eksik baseline, invalid state/date/order ve same-date correction leaf testleri |
| N≥3 formülleri legacy'den bağımsız doğrulanır | Sabit input'tan literal elle hesaplanan golden | 3 compensation segment + statutory sınır, tam bordro değerleri ve carry |

Günlük ücret girdileri her explicit compensation segmentinde hesaplanıp yasal kuruş kuralıyla yuvarlanır, sonra `sum_income_field` / premium-detail merge sözleşmesiyle birleştirilir. Günlük oran aynı kalsa da gerçek compensation sınırı korunur. Sosyal yardım, giyim ve hizmet zammı tüm inclusive takvim günleri ağırlıklandırıldıktan sonra bir kez yuvarlanır. Ek ödeme ve diğer gelir dönem sonu `Option` state'inden bir kez seçilir; null/zero ayrımı korunur.

## Uygulama ve seam

- `payroll_engine.rs` public legacy hesaplama girişini ve production davranışını korur. Policy override, version-ID keyed boundary metadata seam'i ve `compensation_shadow` modülü yalnız `cfg(test)` altındadır. `cargo check -p payroll-core` non-test core yolunu doğrular.
- Shadow policy giriş noktası dönem timeline'ını yalnız `compensationPolicyVersions` ile çözer. Eski dönem settings, `zamAylari` veya `resolve_raise_transition` bu hesap yolunda çağrılmaz. Legacy resolver yalnız test fixture adapter'ının açık policy state'lerini üretmesinde kullanılır.
- Legacy fixture adapter'ı road tutarlarını doğrudan aynı full-state policy version'ların `gunlukVasitaYol` alanına materyalize eder. `None` / `Some([])` / `Some(changes)` ayrımı yalnız version ID'lerine bağlı `RoadRoundingMode` metadata'sında taşınır; calculation path'inde ayrı road amount listesi, kaynak/metin çıkarımı veya ikinci runtime resolver yoktur.
- `FullEarning` her zaman yeni earning rounding slice'ı açar. Dönem içi `RoadOnly` boundary, etkin önceki full state ile yalnız `gunlukVasitaYol` farklıysa kabul edilir; diğer gelir dilimlerini birleştirir. Dated road tutarı her gün için ilgili full policy state'ten okunur.
- Tam parity assertion'ı JSON serileştirilmiş `BordroKaydi` değerlerini karşılaştırır; gelir/kesinti null ve zero, PEK, çalışan/işveren primleri, GV/DV detayları ve istisnalar, carry, statutory snapshot ve diğer tüm serileştirilen alanlar allowlist olmadan dahildir. Fixture zamanları sabitlenir; metadata her iki yolda da `None` beklenir.

### Rounding girdileri

| Gelir | Hak / ağırlık | Shadow dilim kuralı |
|---|---|---|
| Taban brüt | C,T,G,İ,GC,GÇT hak günleri; ücretli rapor günleri ayrıca | Her compensation version'da günlük ücret ve doğrulanmış ücretli rapor ücreti hesaplanıp birleştirilir |
| Yemek | C+GC | Her compensation version'da mevcut `calculate_meal_income`; exemption ayrıca compensation × statutory kesişiminde |
| Yol | C+GC | `PerCompensationSlice`: her `FullEarning` diliminde ayrı toplam ve kuruşa yuvarlama; `PeriodTotal`: tek full-state timeline'daki tüm uygun günleri topla ve bir kez kuruşa yuvarlama |
| İş primi | Aktif personel grubunun oranı × C+GC | Her compensation version'da mevcut premium helper; `merge_is_primi_details` ile amount, gün ve son hak edilen grup detayı birleştirilir |
| Gece / gece tatili | GC / GÇT | İki ayrı gelir, her compensation version'da mevcut gece helper'ı |
| Sosyal yardım / giyim / hizmet | Inclusive dönem takvim günleri | Ağırlıklı toplamdan sonra alan başına tek kuruş yuvarlaması |
| Ek ödeme / diğer gelir | Dönem sonu policy state'i | Dönem başına bir kez; nullable semantics korunur |

## Coverage ve regression haritası

`compensation_shadow::tests` 20 testi şunları doğrular: tüm BordroKaydi karşılaştırması; 28/29/30/31 günlük ve yıl sınırı dönemleri; automatic indexing, explicit override ve art arda iki raise; copy-forward açık policy state'leri; 10.005 gibi fractional base/meal/road/premium/night girdileri; statutory boundary'nin günlük gelirleri bölmemesi; road `None` / `Some([])` / `Some(changes)`; `FullEarning` ile aynı-rate boundary'nin korunması ve `RoadOnly` ile slice birleşmesi; dönem dışı baseline'ın yeni slice açmaması; eksik/tekrar/etkin olmayan metadata, çelişkili RoadOnly state'i ve dönem içi rounding-mode değişiminin reddi; grup ve iş primi detay merge'i; iki günlük paid sickness ve C/T/G/İ/GC/GÇT; union/BES kişi sabiti, kurum sabiti, percentage default ve üyelik önceliği; manual null/zero/decimal round; aynı ay supplementary önceliği, meal exemption, aylık PEK tüketimi, over-cap carry; sonraki ay ve 2027 zinciri; full-state baseline ve hatalı timeline fallback'siz hatalar.

Mevcut 14 senaryolu `raise_transition_regression` paketinin eşlemesi:

| Legacy senaryo | Görev 4 karşılığı |
|---|---|
| `all_four_incomes_split_at_first_of_month_for_28_29_30_31_day_periods_and_new_year` | `full_record_parity_covers_28_29_30_and_31_day_service_periods`, `year_crossing_service_period_uses_explicit_payment_tax_anchor`, fractional earnings testleri |
| `union_dues_use_the_new_daily_wage_without_weighting_paid_days` | `union_and_bes_personal_fixed_institution_fixed_default_and_membership_precedence_match`; kişi/kurum sabiti, percentage default, üye değil ve kesit sonrası paid day yok durumları |
| `automatic_raised_rates_continue_into_two_copied_following_periods` | `cent_precision_auto_indexing_carries_forward_as_an_explicit_policy_state`; ayrıca NORMAL/TİS/NORMAL kopyalanan policy zinciri |
| `consecutive_scheduled_raises_compound_the_effective_rates` | `consecutive_automatic_raises_compound_before_explicit_policy_calculation` |
| `explicitly_entered_rates_take_precedence_without_being_raised_twice` | `automatic_raise_indexing_and_explicit_overrides_become_full_policy_states` |
| `only_worked_days_earn_meals_but_paid_sick_days_keep_their_correct_wage_rate` | `paid_sick_days_and_all_attendance_labels_match_across_the_policy_cut`; Jan31/Feb1 paid sick ücretleri ayrıca literal kontrol edilir |
| `missing_previous_period_missing_settings_gaps_and_invalid_raise_months_fail_closed` | Yeni engine eski geçmiş/raise listesine bağlı değildir; shadow `policy_resolver_failures_do_not_fall_back_to_legacy_settings` içinde eksik baseline, invalid full state/date/order ve aynı-tarih supersession leaf doğrulanır. Legacy invalid-history/raise-month davranışının oracle testi eski regression paketinde kalır. |
| `pre_raise_slice_accepts_a_group_first_defined_in_the_current_period` | `current_only_premium_group_is_materialized_for_the_pre_raise_legacy_slice` |
| `indexed_social_aid_carries_forward_at_cent_precision` | `cent_precision_auto_indexing_carries_forward_as_an_explicit_policy_state`; raw effective state `5450.05076`, ücret çıktısı `5450.05` |
| `retro_replay_does_not_create_allowance_differences_from_segment_rounding` | Görev 6 retro owner'ı; Görev 4 normal-accrual shadow kapsamı dışında |
| `retro_absolute_base_wage_revision_does_not_implicitly_reindex_other_income` | Görev 6 retro owner'ı |
| `retro_replay_after_transition_uses_full_raised_allowances` | Görev 6 retro owner'ı |
| `retro_meal_tax_treatment_uses_the_source_period_meal_policy` | Görev 6 retro owner'ı |
| `rate_history_changes_protect_cross_year_payrolls_and_retro_ledgers_without_source_payroll` | Normal cross-year kısmı `year_crossing_service_period_uses_explicit_payment_tax_anchor`; retro ledger kısmı Görev 6 owner'ı |

## Üç-segment bağımsız golden ve statüter yemek kesişimleri

2026-01-15–2026-02-14 döneminde yalnız bir dönem/ayar, puantaj ve eski bordrosuz üç explicit policy version vardır. İlk iki gün C, kalan günler G'dir. Compensation state tarihleri ve ilgili girdiler:

| Tarih | Gün | Günlük taban | Sosyal yardım | Giyim | Hizmet birimi |
|---|---:|---:|---:|---:|---:|
| 2026-01-15 | 1 | 100 | 100 | 10 | 1 |
| 2026-01-16–31 | 16 | 101 | 110 | 20 | 1.5 |
| 2026-02-01–14 | 14 | 102 | 150 | 30 | 2 |

Günlük yemek tüm state'lerde 10.005; statutory state 2026-01-16'da değişir. Günlük asgari ücret 100, tavan katsayısı 9, yemek SGK/GV cap'leri ilk gün için 1000, sonraki günler için 1001'dir. Dönem sonu ek ödeme `Some(0)`, diğer gelir `None`; kalan günlük oranlar sıfırdır.

Elle hesaplanan literal sonuçlar:

- Taban `100×1 + 101×16 + 102×14 = 3144`.
- Yemek iki farklı compensation segmentindeki birer C günü olduğundan her gerçek earning dilimi ayrı yuvarlanır: `10.005 → 10.01`; toplam `20.02`.
- Sosyal `127.74`; giyim `24.19`; hizmet `5.13` (ağırlıklı ham tutarların tek sefer kuruşa yuvarlanması).
- Brüt toplam `3321.08`; yemek istisnası `20.02`; ham/prim/final PEK `3301.06`, alt/üst sınır `3000/27000`.
- İşçi SGK `462.15`, işsizlik `33.01`, GV `38.39`, DV `2.29`; kesinti toplamı `535.84`, net `2785.24`. İşveren SGK `717.98`, işsizlik `66.02`, toplam `784.00`.
- GV matrahı `2805.90`, brüt GV `420.89`, asgari ücret matrahı/istisnası `2550/382.50`; brüt damga `25.06`, aylık ve uygulanan istisna `22.77`. PEK carry giriş/çıkış listeleri `Some([])`.

Güncel helper kontrolünde compensation boyunca tek yemek state'i kullanılır. İki `10.005` günü iki statutory slice'a düşerse önce iki kesin kesişim toplanır ve bir kez yarım-yukarı yuvarlanır: `20.010 → 20.01`. Tek statutory dilimde de sonuç `20.01` olur. Böylece yalnız gerçek earning sınırı tutarı böler; statutory-only sınır aynı kazancı ikinci kez yuvarlamaz.

## Parity sonucu ve model integration gate

Önceki `310.15 / 310.16` farkı kapandı. `one_field_social_policy_keeps_unchanged_meal_raise_rounding_boundary` testinde sosyal yardım değişirken sabit kalan `10.005` yemek oranı için legacy ile shadow meal `310.15`, brüt `3514.67`, net `2987.47` ve tüm serileştirilmiş kayıt aynıdır. Test tam record equality kullanır.

Fractional boundary regresyonu elle hesaplanan şu gelirleri full record parity ile doğrular: base `3100.15`, meal `110.05`, premium `110.06`, gece `15.03`, gece tatili `20.03`. Road `None` için `PerCompensationSlice` rounding sonucu `220.05`; `Some([])` için `PeriodTotal` ve current road rate tüm eligible günlerde tek toplam rounding sonucu `220.06`; 2026-01-25'teki full-state road amount boundary için `3×20.005 + 8×30.005 = 300.055 → 300.06`. Ayrı statutory kesim 2026-01-23, compensation kesimi 2026-02-01'dir. 2026-01-25 boundary'si `RoadOnly` olarak işaretlenir ve base/meal/premium/night dilimlerini bölmez. Üç durumda da değerler ve bayraklar dışında fixture girdileri ve oracle değişmemiştir.

### Test-only typed candidate

Test-only seam şu version-ID keyed tipi kanıtladı:

```rust
enum CompensationBoundaryScope {
    FullEarning,
    RoadOnly,
}

enum RoadRoundingMode {
    PerCompensationSlice,
    PeriodTotal,
}

struct PolicyBoundaryMetadata {
    versionId: String,
    scope: CompensationBoundaryScope,
    roadRounding: RoadRoundingMode,
}
```

Her resolved active version tam bir metadata kaydı taşımalı. Eksik, yinelenen, bu dönemde etkin olmayan version ID'si veya farklı road rounding mode'ları validation hatasıdır. Metadata `source`, `reason`, rate equality veya başka bir alan üzerinden çıkarılmaz. `FullEarning` aynı full state değerlerinde bile earning slice'ını böler. Dönem içi `RoadOnly`, önceki active full state ile sadece `gunlukVasitaYol` farklıysa kabul edilir; aksi halde fail-closed davranır. Road günlük tutarı daima o günün full `CompensationPolicyState.gunlukVasitaYol` alanından gelir. Önceki effective version dönem başında baseline olarak seçildiğinde eski tarih yeni earning slice oluşturmaz.

`RoadRoundingMode` tüm active version'larda bir bordro dönemi boyunca aynı olmalı. `PerCompensationSlice`, `FullEarning` slice'larında ayrı road toplamı ve yuvarlama yapar; `PeriodTotal` tek timeline'daki tüm C+GC tutarlarını toplar ve bir kez yuvarlar. Rounding mode dönem ortasında değişemez; sonraki dönem baseline'ı yeni mode taşıyabilir.

Fixture eşlemesi:

| Legacy fixture girişi | Full-state version'lar | Typed metadata |
|---|---|---|
| `transportRateChanges == None` | Road tutarı diğer compensation alanları gibi version state'lerinde kalır | Her aktif version `FullEarning` + `PerCompensationSlice` |
| `transportRateChanges == Some([])` | Dönem road baseline tutarı, mevcut compensation version state'lerinin road alanına kopyalanır | Her aktif version `FullEarning` + `PeriodTotal` |
| `transportRateChanges == Some(changes)` | Baseline ve her dated road amount, aynı full-state version dizisine materyalize edilir; compensation cut sonrasında da etkin road amount state'e taşınır | Salt road tarihleri `RoadOnly`; aynı gün compensation cut varsa `FullEarning`; tüm aktif version'lar `PeriodTotal` |

Bu, paralel road amount girdisi değildir: adapter legacy fixture'ını kurarken tüm tutarları tek `CompensationPolicyState` timeline'ına yazar; calculation seam'ine yalnız typed metadata eklenir. Candidate henüz frozen domain model, SQLite, snapshot, native/WASM/browser DTO veya production loader'a eklenmedi.

### Önerilen production contract ve entegrasyon etkisi

Orchestrator/finansal owner incelemesi için önerilen kalıcı alanlar, immutable `CompensationPolicyVersion` satırında zorunlu enum alanlarıdır:

```text
boundaryScope: FullEarning | RoadOnly
roadRoundingMode: PerCompensationSlice | PeriodTotal
```

Wire token'ları sabit ve typed olmalı (ör. `FULL_EARNING`, `ROAD_ONLY`, `PER_COMPENSATION_SLICE`, `PERIOD_TOTAL`). Alanlarda serde/schema default bulunmamalı. Eksik, bilinmeyen veya tutarsız token yeni policy calculation için invalid data sayılmalı; eski/payroll tarihi, eşit tutar veya `source`/`reason` üzerinden otomatik backfill yapılmamalı. Aynı effective date düzeltmesi yeni immutable version ve kendi iki flag'iyle append edilir; `RoadOnly` version'ın parasal road değeri full state'te durur.

Entegrasyon etkileri ve kabul edilmesi gereken kontroller:

| Alan | Gerekli entegrasyon |
|---|---|
| Domain ve SQLite | `CompensationPolicyVersion` serde modeline iki zorunlu enum alanı eklenmeli. `compensation_policy_versions` migration'ı alanları saklamalı ve allowed token'ları doğrulamalı; repository SELECT/INSERT ve correction flow bunları aynen taşımalı. Önceden oluşturulmuş flag'siz kayıt varsa owner onaylı provenance olmadan hesaplanmamalı; release öncesi açık backfill veya karantina kararı gerekir. |
| Payroll snapshot / replay | Mevcut resolved compensation snapshot state segmentlerini saklıyor. Snapshot ayrıca resolved active `versionId` başına iki flag'i saklamalı; schema version artırılmalı ve replay eksik/çelişkili metadata'da reddetmeli. Legacy kayıtlar legacy provenance yolunda kalmalı, yeni default ile sessizce yeniden yorumlanmamalı. |
| Native ve WASM loader | Dönem baseline'ı, dönem içi active version'lar ve supersession zinciri loader'dan boundary metadata ile beraber gelmeli. Native/WASM aynı enum doğrulamasını ve tam rounding testlerini çalıştırmalı. |
| Browser, backup ve sözleşmeler | TS payroll types, generated Rust↔TS contract, payload schema/normalizer, IndexedDB persistence, import/export ve backup restore yeni alanları kayıpsız taşımalı. Eksik enum'a browser-side default verilmemeli. |
| Invalidation ve stale state | Boundary flag'i payroll sonucunun girdisidir: snapshot/digest'e katılmalı. Aynı effective date'e yeni correction eklendiğinde etkilenen dönem ve bağlı vergi/PEK carry zinciri invalidation değerlendirmesine girmeli; finalized kilitleri korunmalı. |
| Ücret politikası UI'si | Create/correction akışı boundary rolünü eşit rate'lerden tahmin etmemeli; kullanıcıya/işlem türüne açık seçim sunmalı. Road amount yine full state alanında düzenlenmeli. UI henüz bu iki alanı temsil etmiyorsa policy yazımı başlatılmamalı. |

Bu değişiklikler bu handoff kapsamı dışında ve ana model/storage sahibi tarafından uygulanıp incelenmeli. Handoff, schema/model, 3B, TypeScript, SQLite, loader, UI veya task contract dosyalarını değiştirmedi.

**B-RND-02 / Task 4 gate BLOCKED.** Test-only candidate üç legacy road varyantını tek full-state timeline ve explicit flags ile tam `BordroKaydi` equality'ye taşıdı. Production gate, owner'ın bu alanları ve snapshot/storage/loader/UI/invalidation etkilerini onaylayıp entegre etmesini ve regression proof'u tekrarlamasını bekler. Task 5 başlatılmadı. Legacy `raise_transition_regression` 14 senaryosunun row-by-row eşlemesi yukarıda; dört retro senaryosu Task 6 owner'ına bırakılmıştır ve bu handoff'ta çalıştırılmamıştır. WASM build/source-hash/package doğrulaması yapılmadı.

## Gerçek doğrulama

| Komut | Sonuç |
|---|---|
| `cargo test -p payroll-core --lib compensation_shadow::tests` | PASS — 20/20 |
| `cargo test -p payroll-core` | PASS — tüm lib ve integration hedefleri |
| `cargo check -p payroll-core` | PASS — production/non-test core, warning yok |
| `git diff --check -- <üç sahipli dosya>` | PASS |

WASM build/source-hash/package doğrulaması bu task scope'unda yapılmadı; orchestrator final consolidation bekliyor. Test-only parity proof geçmiştir; Task 4 production gate'i ise model/storage/snapshot/loader/contract integration owner kararı ve uygulaması beklediği için BLOCKED kalır.
