# Görev 8A — V6 finansal replay ve kabul handoff'u

## Durum ve sınır

8A replay uygulaması mevcut: current-engine NORMAL `CALCULATED`/`FINALIZED`
V6 snapshot'ı, kaydedilmiş girdilerden aynı bordro motoru üzerinden yeniden
hesaplanır ve tam finansal projeksiyonla karşılaştırılır. Kaynak eksikliği,
aynı ödeme ayındaki önceki event uyuşmazlığı ve finansal alan değişikliği
fail-closed sonuç verir. Native, WASM ve browser payload sınırlarında tam
bağlamlı validator kullanılır.

Bu takipte kalan uçtan uca 8A kabul akışları native SQLite ve gerçek Chromium/
IndexedDB üzerinde tamamlandı. Aşağıdaki kanıtlar 12 dönemli native ve üretilmiş
WASM zincirini, V6/V5 restore yollarını ve policy timeline silme bütünlüğünü
kapsar.

Legacy hesap yolu, olağan uygulama route'u, eski alanlar ve eski hesap oracle'ı
korundu. Görev 8B temizliği/route switch'i, operasyonel aktivasyon ve canlı
bordro verisi değişikliği yapılmadı.

## Gereksinim → kanıt

| Gereksinim | Uygulama ve kanıt |
|---|---|
| Canonical replay | `snapshot_replay.rs`, `usedInputs`, kaydedilmiş compensation segmentleri ve statutory/deduction girdileriyle mevcut normal engine seam'ini çağırır. Canlı compensation settings veya uydurma policy kimliği kullanmaz. |
| Tam tutar doğrulaması | `payroll_snapshot.rs`, üretilen gelir/kesinti kalemleri, brüt/kesinti/net, GV/DV, PEK/prim, cumulative, carry, statutory ve attendance sonuçlarını Decimal değer eşitliğiyle karşılaştırır; hata fark eden alanı adlandırır. |
| Aynı ay önceki event'leri | Replay, snapshot'taki accrual kimliği, dönem, sıra ve ödeme tarihine göre tek authoritative satırı ister; eksik/duplicate satırı ve frozen GV/PEK/carry/statutory farkını reddeder. Eski pozitif ödeme satırı, sıfır ledger görünse dahi downstream'da emekli sayılmaz. |
| Native/WASM/browser validator sınırı | `compensation_policy_entry_test::native_2026_2027_acceptance_roundtrips_v6_v5_and_policy_timeline`, gerçek native servisleriyle 12 dönemin calculate → persist → finalize akışını, V6 restore/readback/replay'i ve restore sonrası normal+retro hesabı doğrular. `api.browser.test.ts` içindeki yeni Chromium kabul testi, üretilmiş WASM ve gerçek `useBackupController`/IndexedDB yükleme yoluyla aynı zinciri ve taze sayfa/runtime hesabını doğrular. |
| Gerçek V6 kayıt | Native test, `PayrollService::build_dataset_snapshot` ve repository policy timeline'ını V6 payload şekline bağlayıp yeni SQLite veritabanına restore eder; 12 finalized satırın tümünü replay eder. Chromium testi V6 JSON'u uygulamanın backup controller'ından mevcut sentinel kayıt üzerine import eder, sentinel'ın değiştiğini doğrular, sayfayı yeniden yükleyip state'i IndexedDB persistence hook'undan okur ve yeni WASM runtime'da normal+retro hesap yapar. |
| 2026–2027 geçmişsiz senaryo | Core'daki `compensation_shadow.rs::synthetic_2026_2027_acceptance_uses_sep_2026_baseline_without_wage_history` testi korunuyor. Yeni native ve Chromium kabul testleri 2026-09–2027-08 arasında 12 ardışık 15–14 dönemi aynı DB/IndexedDB zincirinde hesaplar; 2026-09-15 baseline, 2027-03-01 ve 2027-09-01 değişimleri, vergi opening'leri, boş `zamAylari`, finalized bordrolar ve sınır dönemlerindeki tam segment/tarih/version ID'lerini doğrular. |
| V5 restore ve baseline | Yeni native ve Chromium testleri V5 backup'ı gerçek restore yolundan yükler ve boş policy timeline'ını doğrular. Baseline öncesi policy hesabı fail-closed olur ve payroll ledger değişmeden kalır; ardından baseline CompensationPolicyService/API preview+save yoluyla kaydedilir ve aynı dönem hesabı kaydedilen version ID'siyle başarılı olur. |
| Dönem silme ve policy geçmişi | Yeni native kabul testi `PeriodService::delete_period` öncesi/sonrası tüm ham SQLite policy satırlarını birebir karşılaştırır. Chromium kabul testi bağımsız 2028-01 dönemini WASM mutation impact'iyle siler; silme öncesi/sonrası ve IndexedDB readback sonrası tam `compensationPolicyVersions` JSON dizisinin eşit olduğunu doğrular. |

2027 yıllık/statutory değerleri bu senaryoda tarih ve engine yolu için sentetik
test girdileridir; resmî tarife olarak sunulmaz. Testler izole SQLite/fixture
kullanır; canlı veritabanı açılmadı.

## Doğrulama

Bu takip denetiminde yalnız ilgili native, browser, WASM freshness ve silme
testleri çalıştırıldı; tüm repository suite'leri tekrar edilmedi.

### Önceki tam regresyon turu (bu takipte tekrar edilmedi)

