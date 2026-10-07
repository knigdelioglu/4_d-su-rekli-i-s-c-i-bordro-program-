# Görev 5A — Ücret politikası API ve UI sözleşmesi handoff'u

## Durum ve sınır

5A, 5B normal motor entegrasyonu ile Görev 7 UI'ının bağımsız çalışabilmesi için ortak TypeScript contract'ını ve dar tip/contract testlerini ekledi. Runtime load, preview, save, loader, native command, browser mutation veya UI sayfası bu görevde uygulanmadı. Contract modülünü import etmek tek başına bir API'yi etkinleştirmez; Tauri command kaydı, hook ve App route eklenmedi, repository'ye direct append yüzeyi açılmadı.

Ana geçiş contract'ı ve görev haritası değiştirilmedi. Finansal/domain/resolver/snapshot, SQLite/repository implementasyonu, backup/V6, WASM paketleri, `payroll_engine.rs`, `retro.rs` ve eski production calculate davranışı bu görevin dışında tutuldu.

## Dondurulmuş ortak modül

Ortak kaynak: [`src/services/compensationPolicy/contract.ts`](../src/services/compensationPolicy/contract.ts). Hem 5B hem 7 bu dosyayı tüketir; her iki owner da dosyayı paralel olarak yamamaz. Contract revizyonu gerekirse önce orchestrator'a öneri iletilir, tek owner değiştirir ve iki dal revize contract'a göre tekrar doğrulanır.

Temel export'lar:

- Canonical engine modellerinden türetilmiş `CompensationPolicyStateWire`, `CompensationPolicyVersionWire` ve `ResolvedCompensationSegmentWire`; para alanları `Exactify` ile wire üzerinde `string`/nullable-string'dir.
- Canonical mevcut `CompensationBoundaryScope` (`FULL_EARNING | ROAD_ONLY`) ve `RoadRoundingMode` (`PER_COMPENSATION_SLICE | PERIOD_TOTAL`). Yeni ücret/yol otorite kaynağı veya delta/replay alanı yoktur.
- `CompensationPolicyApi` metotları ve `CompensationPolicyLoadRequest`, `CompensationPolicyPreviewRequest`, `CompensationPolicySaveRequest` DTO'ları.
- `CompensationPolicyWriteOperation`: `INITIAL_BASELINE`, `NEW_EFFECTIVE_DATE`, `SAME_DATE_CORRECTION` ayrımı. Düzeltme `supersedesVersionId` ile aynı tarihli aktif leaf'i hedefler. `id`, `createdAt`, `source` backend'e aittir; `reason`, effective date, full state ve iki boundary flag açık girdidir.
- `CompensationPolicyPageProps`, `CompensationPolicyStateDraft`, `CompensationPolicyDraftValidation`, async state ve save view-state union'ları.

Raw form draft, exact domain DTO değildir. Formdaki her Decimal metni ham `string` olarak tutulur; kullanıcı yazarken geçici/geçersiz metin bulunabilir. Nullable parasal alanlarda `{ kind: 'ABSENT' }` ile `{ kind: 'VALUE', raw: '0' }` ayrıdır. Wire tarafında alanlar eksiksizdir, yokluk `null` ile ifade edilir ve açık sıfır `'0'` olarak kalır. Dönüşüm yalnız exact Decimal helper/validasyon sonrasında yapılır; para için JavaScript `Number` kullanılmaz.

### Load semantiği

`load` kapsamı tek mevcut kurum timeline'ı ve bir bordro hizmet dönemi aralığıdır: `periodId`, inclusive başlangıç/bitiş ve as-of projection tarihi. Dönem izdüşümü, repository'nin `get_versions_for_period` scoped okumasına karşılık gelir: dönem başlangıcını kapsayan en son effective date ile dönem içi sürümler ve aynı-tarih supersession ata kayıtları gelir. Bu, tam timeline audit'i değildir.

Yük sonucu raw timeline sürümlerini, çözümlenmiş aralıkları, `RESOLVED | MISSING_START_POLICY` durumunu ve `ACTIVE | MISSING` aktif-policy izdüşümünü ayrı sunar. Yükleyici başlangıç policy'si üretmez. Aktif dönem ayarlarından gelen `nonAuthoritativePrefill` `PREFILL_ONLY` olarak işaretlidir, eksik alanları taşıyabilir, `null`/`'0'` farkını korur ve kayıtlı ücret otoritesi sayılamaz. Prefill verilmeyişi veya eksikliği boş değerleri finansal default'a çevirmez.

