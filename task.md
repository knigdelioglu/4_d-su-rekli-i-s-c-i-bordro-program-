# STATUS: DONE

Kod uygulandı ve test/doğrulama komutları başarıyla çalıştırıldı.
Commit/push yapılmadı. Başlangıçta working tree temizdi.

## 1. Eski problem
Rust resolve_accrual_input / validate_supplementary_after_normal; UI form açılışı,
disabled state ve tooltip NORMAL-first kuralını dayatıyordu. Sequence tipe bağlıydı.

## 2. Yeni payment-event modeli
Canonical taxYear/taxMonth → paymentDate → sequence → accrualId.
Core input/payroll order constructor; mutation policy ve native GV/liste tüketicileri
aynı sıralamayı kullanır. UI comparator bunun presentation karşılığıdır.
Aynı-ay prior event'ler, yıllık GV ve ilgili PEK zinciri authoritative olmalıdır.
CALCULATED/FINALIZED kabul edilir; gerekli DRAFT/STALE state sessizce atlanmaz.

## 3. GV Exemption modülü
GvExemptionState::resolve(monthly_entitlement, used_before, gross_tax).
monthly_entitlement, used_before, remaining_before, applied_current,
remaining_after ve withheld_income_tax üretir. Decimal/GV half-up korunur.
Policy tahakkuk tipini, veritabanını ve mutable bakiye tablosunu bilmez.
Gerçek GV kümülatifi ile asgari ücret takvim referansı ayrıdır.

## 4. Normal-first kaldırılan noktalar
Core normal varlığı/tarih/sıra kontrolleri ile UI form, buton ve açıklama engelleri.
Puantaj ve statutory preflight sözleşmeleri korunur. NORMAL gelir üretimi korunur.

## 5. Sequence değişikliği
Tüm türlerde sequence>=0. Aynı personel/vergi ayı/ödeme tarihi/sequence benzersizdir.
UI ve örtük NORMAL çağrısı mevcut aynı-tarih event'lerinin sonrasını seçer.
Eski metadata ve legacy NORMAL içi manuel gelirler dönüştürülmedi.

## 6. Downstream invalidation
Backdated insert ve recalculation mevcut ortak policy'yi kullanır.
Yeni tek-event delete komutu SQLite transaction içinde silme+STALE uygular.
Browser aynı Rust/WASM policy impact'ini uygular. Mutable DRAFT/CALCULATED STALE olur.

## 7. FINALIZED koruması
FINALIZED kaydı silmek veya downstream FINALIZED state'ini etkileyen
insert/recalculate/delete reddedilir. Silme testi bütün persisted kayıtların
serialization'ını mutation öncesi/sonrası karşılaştırır.

## 8. GV/DV/PEK state doğrulaması
Aynı-ay snapshot toplamlarından GV/DV kullanılan istisna ve PEK kapasitesi devam eder.
PEK devri son kronolojik önceki event'ten gelir; ay geçişi ilk event'in türünden
bağımsızdır. Aynı ay aging tekrar edilmez. Geç NORMAL'in işveren alt sınır farkı
MTD PEK düşülerek tamamlanır; işçi matrahına eklenmez.
Bu davranışlar için regression kaynakları eklendi; testler çalıştırılmadı.

## 9. Değiştirilen dosyalar