| Komut | Sonuç |
|---|---|
| `cargo test -p payroll-core` | PASS — 76 unit testi ve tüm core integration/doc-test hedefleri. |
| `bun run test:rust` | PASS — Tauri lib, migration/backup, native API/persistence ve regression hedefleri. |
| `bun run wasm:test` | PASS — 9 WASM testi. |
| `bun test` | PASS — 375 test, 1.673 assertion, 46 dosya; gerçek Chromium/IndexedDB testleri dahil. |
| `bun run verify:wasm-package` | PASS. |
| `bun run verify:payroll-contract` | PASS — Rust contract ve generated TS senkron. |
| `bun run verify:production-graph` | PASS — release graph `PayrollEngine` kullanıyor. |
| `bun run build` | PASS — Vite production build tamamlandı. Vite mevcut büyük JS chunk uyarısı verdi. |

### Bu takip denetimindeki hedefli doğrulama

| Komut | Sonuç |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --test compensation_policy_entry_test` | PASS — 2 native scoped calculation/finalization testleri; 2026-09, 2027-03, 2027-09 tek-dönem senaryoları. |
| `cargo test --manifest-path src-tauri/Cargo.toml --test compensation_policy_entry_test native_2026_2027_acceptance_roundtrips_v6_v5_and_policy_timeline` | PASS — yeni 12 dönem native acceptance testi; V6 fresh SQLite restore, V5 baseline gate, post-restore normal+retro ve ham timeline silme karşılaştırması. |
| `cargo test --manifest-path src-tauri/Cargo.toml --test compensation_policy_api_test` | PASS — 9 native policy API testi; baseline preview/save dahil. |
| `cargo test --manifest-path src-tauri/Cargo.toml --test migration_legacy_serde_parity_test` | PASS — 28 native V5/V6 restore/serde testi; shared V6 calculated/finalized readback dahil. |
| `bun test src/services/compensationPolicy/api.browser.test.ts` | PASS — 4 gerçek Chromium/IndexedDB testi, 27 assertion. Sandboxed ilk deneme Chromium beforeAll aşamasında 60 saniyede timeout oldu; aynı komut sandbox dışı yürütmede geçti. |
| `~/.bun/bin/bun test src/services/compensationPolicy/api.browser.test.ts -t '12 periods|V5 backup'` | PASS — 2 yeni Chromium/WASM kabul testi, 29 assertion; V6 sentinel restore, fresh page/runtime, post-restore normal+retro, V5 fail-closed/baseline ve exact timeline readback dahil. |
| `bun test src/services/storage/browserPayrollStore.test.ts -t 'V5'` | PASS — 2 V5 browser-store testi, 18 assertion. |
| `bun test src/services/storage/periodDeletion.test.ts` | PASS — 4 deletion testi, 19 assertion; immutable policy timeline eşitliği assertion'ı yok. |
| `bun run verify:wasm-freshness` | PASS — `1458744f75a7413a9f255951ae8fd230334a8b5e99765bf3ea7d966e2df0a4bb`. |
| `node_modules/.bin/tsc --noEmit` | FAIL — repository-wide existing test matcher typings (`bun:test`) ve `PuantajGrid.tsx` tanılamaları var. Değişen `api.browser.test.ts` ve `apiBrowserTestHarness.ts` için tanılama filtresi boş döndü. |

Önceki tam `bun test` turu ve bu denetimdeki Chromium testi macOS Chromium'u
sandbox içinde başlatamadığı için sandbox dışı yürütüldü; her iki çalıştırma da
geçti. `cargo fmt --all -- --check` çalışma ağacındaki geniş Rust biçim
farkları nedeniyle temiz değil; toplu format uygulanmadı. Handoff için sondaki
boşluk taraması sonuç vermedi; `git diff --check` tracked diff için PASS.

## 8A uygulama dosya grupları

- Core replay, tam validator ve regresyon/2026–2027 acceptance: `crates/payroll-core/src/payroll_snapshot.rs`, `crates/payroll-core/src/payroll_engine.rs`, `crates/payroll-core/src/payroll_engine/snapshot_replay.rs`, `crates/payroll-core/src/payroll_engine/compensation_shadow.rs`, core snapshot/retro testleri ve V6 fixture.
- WASM/native/browser save/restore sınırları: `crates/payroll-wasm/src/lib.rs`, `src-tauri/src/repositories/payroll_repo.rs`, `src-tauri/src/services/migration_service.rs`, `src-tauri/src/services/compensation_policy_service.rs`, browser payroll payload/store hook ve ilgili testler. Generated WASM paketi kaynak değişiminden sonra yeniden üretildi.
- Test fikstürü doğruluğu: native 15–14/retro akışında stale event önce canonical replay ile çözülür; eski migration testleri gerçek eski `user_version` şemalarında kurulur; yemek istisnası regresyonları test ettikleri politikayı açıkça yazar; latest migration beklentisi migration sabitinden alınır.
- Bu takipte değiştirilen dosyalar: `src-tauri/tests/compensation_policy_entry_test.rs`, `src/services/compensationPolicy/api.browser.test.ts`, `src/services/compensationPolicy/apiBrowserTestHarness.ts` ve bu handoff.

## Sonraki kapı

8A replay uygulaması ve native/browser kabul kanıtı tamamlandı. Task 8B temizliği,
route switch'i ve operasyonel aktivasyon ayrı kapsamdır; production activation
checklist'i ayrıca kapanmalıdır. Bu handoff release activation izni vermez.
