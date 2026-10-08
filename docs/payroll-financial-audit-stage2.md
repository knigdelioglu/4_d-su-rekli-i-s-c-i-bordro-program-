# 4/D Bordro Programı — Aşama 2 Finansal Denetim Raporu

**Denetim tarihi:** 8 Ekim 2026
**İncelenen UAT:** Şubat 2027–Eylül 2028, 20 sentetik çalışan
**Kapsam:** Bordro çekirdeği, native veri sınırları, oracle/regresyon testleri ve önceki Codex UAT çıktıları.

## Sonuç ve kapsam ilkesi

Bu denetim, uygulamanın kendi değerlerini beklenen sonuç kabul etmez. Bağımsız sonuçlar; tam kuruş girdileri, ayrı integer-rational yuvarlama, artan oranlı vergi dilimlerini parça parça toplama, SGK/işsizlik oranlarını PEK sınırlarıyla uygulama ve elle hazırlanmış golden çalışma kâğıtları üzerinden üretilmiştir. Oracle modülleri production hesap fonksiyonlarını çağırmaz. Uygulama çıktıları yalnız karşılaştırma tarafında kullanılmıştır.

2026 mevzuat parametreleri resmi kaynaklarla karşılaştırılmıştır. 2027–2028 test vergi dilimleri, taban/tavan, ücret artışları ve diğer tutarlar **sentetiktir**; bu tarihler için resmî veya sözleşmesel oran doğrulaması iddia edilmez. Sonuçlar iki ayrı soruyu cevaplar: (1) belirtilen parametrelerle matematik doğru mu, (2) parametrenin kendisi mevzuat/sözleşme bakımından doğrulandı mı.

## Kural ve parametre dayanağı