| Dosya | Değişiklik |
|---|---|
| [.hermes/INVARIANTS.md](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/.hermes/INVARIANTS.md>) | Payment-event, sequence, shared state, invalidation ve PEK alt sınır kuralları. |
| [crates/payroll-core/src/lib.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/crates/payroll-core/src/lib.rs>) | Pure GV policy modülünün kaydı. |
| [crates/payroll-core/src/gv_exemption.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/crates/payroll-core/src/gv_exemption.rs>) | GvExemptionState ve 5.615,10 Ağustos regression senaryosu. |
| [crates/payroll-core/src/payroll_engine.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/crates/payroll-core/src/payroll_engine.rs>) | NORMAL-first kaldırıldı; canonical order, sequence allocation, kronolojik PEK state. |
| [crates/payroll-core/src/policies.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/crates/payroll-core/src/policies.rs>) | Ortak core sırası ve ACCRUAL_DELETE mutation policy. |
| [crates/payroll-core/tests/payroll_engine_regression.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/crates/payroll-core/tests/payroll_engine_regression.rs>) | Bağımsız event, tam normal gelirleri, paylaşılan snapshot, sequence ve aging testleri. |
| [crates/payroll-wasm/src/lib.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/crates/payroll-wasm/src/lib.rs>) | Bağımsız event ve delete-policy native/WASM parity testi. |
| [src-tauri/src/commands/payroll_cmd.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src-tauri/src/commands/payroll_cmd.rs>) | Tek tahakkuk silme IPC komutu. |
| [src-tauri/src/domain/calculations.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src-tauri/src/domain/calculations.rs>) | GV policy bağlantısı; PEK ay geçişi ve MTD işveren alt sınır tamamlaması. |
| [src-tauri/src/lib.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src-tauri/src/lib.rs>) | Silme komutunun kaydı. |
| [src-tauri/src/repositories/payroll_invalidation_repo.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src-tauri/src/repositories/payroll_invalidation_repo.rs>) | CALCULATED/DRAFT downstream STALE güncellemesi. |
| [src-tauri/src/repositories/payroll_repo.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src-tauri/src/repositories/payroll_repo.rs>) | Canonical liste sırası ve atomik korumalı silme. |
| [src-tauri/src/services/cumulative_tax_service.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src-tauri/src/services/cumulative_tax_service.rs>) | Native same-month sırası ortak core resolver kullanıyor. |
| [src-tauri/tests/multi_accrual_regression_test.rs](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src-tauri/tests/multi_accrual_regression_test.rs>) | Ağustos SQLite insertion/delete/finalized snapshot ve alt sınır testleri. |
| [src/App.tsx](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/App.tsx>) | Browser/native tahakkuk silme orchestration. |
| [src/components/BordroHesaplama.tsx](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/components/BordroHesaplama.tsx>) | NORMAL engelleri kaldırıldı; otomatik sequence, canonical liste ve silme aksiyonu. |
| [src/components/Listeler/accrualListData.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/components/Listeler/accrualListData.ts>) | Ortak presentation comparator. |
| [src/services/payrollEngine/paymentEventOrder.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/services/payrollEngine/paymentEventOrder.ts>) | UI sequence allocation ve canonical sıralamanın presentation karşılığı; bordro formülü içermez. |
| [src/services/payrollEngine/paymentEventOrder.test.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/services/payrollEngine/paymentEventOrder.test.ts>) | Aynı gün, farklı tür ve çalışma dönemi sequence testleri. |
| [src/services/payrollEngine/types.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/services/payrollEngine/types.ts>) | ACCRUAL_DELETE boundary tipi. |
| [src/services/storage/browserPayrollPolicies.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/services/storage/browserPayrollPolicies.ts>) | Mutable downstream STALE uygulaması. |
| [src/services/storage/browserPayrollPolicies.test.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/services/storage/browserPayrollPolicies.test.ts>) | Delete impact uygulaması ve snapshot korunması testi. |
| [src/services/storage/browserPayrollStore.test.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/services/storage/browserPayrollStore.test.ts>) | TEDIYE seq0 / NORMAL seq1 backup uyumluluk testi. |
| [src/services/tauriBridge.ts](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/services/tauriBridge.ts>) | Native silme çağrısı. |
| [src/wasm/pkg/payroll_wasm_bg.wasm](</Volumes/Lacie/MacBook Klasörleri/4_d-sürekli-i̇şçi-bordro-programı/src/wasm/pkg/payroll_wasm_bg.wasm>) | Güncel Rust core’dan üretilen production artifact. |

`task.md`: discovery, requirement/evidence ve bitiş kaydı (bu dosya).

## 10. Testler ve statik doğrulama

| Komut | Sonuç |
|---|---|
| cargo test --workspace | PASS; tüm workspace testleri geçti. |
| bun test | PASS; 158 test, 0 hata, 579 expect() çağrısı. |
| bun run wasm:test | NOT RUN — ayrı WASM browser harness gerektiriyor; native parity testleri workspace içinde geçti. |
| cargo check --workspace --tests | PASS. |
| bun run lint | PASS; TypeScript + production graph. |
| bun run wasm:build | PASS; production artifact yenilendi. |
| bun run build | PASS; Vite büyük chunk uyarısı var. |
| git diff --check | PASS. |

Çalıştırılan testler: Rust workspace testleri ve 158 Bun testi; başarısız test yok.

## 11. Regression senaryoları

| Senaryo | Kaynak / test |
|---|---|
| A, F, E | payroll_engine_regression: supplementary_is_independent_and_only_prior_events_must_be_authoritative |
| B | gv_exemption: august_three_payment_events_share_entitlement |
| C, D, E, K, L, M | payroll_engine_regression: payment_event_chain_shares_snapshots_and_preserves_full_normal_income |
| G, H, I, J, O | multi_accrual_regression_test: payment_event_backdated_insert_delete_and_finalized_protection |
| N | payroll_engine_regression: legacy_normal_then_tediye_and_default_normal_sequence_remain_supported; mevcut native multi-accrual testi |
| PEK aging | payroll_engine_regression: first_supplementary_event_advances_carry_month_once; mevcut devreden testleri |
| Native/WASM | payroll-wasm: independent_payment_events_and_delete_policy_match_native |
| UI sıra/backup/delete | paymentEventOrder.test.ts; browserPayrollStore.test.ts; browserPayrollPolicies.test.ts |

## 12. Kalan risk / teknik borç
WASM browser harness (`bun run wasm:test`) ayrıca çalıştırılmadı. UI runtime manuel
olarak denenmedi. Vite 500 kB üzeri chunk uyarısı devam ediyor.
Şema migration'ı veya mutable GV bakiye tablosu eklenmedi.
