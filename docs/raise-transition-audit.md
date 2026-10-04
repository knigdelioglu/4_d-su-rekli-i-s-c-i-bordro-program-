# Zam geçişi denetimi — 4 Ekim 2026

Hesaplama sahibi `crates/payroll-core/src/raise_transition.rs` modülüdür.
NORMAL bordro ve retro hesap aynı değişmez dönem ayarlarından aynı etkin
zamlı tutarları türetir; Tauri ve tarayıcı/WASM ortak Rust motorunu kullanır.
Veritabanı şeması ve kayıtlı ayarlar değiştirilmedi.

## Kural ve kanıt

| Gereksinim | Uygulama | Kanıt |
|---|---|---|
| Zam ayının 1'inde eski/yeni ücret ayrımı | Takvim tarihi ve puantaj kodu | `raise_transition_regression`: 28/29/30/31 gün, yıl geçişi |
| Günlük taban ve yemek hesabı | Eski/yeni hak günleri ayrı hesaplanır; yemek yalnız Ç/GÇ günlerine verilir | Aynı test dosyası; native ücretli rapor testi |
| Sendika aidatı zamlı günlük ücretin %65'i | NORMAL kesintiler güncel dönem ayarlarından hesaplanır; maaşın gün ağırlıklı ortalaması aidat matrahını değiştirmez | 28/29/30/31 gün ve yıl geçişi Rust testi; üyelik/sabit tutar; gerçek WASM vergi/net tutarlılık testi |
| Sosyal ve giyim yardımı | `(eski aylık tutar × eski takvim günü + yeni aylık tutar × yeni takvim günü) / dönem takvim günü` | Takvim matrisi ve elle girilen tutar testleri |
| Aynı bırakılan tutarlara ortak zam | Taban ücret değişiminden türetilen mevcut iki ondalıklı yüzde kuralı korunur | Var olan 7,08% testi ve yeni 10%/ardışık zam testleri |
| Zamlı tutarın sonraki döneme taşınması | Ham ayar eşitliği ile açık giriş ayrılır; etkin önceki tutar üzerinden devam edilir | İki sonraki dönem ve ardışık zam testleri; gerçek WASM JSON testi |
| Retro tutarlarda tutarlılık | Önce etkin tarihsel ayarlar çözülür, sonra mutlak revizyon uygulanır; aylık yardımlar toplamda bir kez yuvarlanır | Kuruş farkı, yalnız taban revizyonu, geçiş sonrası retro testleri |
| Ayar değişikliğinde eski sonuçların korunmaması | Zam gelir bağımlılığı çalışma tarihine göre vergi yılı sınırını aşar | Açık/kesinleşmiş bordro ve retro kaydı testleri; WASM koruma testi |
| Eksik geçmişte yanlış ücret kullanılmaması | Eksik önceki dönem/ayar, tarih boşluğu ve geçersiz zam ayı görünür hata verir | Rust ve WASM hata testleri |

15–14 döneminde eski ayın 15'i de dahildir. Eski ay 28/29/30/31 günse
sırasıyla 14/15/16/17 gün eski, yeni ayın ilk 14 günü yeni tutar uygulanır.
SGK'nın tam dönem için 30 günlük normalizasyonu ücretin takvim günlerinden ayrıdır.

## Düzeltilen sorunlar

- Yalnız geçiş döneminde türetilen zamlı tutarlar, kopyalanmış sonraki dönem
  ayarlarında eski tutarlara dönebiliyordu.
- Retro aylık yardım parçalarının ayrı yuvarlanması, değişmeyen hakta kuruş
  farkı oluşturabiliyordu.
- Mutlak retro taban ücret revizyonundan sonra tekrar zam türetmek,
  revizyonda belirtilmeyen kalemleri değiştirebiliyordu.
- Kaynak dönemin bordrosu olmadığında ayar bağımlılığı vergi yılı sınırında
  kesilebiliyor; sonraki bordro/retro sonucu güncel kalmış gibi görünebiliyordu.

Var olan iki test girdisi yeni kurala uyarlandı: zamlı yemek artık ücretle
artar; değişmeyen hak revizyonu testi yol tutarı üzerinden tüm gelirin
korunduğunu denetler. Ücretli rapor testinde yemek 100 + 200 = 300 TL olur;
raporlu günlere yemek verilmez.

## Kullanım sınırları

Zam ayı seçilmeli ve taban ücretin yeni tutarı girilmelidir. Önceki dönemle
aynı bırakılan yemek/yardım tutarları otomatik artar ve sonraki dönemlere taşınır.
Elle farklı girilen tutar açık giriş kabul edilir; bu tutarın aynı zam oranına
uygunluğu otomatik zorlanmaz. Mevcut hizmet zammı birimi davranışı da korunur.
Ayar geçmişi kesintisiz olmalıdır; en eski kayıt başlangıç tutarlarını sağlar.