Scoped okumanın içinde gereken eski/flag'siz/invalid satır `QUARANTINED_POLICY` hatasıdır ve satır kimliği/effective date/hata sınıfı taşır. Kapsam dışı bozuk kayıtları scoped projection'a zorla eklemek yoktur. Preview/save stale-state ve correction güvenliği için tam kurum timeline audit'i ayrıca yapar; bu yol kapsam dışı karantinayı da yakalayıp fail-closed olur. Scoped load ile global audit aynı kavram değildir.

### Operasyon, RoadOnly ve stale-write semantiği

- Başlangıç baseline'ı, ileri tarihli yeni policy ve aynı-tarihli correction farklı tagged operation'lardır. İlk baseline için save request'te effective date'e bağlı açık `initialBaselineConfirmation` gerekir. Baseline `ROAD_ONLY` olabilir; dönem başlangıcındaki/clipped baseline scope'u, önceki bir policy karşılaştırması yok diye reddedilmez.
- Sonraki tarihli sürümlerde ve same-date correction'larda `ROAD_ONLY` yalnız `ROAD_AMOUNT_ONLY` intention ile ifade edilir. Backend, road tutarı dışındaki full state'in önceki aktif full state ile eşitliğini doğrular. Dönem içindeki aktif sürümlerin `roadRoundingMode` değeri de backend'de ortak dönem invariant'ı olarak doğrulanır.
- Her sürüm, `boundaryScope` ne olursa olsun tam ve authoritative `CompensationPolicyState` taşır. `ROAD_ONLY` delta, inheritance veya yalnız yol değeri değildir. Effective date'te state kaynağını her tür sonraki aktif policy sürümü değiştirir. Bu nedenle preview `policyStateWindow` bitişini bir sonraki aktif sürümden hesaplar. Rounding kapsamı ayrıdır: `FULL_EARNING` yeni kazanç yuvarlama dilimi açar; `ROAD_ONLY` var olan dilimi bölmez. Preview ayrıca `earningRoundingWindow` ve bir sonraki `FULL_EARNING` tarihini verir; iki pencere birleştirilmez.
- Bir correction/ekleme, sonraki mevcut `ROAD_ONLY` sürümünün non-road eşitliğini bozuyorsa `ROAD_ONLY_STATE_CONFLICT` ile başarısız olur; descendant state sessizce yeniden yazılmaz/türetilmez.
- Preview sonucu doğrudan compensation state etkisini, GV/PEK/payment/carry dependency closure etkilerini ve `finalizedBlockers` listesini ayrı taşır. CALCULATED etkisi açık `CALCULATED_TO_STALE` olarak görünür. Preview yetki değildir.
- `previewToken`, aynı operation'ın exact kopyasını, hedef effective date'teki aktif leaf kimliklerini, tam timeline'ın sıralı version ID'lerini ve dependency closure payroll kimlik/status/timestamp/provenance/snapshot token'larını taşır. Save bunu sadece istemciden gelen bir izin gibi kullanmaz: native/browser save sınırında authoritative timeline ve dependency durumunu tekrar okur, token/operation eşitliğini kontrol eder ve preview'ı transaction içinde yeniden hesaplar. Eski hedef leaf veya state conflict üretir. Yeni `FINALIZED` closure varsa policy append ve CALCULATED→STALE geçişleri dahil işlem bütünü rollback olur.
- Geçmiş bir FINALIZED ekonomik hakkı normal ileri timeline correction'ı ile değiştirilmez; retro revision/settlement yolu gerekir. Current-engine payroll snapshot'ı normal calculate entegrasyonunda zorunludur. Bu contract veya Task 5 entegrasyonu production activation izni değildir; parity, gerçek V6 replay ve gerçek acceptance kapısı Task 8/orchestrator'dadır.

### Önerilen Rust DTO eşlemesi (runtime uygulanmadı)

Mevcut core `CompensationPolicyState`, `CompensationPolicyVersion`, `ResolvedCompensationSegment`, `CompensationBoundaryScope` ve `RoadRoundingMode` kullanılmalı; ikinci finansal DTO/domain enum'u türetilmemeli. Uygulama katmanında Serde `tag = "kind"` kullanan `CompensationPolicyWriteOperationDto` enum'u (`InitialBaseline`, `NewEffectiveDate`, `SameDateCorrection`) ve contract'taki target/preview token DTO'ları yeterlidir. Alan adları `camelCase`, operation token'ları contract'taki SCREAMING_SNAKE_CASE olur. Core Decimal alanları mevcut `rust_decimal::serde::str` / `str_option` ile exact string/null/zero semantiğini korur. Rust komut sonucu TypeScript `CompensationPolicyOperationResult<T>` discriminator'ına eşlenebilir; doğrudan `CompensationPolicyRepository::insert_version` IPC'ye açılmaz.

## UI component contract'ı

