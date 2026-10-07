# Görev 7 — Ücret Politikası UI handoff'u

## Durum ve dosya sınırı

Görev 7 UI, frozen `CompensationPolicyPageProps` sözleşmesini kullanan named `CompensationPolicyPage` export'u, form ve exact-Decimal yardımcılarıyla tamamlandı. Bu uygulama `App.tsx`, controller/hook, bridge, API, storage veya frozen contract dosyalarını değiştirmedi. Güncel paylaşılan ağaçta App rotası Task 5B kodu tarafından sayfaya bağlanmıştır; gerçek API/persistence uçtan uca doğrulaması bu UI testlerinin kapsamında değildir.

Görev 7 dosyaları:

- `src/components/Settings/CompensationPolicyPage.tsx`
- `src/components/Settings/CompensationPolicyForm.tsx`
- `src/components/Settings/compensationPolicyDecimal.ts`
- `src/components/Settings/CompensationPolicyPage.test.tsx`
- `src/components/Settings/compensationPolicyDecimal.test.ts`
- `src/components/Settings/CompensationPolicyPageTestHarness.tsx` — yalnız rendered UI test harness'ı
- `docs/compensation-task-7-handoff.md`

## UI akışı ve güvenlik davranışı

- Yalnız yetkili, aynı hizmet dönemi kapsamındaki yük gösterilir. Scoped sürümler, as-of projection ve çözümlenmiş dönem dilimleri ayrı anlatılır; görünümün tam kurum geçmişi olmadığı belirtilir. Quarantine ve load hataları yeniden yükleme eylemiyle gösterilir.
- Aktif dönem ayarları yalnız kullanıcı isteğiyle boş taslağa kopyalanır ve authoritative olmadıkları etiketlenir. Eksik değerler boş kalır; otomatik baseline veya parasal varsayılan üretilmez.
- Form bütün ücret alanlarını, iş primi grup kimliği/ad/oran/aktif bilgisini, işlem türünü, effective date'i, gerekçeyi ve iki yuvarlama seçimini taşır. Her sürüm tam state'tir. `ROAD_ONLY` için yol dışındaki state'in korunması gerektiği açıklanır; baseline'da da seçilebilir.
- İlk baseline için tarih + tam operation fingerprint'ine bağlı açık onay ve iptal gerekir. Tarih veya herhangi bir draft alanı değişince onay düşer. Aynı tarih düzeltmesi yalnız tek scoped etkin leaf seçildiğinde hazırlanır ve `supersedesVersionId` ile preview'e gider.
- Decimal metinleri BigInt ile exact çözülür; wire'a yalnız doğrulanmış canonical string/null gider. UI'da ondalık virgül kullanılır. Yüzde yardımcısı taban ücret ve yemeği immutable biçimde artırır; diğer state ve gruplar korunur. Yuvarlama, replay kuralı veya delta saklamaz.
- Preview tam operation ve scope ile eşleşmediğinde stale sayılır. Ücret state penceresi, kazanç yuvarlama penceresi, doğrudan etkilenen dönemler ve GV/PEK/ödeme/devreden bağımlılık kapanışı ayrı gösterilir. `FINALIZED` engelleri kaydetmeyi kapatır.
- Duplicate save engellenir. Conflict, write hatası, persisted fakat authoritative reload başarısızlığı ve reload sonrası başarı farklı durumlar olarak gösterilir. Başarı yalnız `succeeded` state'inde ve aynı operation için görünür.
- Alan hataları ilgili input'a bağlanır; form hatasında özet odaklanır. Yükleme, kayıt, stale preview ve başarı durumları semantik status/alert rolleriyle duyurulur. Tailwind ile 1440px Chromium rendered review'da başlık, scoped timeline, baseline uyarısı, bölümlenmiş form ve alt eylemler gözden geçirildi.

## Doğrulama kanıtı

| Komut | Sonuç |
|---|---|
| `bun test src/components/Settings/compensationPolicyDecimal.test.ts` | PASS — 11 test, 65 assertion. Decimal sınırı/roundtrip, null–zero, invalid full-state, baseline ve ROAD_ONLY operation, percentage, same-date leaf. |
| `bun test src/components/Settings/CompensationPolicyPage.test.tsx` | PASS — 7 rendered Chromium testi, 51 assertion. Mock frozen props/callbacks ile baseline confirm/cancel, form/preview/save, ROAD_ONLY, geçersiz correction leaf ve stale preview, blocker, conflict, persistence/reload durumları. Test harness gerçek API, DB veya kullanıcı verisine erişmez. |
| `bunx tsc --noEmit --pretty false` | EXIT 2 — toplam 66 repo tanısı; Task 5A handoff'undaki baseline da 66'dır. Task 7 component/helper/harness ve politika API dosyalarında tanı yoktur. Tanılar ilgisiz mevcut test modülleri ile `PuantajGrid.tsx` dosyasındadır. |

Production build, Rust/WASM/payroll hesaplama veya gerçek persistence testi bu görevde çalıştırılmadı.

## Entegrasyon notu

Güncel paylaşılan `src/App.tsx` içinde politika sayfası Task 5B controller'ı üzerinden route edilmiştir. Bu handoff, UI prop-contract davranışını ve rendered kullanıcı akışını doğrular; 5B API'nin native/browser persistence, transaction rollback, lisans kapısı veya gerçek authoritative reload kanıtı yerine geçmez. Production activation ve gerçek senaryo kanıtları migration görev haritasındaki sonraki kapılarda kalır.