Önceden hesaplanmış açık bordrolar yeni motorla yeniden hesaplanmalıdır.
Kesinleşmiş kayıtlar değiştirilmedi. Bu denetim kullanıcı veritabanında toplu
hesaplama veya kayıt değişikliği yapmaz.

## Doğrulama

- `cargo test -p payroll-core`: PASS, 282 test (10 yeni zam testi).
- Seçili native testler (`domain_tests`, `retro_native_regression_test`,
  `paid_sick_wage_regression_test`, `payroll_dependency_state_regression_test`):
  PASS, 104 test.
- `bun test`: PASS, 322 test; yeni dosyada gerçek production WASM ile 3 test.
- `bun run wasm:build`, `bun run build`, `bun run verify:wasm-package`: PASS.
- `cargo clippy -p payroll-core --all-targets -- -D warnings`: PASS.
- `bun run lint`: FAIL; mevcut `src/bun-test.d.ts` tanımında kullanılan
  `toContain`, `toBeNull`, `toMatchObject`, `rejects` vb. metotlar eksik.
  Zam modülüyle ilgisiz test tipi sorunu bu kapsamda değiştirilmedi.
- `bun run verify:production-graph` ve `git diff --check`: PASS.

## Bu denetimde değişen dosyalar

- `crates/payroll-core/src/raise_transition.rs` (yeni ortak modül)
- `crates/payroll-core/src/lib.rs`
- `crates/payroll-core/src/payroll_engine.rs`
- `crates/payroll-core/src/retro.rs`
- `crates/payroll-core/src/policies.rs`
- `crates/payroll-core/tests/raise_transition_regression.rs` (yeni)
- `crates/payroll-core/tests/independent_audit_20260908.rs`
- `src-tauri/tests/paid_sick_wage_regression_test.rs`
- `src/services/payrollEngine/raiseTransition.wasm.test.ts` (yeni)
- `src/components/Settings/IncomeParametersSection.tsx`
- `src/wasm/pkg/payroll_wasm_bg.wasm`
- `src/wasm/pkg/source-hash.txt`
- `docs/raise-transition-audit.md` (bu rapor)

Görev başında bulunan diğer çalışma ağacı değişiklikleri korunmuştur.

## Sendika aidatı düzeltmesi — kullanıcı kuralı

Kullanıcı, aidatın zamlı yeni günlük ücretin %65'i olacağını doğruladı.
Geçişte maaşın gün ağırlıklı ortalamasının kurum ayarlarındaki günlük taban
ücretin üzerine yazılması kaldırıldı. NORMAL kesinti hesabı artık güncel
dönem ücretini kullanır; eski/yeni maaş parçaları gelir hesabında korunur.
Oran ayarı ve pozitif sabit aidat öncelikleri korunur; üye olmayan çalışana
aidat kesilmez. GV matrahı ve net, aynı düzeltilmiş kesintiyi kullanır.

Örnek: 2.616,26 × %65 = **1.700,57 TL**. Yeniden kurulmuş Eylül
karşılaştırmasında maaş 78.163,40 TL olarak korunur, net 62.381,89 TL olur;
verilen bordroya göre fark +2,49 TL'dir. Yemek istisnası ve geçmiş matrah
varsayımları nedeniyle bu, gerçek çalışanın kesin neti onayı değildir.

- `cargo test -p payroll-core`: PASS, 283 test; eski ağırlıklı aidatı bekleyen
  `payroll_engine_quality_round2` doğrulaması yeni kurala güncellendi.
- `bun test`: PASS, 323 test; gerçek WASM zam testi dosyasında 4 test.
- `cargo clippy -p payroll-core --all-targets -- -D warnings`: PASS.
- `bun run wasm:build`, `bun run build`, `bun run verify:wasm-package`,
  `bun run verify:production-graph`: PASS.
- Gerçek WASM ile çalışan karşılaştırması: PASS; aidat, maaş ve net doğrulandı.
- Önceden hesaplanan bordrolar otomatik değiştirilmedi; yeni sonucu almak
  için açık bordrolar yeniden hesaplanmalıdır.

## Sonraki kullanıcı tercihi: yemek vergi hesabı

Ücret ekranına varsayılan işaretli, yemek yardımını tamamen GV'den istisna ve
 tamamen damgaya tabi tutan dönem seçeneği eklendi. Önceki günlük-sınırlı
GV/DV yöntemi seçim kaldırılınca uygulanır. Ayrıntılar ve güncel doğrulama
`docs/meal-payroll-tax-policy.md` dosyasındadır.