Görev 7'nin eklenecek component'i [`src/components/Settings/CompensationPolicyPage.tsx`](../src/components/Settings/CompensationPolicyPage.tsx) içinde named `CompensationPolicyPage` export etsin ve yalnız `CompensationPolicyPageProps` tüketip yayınlasın. Dondurulmuş callback adları:

- `onLoad`, `onDraftChange`, `onValidateDraft`
- `onConfirmInitialBaseline`, `onCancelInitialBaseline`
- `onPreview`, `onSave`, `onReloadAuthoritative`

Props; load `idle/loading/ready/error`, draft ve tam-state validasyonu, preview bekleme/hata/etki sonucu, saving, conflict, blocked, hata, authoritative reload sürüyor, reload başarısız ve başarı durumlarını ayırır. `succeeded` save state'i receipt yanında başarılı `authoritativeLoad` taşımak zorundadır. Commit tamamlanıp reload başarısızsa yalnız `persisted-reload-failed` gösterilir; doğrulanmamış form draft'ı saved/authoritative olarak gösterilmez. `finalizedBlockers` preview success içinde listelenir, save'de blocker ise typed hata olur.

## Requirement → flow → success/failure → evidence

| Gereksinim | Akış / sahibi | Başarı davranışı | Hata davranışı | 5A kanıtı |
|---|---|---|---|---|
| Canonical full state ve exact Decimal wire | `contract.ts` → 5B bridge/native/browser ve 7 form | Full state bütün alanları taşır; exact string, nullable `null`, açık `'0'`, grup `aktif` ve iki metadata flag'i korunur | Wire numeric money type'ı derleme hatası; runtime decimal sınırı numeric alanı reddeder | `contract.test.ts`: full DTO `satisfies`, number `@ts-expect-error`, `assertExactDecimalDto`, serialize/parse equality |
| Load timeline, aktif projection, missing baseline ve prefill | `CompensationPolicyApi.load` → scoped repository read → UI | Timeline + active projection + missing-start durumu + açıkça non-authoritative prefill | Scoped kayıttaki unsupported metadata quarantine sonucu; loader baseline yazmaz | `contract.test.ts` ready/missing ve quarantine result tipleri; runtime load henüz yok |
| Baseline, effective date, correction ve provenance | Write operation → 5B service transaction | Üç operation ayrı typed; full state/flag açık; backend ID/time/source sahibi; correction leaf'i açık supersedes hedefi | Stale leaf conflict; yanlış RoadOnly state veya invalid descendant için fail-closed | `contract.test.ts` baseline/new/correction fixtures ve correction hedef assertion'ı |
| Preview, dependency closure, FINALIZED ve stale write | Preview → token → transactional save | Direct policy-state window ve rounding slice ayrı; GV/PEK/payment/carry ve blocker listeleri ayrı; token tam state'i bağlar | Token/operation değişmişse conflict; current finalized closure varsa append + stale marking birlikte rollback | Type/shape contract ve ROAD_ONLY window fixture; gerçek transaction proof'u 5B'de |
| UI lifecycle ve authoritative reload | `CompensationPolicyPageProps` → 7 component → 5B hook | Confirm/cancel, validation, preview, saving/conflict/blocked/success; success sadece reload ile | Reload hatası persisted ama authoritative olmayan durum olarak kalır | Props fixture; React davranış/component tests 7'ye ait |
| Runtime güvenlik ve production gate | 5B native/browser application API + Task8 acceptance | License/service gate arkasında typed API; current-engine snapshot ve atomik invalidation | Yetkisiz istek, eksik snapshot, quarantine veya finalized closure fail-closed | 5A'da command/runtime bilerek eklenmedi; uygulama kanıtı 5B/8'e ait |

## Kesin dosya sahipliği: sonraki paralel işler

| Owner | Exclusive dosyalar / sorumluluk |
|---|---|
| 5A frozen shared contract | `src/services/compensationPolicy/contract.ts`, `src/services/compensationPolicy/contract.test.ts`, bu handoff dosyası. Bu dosyalar teslimden sonra ortak sözleşme olarak dondurulur; değişiklik önerisi orchestrator'a gider. |
| 5B motor, loader, preflight, invalidation ve runtime wiring | `crates/payroll-core/src/payroll_engine.rs`, `crates/payroll-core/src/policies.rs`, `src-tauri/src/services/payroll_preflight_service.rs`, `src-tauri/src/services/payroll_service.rs`, `src-tauri/src/repositories/payroll_invalidation_repo.rs`, `src-tauri/src/commands/compensation_policy_cmd.rs` (yeni), `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`, `src/services/payrollEngine/index.ts`, `src/services/payrollEngine/wasmRuntime.ts`, `src/services/payrollEngine/types.ts` (yalnız gereken adapter wiring), `src/services/tauriBridge.ts`, `src/services/storage/browserPayrollStore.ts`, `src/services/storage/browserPayrollPolicies.ts`, `src/hooks/usePayrollMutationController.ts`, `src/App.tsx`. Core resolver/domain, snapshot, schema/migration, repository veri modeli, backup/V6 codec, generated contract ve retro modülleri bu sahipliğe dahil değildir. Yeni adapter dosyası gerekirse önce orchestrator'a bildirilsin. |
| 7 yeni politika UI'ı | `src/components/Settings/CompensationPolicyPage.tsx`, `src/components/Settings/CompensationPolicyForm.tsx`, `src/components/Settings/compensationPolicyDecimal.ts`, `src/components/Settings/CompensationPolicyPage.test.tsx`, `src/components/Settings/compensationPolicyDecimal.test.ts`. Bu dosyalar yenidir; `App.tsx`, hook, bridge, `contract.ts` ve storage sahibi değildir. |