- Türk lirası hesap sonuçlarının kuruşa yuvarlanması için 5083 sayılı Kanun’un 2. maddesinin 5228 sayılı Kanun’la değiştirilen metni, sonuç ve ödeme aşamasında yarım kuruş ve üzerini bir kuruşa tamamlama kuralını içerir. Production tutar yuvarlaması bu nedenle `HALF_UP`/işaret yönünde yarım değerden uzağa olacak şekilde yapılır. [TBMM, 5228 sayılı Kanun metni, 5083 değişikliği](https://www5.tbmm.gov.tr/tutanaklar/KANUNLAR_KARARLAR/kanuntbmmc088/kanuntbmmc088/kanuntbmmc08805228.pdf)
- 2026 ücret gelirleri GV dilimleri GİB’nin 2026 tarifesinden alınan bağımsız snapshot ile karşılaştırılır. [GİB, 2026 Gelir Vergisi Tarifesi](https://cdn.gib.gov.tr/api/gibportal-file/file/getFileResources?objectKey=arsiv%2Fyardim-kaynaklar%2Fyararli-bilgiler%2Fgelir-vergisi-tarifeleri%2Fgelir-vergisi-tarifesi-2026.pdf)
- 2026 PEK alt/üst sınırları, yemek istisnası ve ücret dışı ödeme/sonraki iki aya taşıma açıklaması SGK’nın 2026 sayfasındaki sınırlar ve 5510 sayılı Kanun 80/82 anlatımıyla karşılaştırılır. Bu kaynak, özel durumdaki ücret dışı ödemeleri kendi başına sınıflandırmaz; kalemin ücret niteliği ve sözleşme dayanağı ayrıca gerekir. [SGK, Prime Esas Kazanç Miktarları 2026](https://www.sgk.gov.tr/Content/Post/2e0c9e1a-2cfe-4456-af10-49d3de0c58ba/Prime-Esas-Kazanc-Miktarlari-2026-01-14-10-35-39)
- Asgari ücret GV istisnası ile 2026 GV/DV uygulaması, GİB ücret rehberi ve mevcut `statutory/2026.json` bağımsız snapshot’ı üzerinden değerlendirilmiştir. [GİB, Ücret Geliri 2026](https://intvrg.gib.gov.tr/hazirbeyan/assets/pdf/DUYURU_UNIVERSAL_2026_2026_Ucret_Geliri.pdf)
- Yemek ve yol kalemlerinin vergi/SGK niteliği, ilgili kalem ve tarih parametreleriyle sınırlı değerlendirilmiştir. SGK’nın 2026 kaynağı günlük yemek PEK istisnasını verir; 2027–2028 tutarları uygulama varsayımlarıdır.

## Doğrulanmış finansal hatalar ve düzeltmeler

| Önem | Bulgu ve kök neden | Düzeltme | Bağımsız regresyon kanıtı |
|---|---|---|---|
| P2 | Para tutarlarında `Decimal::round_dp(2)` Rust varsayılanı olan en yakına/çifte yuvarlamayı kullanıyordu. Yarım kuruşlarda kanuni ödeme yuvarlamasıyla uyuşmuyordu. | Ortak `round_currency_amount` `MidpointAwayFromZero` olarak tanımlandı ve payroll-core/native parasal ara/sonuç noktalarına taşındı. Oran yüzdesi gibi parasal olmayan iki basamaklı değerler ayrı bırakıldı. | `tax_properties::stamp_tax_half_kurus_matches_integer_rational_oracle`: 33.500 × 0,00759 = 254,265 → 254,27; istisna sonrası 3,57. Eski sonuç 254,26 / 3,56 olurdu. `work_premium_half_kurus_matches_integer_rational_oracle`: 1.000,01 × %50 = 500,005 → 500,01 (eski 500,00). Integer-rational oracle production yuvarlama API’sini çağırmaz. |
| P2 | Yemek istisnasında aynı ücret parçası yalnızca mevzuat yürürlük tarihine göre segmentlere ayrıldığında her segment ayrı yuvarlanarak toplam istisnayı bir kuruş artırabiliyordu. | Gerçek ücret/kazanç parçaları ayrı hesaplanır; her parçanın tarih içindeki yasal kesişimleri biriktirilir ve o ücret parçası için yalnız bir kez kuruşa yuvarlanır. | `tax_properties::statutory_meal_date_split_does_not_round_one_payment_twice`: 10,005 + 10,005 aynı ödeme olup tarih sınırıyla ikiye ayrıldığında beklenen 20,01. Bağımsız oracle aynı tarih kesişimlerini ayrı bir matematikle toplar. |
| P2 | Puantaj kaydı haritasında tarih formatı ve dönem dışı gün denetleniyordu; fakat eksik gün anahtarları repository tarafından kabul edilebiliyordu. Böyle bir kısmi harita draft/snapshot akışına girebilirdi. | Repository, bütün dönem günlerinin kanonik `YYYY-MM-DD` anahtarlarıyla tam bulunmasını zorunlu kılar; eksik tarih açık validation hatası üretir. Mevcut fail-closed finalization koruması korunur. | Native testler eksik tarih ve kanonik olmayan anahtarları reddeder; tam 28/29/30/31 günlük 15–14 dönemleri kabul edilir. 13/13 `attendance_period_invariant_regression_test` geçti. Mevcut UAT DB’de incelenen 415 puantaj satırında eksik gün bulunmadı. |

`cargo test --workspace` sırasında tam puantaj ön koşulu ortaya çıkınca kısmi fixture’lar, amaçlanan eksik/raporlu günleri açıkça `R` olarak yazarak tamamlandı. Test verisi uydurularak production validasyonu gevşetilmedi.

## UAT Kasım–Aralık 2027 kesinti farkı

Önceki UAT raporundaki çalışan kesintileri Kasımda **897.943,43 TL**, Aralıkta **599.384,80 TL** idi; görünen düşüş **298.558,63 TL**. Bağımsız yeniden oynatma UAT’deki satır girdilerini, kaydedilmiş kümülatif değerleri ve o senaryoda tanımlanmış sentetik dilimleri kullanır. Hesap sonucunu beklenen kabul etmez.

| Kalem, 20 çalışan toplamı | Kasım 2027 | Aralık 2027 | Aralık − Kasım |
|---|---:|---:|---:|
| Brüt | 2.316.524,73 | 2.394.701,37 | +78.176,64 |
| GV matrahı | 1.841.291,49 | 1.902.641,62 | +61.350,13 |
| Brüt GV | 736.516,61 | 285.396,27 | −451.120,34 |
| Asgari ücret GV istisnası | 224.604,00 | 84.226,60 | −140.377,40 |
| Kesilen GV | 511.912,61 | 201.169,67 | −310.742,94 |
| GV dışı çalışan kesintileri | 386.030,82 | 398.215,13 | +12.184,31 |
| Toplam çalışan kesintisi | 897.943,43 | 599.384,80 | **−298.558,63** |
| Net ödeme | 1.418.581,30 | 1.795.316,57 | +376.735,27 |

Toplam seviyesinde fark aritmetik olarak kapanır: GV’deki **310.742,94 TL azalış**, rapordaki GV dışı çalışan kesintileri toplamındaki **12.184,31 TL artışla** netleşerek kesintilerde **298.558,63 TL düşüş** verir. Brüt artışı 78.176,64 TL ile net artışı da (brüt artışı + kesinti düşüşü) 376.735,27 TL olur. Ancak UAT dosyasında GV dışı kesintilerin SGK, işsizlik, sendika, BES/OKS ve diğer türlere göre dökümü bulunmadığından bu 12.184,31 TL artış alt kalemlere bağımsız biçimde paylaştırılamadı; 299 bin TL anomalisi bu nedenle tam kalem mutabakatı sayılmaz.

Vergi düşüşünün yönü, UAT’nin hizmet dönemi etiketleri ve vergi ödeme ayı kuralıyla uyumludur: `2027-11` hizmeti 15 Kasım–14 Aralık olup 2027 vergi ayı 12’ye; `2027-12` hizmeti 15 Aralık–14 Ocak olup 2028 vergi ayı 1’e bağlanmıştır. Sentetik dilimlerde Kasım önceki kümülatif matrah 721.430,08 TL ile %40 marjinal dilimdedir; Ocak yeni vergi yılı olduğu için GV ve asgari ücret istisnası kümülatifleri sıfırdan başlar ve ilk %15 diliminden hesaplanır. Bağımsız örnek satır testinde Kasım/Aralık sırasıyla GV **25.958,69 / 10.195,82 TL** çıkar ve uygulama sonuçlarıyla kuruşuna kadar eşleşir (`synthetic_uat_payment_month_boundary_resets_cumulative_gv`). UAT’deki toplam 40 vergi satırı PEK, çalışan SGK, işsizlik veya GV dışı kesintilerin bağımsız yeniden oynatılmasına yetecek kişi/kalem girdilerini içermediğinden bu bileşenler hakkında mismatch yokluğu iddia edilmez.

Bu karşılaştırmanın 40 gözlenen GV satırı anonim kuruş girdileri ve uygulama çıktısı karşılaştırmaları olarak `crates/payroll-core/tests/fixtures/uat_2027_tax_boundary_reference.json` içine kaydedildi. Fixture her satır için GV matrahını, önceki kümülatifleri, asgari ücret referansını ve gözlenen vergi alanlarını içerir. `tax_properties::synthetic_uat_november_december_cohort_matches_independent_tax_oracle` her satırın vergi sonucunu bağımsız parça-parça oracle ile yeniden hesaplar; beklenen değer oracle’dan gelir, fixture’daki uygulama alanı yalnızca ölçülen sonuçtur. Testin bağımsız brüt GV/istisna/kesilen GV toplamları Kasım için **736.516,61 / 224.604,00 / 511.912,61 TL**, Aralık için **285.396,27 / 84.226,60 / 201.169,67 TL** olur. UAT raporu per-person brüt, PEK, çalışan/işveren SGK-işsizlik ve kesinti türü dökümünü taşımadığından bu fixture Kasım/Aralık PEK veya GV dışı kesintileri yeniden hesapladığını iddia etmez.

Bu açıklama vergi parametrelerinin resmî 2027/28 olduğu anlamına gelmez. Ayrıca Ekim 2027 tüm 20 kişi için yuvarlama-politika hatasıyla bloke olduğundan UAT’nin 2027 kümülatif tarihçesi kesintisiz değildir. Kasım/Aralık karşılaştırması kendi kayıtlı vergi kümülatiflerinin aritmetiğini doğrular; eksik Ekim bordrosunu veya yıllık tam vergi zincirini onaylamaz. UAT’de 2028 Şubat ücret artışında yol tutarı ve yuvarlama modu değişimi gözlendi, Ekim 2027 ve Ocak 2028 hesaplamaları tamamlanamadı.

## Kategori kapsamı ve durumu

`PASSED` yalnızca tabloda yazılı oracle/regresyon örneklerinin geçtiğini belirtir; modülün her hukuki/sözleşmesel varyantı için genel uygunluk beyanı değildir.

| Kategori | Durum | Bağımsız kanıt ve açık sınır |
|---|---|---|
| Brüt, günlük ücret, kısmi ay, çalışma günü | Kısmi | Core goldens ve normal-payroll oracle taban/gün/kazanç matematiğini sınar; değişik hizmet günü ve farklı gerçek ücret parçalarının her kombinasyonu exhaustive değildir. Native attendance invariant tam tarih aralığını ayrıca sınar. |
| Çalışan/işveren SGK ve işsizlik | Kısmi | Oracle oran × PEK, cap ve toplam katkıyı test eder; 2026 parametre snapshot’ı resmi değerlerle karşılaştırılır. Teşvik/işveren indirimi ve tüm sentetik işveren oranları için dış SGK tahakkuku yoktur. |
| PEK alt/üst sınırı, istisna ve devreden PEK | Kısmi | Bağımsız PEK oracle ve 2026 sınır golden’ları, alt/üst sınır, carry kapasitesi, iki ay ömrü ve aynı ödeme ayı event’lerini kapsar. Bazı sıfır gün/ödeme kalemi sınıflandırmaları sözleşme yorumuna bağlıdır (aşağıya bakın). |
| GV matrahı, kümülatif GV, dilim, asgari ücret istisnası | Kısmi | Bağımsız parça parça vergi ve minimum istisna oracle’ları, yıllık tarifeler ve sınır değerleri; sentetik UAT Kasım/Ocak geçişi ayrı test edilir. Tüm 24 aylık kohortta UI’den yeniden yürütülen kesintisiz zincir yoktur. |
| Damga vergisi ve istisnası | Kısmi | Oran × matrah, asgari ücret istisnası ve half-kuruş regresyonu integer-rational oracle’la test edilir. Her ödeme türü ve dönem için dış vergi beyanı yoktur. |
| Ara işlem yuvarlama ve kuruş farkı | **PASSED (test kapsamı)** | Half-up tutar helper’ı, SGK/GV/DV/iş primi bağlantıları ve öğün tarih segmenti ayrımı için exact kuruş regression’ları var. İş kuralı yuvarlamanın hangi ara adımda uygulanacağını belirleyen yerde o sözleşme korunmuştur. |
| Zam, bileşik artış, yol/yemek ve iş primi | Kısmi | Ücret politika zaman çizelgesi, oracle ile iş primi/meal sınırı, mevcut migration/parity testleri ve UAT’nin görülen tarihlerdeki dört yol artışı var. 10/10 yol tarihi UAT’de kaydedilmemiş; 2027–28 oranları sentetik. |
| Sendika, BES/OKS, nafaka, icra, kişi borcu, avans, diğer kesinti ve çoklu sıra | Kısmi | Mevcut bağımsız goldens ve deduction oracle bazı sabit/oranlı kesintileri, GV matrahı ve net muhasebesini kapsar. UAT’de 24 profil tamamlanmadı; bütün kesinti türleri ve mahsup önceliklerinin tam, ortak bağımsız golden matrisi yoktur. |
| Puantaj kodları, 28/29/30/31, artık yıl ve hastalık raporu | Kısmi | 7 puantaj kodu, 15–14 dönem uzunlukları, leap-year ve sick quota/overlap testleri mevcuttur; yeni native gate tam tarih haritasını zorunlu kılar. UAT gerçek rapor kayıt modülünde ilk beş raporun iki günlük kurum kuralı, altıncı rapor, bölünme/düzeltme/silme akışını çalıştırmadı. |
| Tediye, TİS, ek tahakkuk sırası | Kısmi | Golden ve event zinciri testleri aynı ay PEK/vergi istisnası taşımasını; UAT bir Tediye ve bir TİS’i hesapladı. Tüm farklı tahakkuk tarih/sıraları ve UI’de ek ödeme kapsamı tamamlanmadı. |
| Retro pozitif/negatif/sıfır, tekrar ve kesinleşmiş dönem | Kısmi | Retro oracle’ı kaynak dönem farkları, pozitif/negatif, sıfır tekrar, deterministic replay ve immutable baseline regression’larını test eder. Eksik normal source period artık 0 kabul edilmez, fail-closed testle korunur (`bug_retro_001_test_b_missing_normal_inside_window_fails_closed`, `native_bug_retro_001_missing_normal_source_period_is_rejected_not_zero`). UAT UI’de pozitif örnek dışında negatif/sıfır tüm akışları tamamlamadı. |
| Dönemler arası net banka/SGK/kesinti mutabakatı | **NOT VERIFIED** | UAT Eylül 2028’de 20 normal + bir retro banka XLSX’ini 1.510.835,21 TL olarak açtı; program SGK listesinde 980.439,02 TL ve uygulama girildiğinde sıfır fark gösterdi. Bu, dış banka dosyası/SGK tahakkuk fişiyle bağımsız mutabakat değildir. 20 dönem için tüm dışa aktarımlar satır bazında eşleştirilmedi. |

## Açık ve çözülemeyen mali/iş kuralı farkları

1. **0 çalışma günü ve ücret kalemlerinin PEK sınıfı — UNRESOLVED, P1 aday.** UAT source audit’te 29 ücretsiz `R` günlü örnekte 4.025,00 TL brüt kalem bulunmasına karşın uygulama SGK/işsizlik primini 0 ve sonraki aya PEK carry’sini 200,00 gösteriyor; brüt kalemlerin kalan 3.825,00 TL PEK karşılığı açıklanamıyor. Bu, genel olarak bütün 0 gün ödemeler PEK’e tabidir diye düzeltilemez: SGK 2026 rehberi ücret dışı PEK’e tabi ödeme, prim günü/ücret bulunmayan ayda takip eden en fazla iki aya aktarılabilir diyor; ücret, TİS yardımı ve diğer kalemlerin niteliği ayrı sınıflandırılmalıdır. Örnek kalemlerin iş sözleşmesi/TİS ve parametre türü bu denetim paketinde yoktur. Kesin kök neden ve beklenen prim onaylanmadı; PASS değildir. [SGK 2026 PEK açıklaması](https://www.sgk.gov.tr/Content/Post/2e0c9e1a-2cfe-4456-af10-49d3de0c58ba/Prime-Esas-Kazanc-Miktarlari-2026-01-14-10-35-39)
2. **Dış kaynaklı banka/SGK teyidi — NOT AVAILABLE.** UAT’nin program tutarını yine programın karşılaştırma kutusuna girmesi bağımsız doğrulama değildir. Gerçek banka ödeme dosyası, SGK bildirgesi/tahakkuk fişi veya kurumun onayladığı ayrık referans yok.
3. **2027–2028 mevzuat uygunluğu — NOT VERIFIED.** Bu rakamlar sentetik test değerleri. Matematik o parametreler karşısında denetlenmiştir; resmî oran olarak sunulmamıştır.
4. **Kasım–Aralık çalışan kesintisi alt kalem mutabakatı — NOT VERIFIED.** UAT raporu yalnızca çalışan kesintilerinin dönem toplamını verir; 12.184,31 TL GV dışı artışın SGK, işsizlik, sendika, BES/OKS ve diğer kesintiler arasındaki bileşimi için satır/kalem verisi yoktur. Toplam farkın eşitliği doğrulanır, bileşimi doğrulanmaz.
5. **UAT akış bütünlüğü — PARTIAL.** 18/20 dönem ve 360/400 normal bordro hesabı tamamlandı; Ekim 2027/Ocak 2028 40 kayıt bloke oldu. 24 profilin tamamı, tüm rapor kuralları, ek ödeme ve tüm retro türlerinin arayüz akışı tamamlanmadı.

## Test ve CI durumu

Yerel doğrulamalar:

- `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` ve tam `cargo test --workspace`: geçti.
- `node scripts/build-wasm.mjs`, WASM package allowlist/freshness ve `node scripts/verify-web-build.mjs`: geçti. Release WASM, düzeltmeleri tarayıcı motoruna taşımak için yenilendi.
- TypeScript typecheck, production graph, Rust→TypeScript contract ve privacy boundary: geçti.
- Vite production build ve Playwright tarayıcı E2E: geçti; 34/34 senaryo. Netlify runtime smoke: geçti; CSP, WASM yükleme, IndexedDB ve hesaplama kontrolleri.
- Bu macOS makinede Bun executable bulunmadığından `bun run test:ci` yerel olarak başlatılamadı; ayrıca yerel `tauri-driver` yok. GitHub Actions’ın TypeScript Bun test/coverage, WASM adapter ve Linux/Windows native WebView adımları bu nedenle teslim kapısıdır.
- `cargo build --manifest-path src-tauri/Cargo.toml --bin bordro-programi`: geçti.
- `PROPTEST_RNG_SEED=2026091001 cargo llvm-cov --package payroll-core --package bordro-programi --all-features --json --summary-only --output-path target/payroll-coverage.json`: tüm Rust testleri geçti. `node scripts/check-rust-coverage.mjs target/payroll-coverage.json docs/payroll-coverage-baseline.json`: 84 kritik dosya metriğinin hiçbiri baseline altında değil (PASS).

GitHub Actions durumu push ve run tamamlandıktan sonra eklenecektir. Workflow yeşil değilse teslim “tamamlandı” sayılmaz. Bu rapor tüm mevzuat/sözleşme varyantlarının PASS olduğu iddiası değildir.