5B, native load için scoped `get_versions_for_period` davranışını korumalı; preview/save stale ve correction kontrolü gereken yerde full audit'i ayrı kullanmalıdır. Mevcut repo append helper'ını yalnız service-owned transaction içinde çağırmalı. Native save command'ı uygun license/security kontrolü olmadan Tauri handler'a kaydedilmemeli. Browser tarafı aynı operation/token/error/rollback semantiğini IndexedDB transaction sınırında sağlamalı. İki runtime aynı DTO'ları kullanır.

## Başlatma talimatı ve açık entegrasyon işleri

1. 5B ve 7 `src/services/compensationPolicy/contract.ts` export'larını bu sürümde frozen kabul edip kendi sahiplik tablosundaki dosyalarda çalışır. Handoff ve ortak test değiştirilmez.
2. 5B önce native/browser API'yi `CompensationPolicyApi` ile uyumlu bağlar; scoped load ile tam audit'i ayırır; token karşılaştırması, same-date leaf kontrolü, road-only descendant validasyonu, dependency closure tekrarı, finalized rollback, current-engine snapshot şartı ve auth/license kapısını uygulayıp test eder. Normal legacy calculate üretim yolu 5A tarafından değiştirilmedi; production switch/parity/V6 activation gate ayrıca kalır.
3. 7 page/form ve exact Decimal helper'ları contract draft'tan exact state'e Number kullanmadan dönüştürür. İlk baseline'da explicit confirm/cancel ister; valid full state olmadan preview/save çağırmaz; ROAD_ONLY niyetini kullanıcıdan alır; save sonrası success için `authoritativeLoad` şartını uygular.
4. Hangi sahip olursa olsun API şekli yetersiz çıkarsa kendi dalında ortak tipe geçici paralel patch atmaz; orchestrator'a ihtiyaç ve uyumsuz senaryoyu bildirir. Tek sözleşme revizyonu sonrası iki owner yeniden doğrular.

Runtime tarafında hâlâ uygulanması gerekenler: native command/service ve license gate; scoped loader ve karantina mapping'i; browser eşdeğeri; exact UI draft converter/validation; tam etki/closure preview; optimistic token recheck ve transaction rollback; CALCULATED→STALE atomikliği; authoritative reload; real native/browser integration tests. Bunlar 5A'da implemented değildir. Mevcut normal production bordro hesabı eski yolda kalır; current-engine snapshot üretimi ile production activation 5B/8 gate'lerinde kalır.

## 5A dosyaları ve gerçek doğrulama

- Yeni: `src/services/compensationPolicy/contract.ts`
- Yeni: `src/services/compensationPolicy/contract.test.ts`
- Yeni: `docs/compensation-task-5a-handoff.md`

| Komut | Sonuç |
|---|---|
| `bun test src/services/compensationPolicy/contract.test.ts` | PASS — 7 test, 30 assertion |
| `bunx tsc --noEmit --pretty false` (değişiklik öncesi baseline ve değişiklik sonrası) | EXIT 2 — iki çalıştırmada 66 mevcut genel TS diagnostic; yeni contract/test dosyalarında diagnostic yok. Yeni contract type assertion'ları/`@ts-expect-error` kontrolleri bu project run içinde derlendi. |
| `bun run build` | PASS — `verify:wasm-freshness` PASS, Vite production build PASS; bundle büyüklüğü için advisory warning var. |
| `git diff --check -- src/services/compensationPolicy docs/compensation-task-5a-handoff.md` | PASS — tracked diff boşluk kontrolü; yeni dosyalar ayrıca aşağıdaki son whitespace taramasına dahil edildi. |

WASM kaynakları/paketi 5A'da değiştirilmedi; package rebuild çalıştırılmadı. Test listesi yalnız bu handoff'ta gerçekten çalıştırılan komutları yansıtır.
