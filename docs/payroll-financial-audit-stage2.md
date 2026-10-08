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
- Çalışanın hayat/şahıs sigortası indirimi için GİB’nin 2026 açıklamasındaki aylık ücretin %15’i ve yıllık brüt asgari ücret sınırı, hayat primi için %50, sağlık/diğer sayılan şahıs sigortaları için %100 kuralı esas alınmıştır. BES katkısı bu GV indirimine eklenmemiştir. UAT’deki 2027/2028 yıllık limit değerleri sentetik parametredir. [GİB, 2026 Hayat/Şahıs Sigorta Primleri](https://cdn.gib.gov.tr/api/gibportal-file/file/getFileResources?objectKey=arsiv%2Fyardim-kaynaklar%2Finfografikler%2Fpdfs%2F2026_hayatsahis_sigortaprim_info.pdf)
- Çalışan sendika aidatının GV matrahından indirimi, GİB rehberinin belge/aidat makbuzu şartı ile sınırlı değerlendirilebilir; UAT’de makbuz kanıtı olmadığından matematiksel kesinti doğrulandı, uygunluk kanıtı doğrulanmadı. [GİB, 2026 Beyanname Hazırlama Kılavuzu](https://intvrg.gib.gov.tr/hazirbeyan/assets/pdf/2026hbskilavuz.pdf)
- Yemek ve yol kalemlerinin vergi/SGK niteliği, ilgili kalem ve tarih parametreleriyle sınırlı değerlendirilmiştir. SGK’nın 2026 kaynağı günlük yemek PEK istisnasını verir; 2027–2028 tutarları uygulama varsayımlarıdır.

## Doğrulanmış finansal hatalar ve düzeltmeler

| Önem | Bulgu ve kök neden | Düzeltme | Bağımsız regresyon kanıtı |
|---|---|---|---|
| P2 | Para tutarlarında `Decimal::round_dp(2)` Rust varsayılanı olan en yakına/çifte yuvarlamayı kullanıyordu. Yarım kuruşlarda kanuni ödeme yuvarlamasıyla uyuşmuyordu. | Ortak `round_currency_amount` `MidpointAwayFromZero` olarak tanımlandı ve payroll-core/native parasal ara/sonuç noktalarına taşındı. Oran yüzdesi gibi parasal olmayan iki basamaklı değerler ayrı bırakıldı. | `tax_properties::stamp_tax_half_kurus_matches_integer_rational_oracle`: 33.500 × 0,00759 = 254,265 → 254,27; istisna sonrası 3,57. Eski sonuç 254,26 / 3,56 olurdu. `work_premium_half_kurus_matches_integer_rational_oracle`: 1.000,01 × %50 = 500,005 → 500,01 (eski 500,00). Integer-rational oracle production yuvarlama API’sini çağırmaz. |
| P2 | GV sigorta primi aylık %15 sınırının brüt ücret tabanı tüm yol yardımını ve yemek tutarının tamamını çıkarıyordu. Yol bu bordroda vergilendirilen nakdi ücrettir; yemek için de yalnızca GV'den gerçekten istisna edilen tutar dışarıda kalmalıdır. | Yeni tahakkuklar `compensation-engine-v2` ile `income_total − retro_income_tax_exempt − normal_meal_tax_exemption` tabanını kullanır. `compensation-engine-v1` snapshot'larının ve V5 yedek replay'inin tarihsel doğrulaması, eski sonuçları korumak için v1 formülünü sürümlü uyumluluk dalında kullanır; yeniden hesaplanan yeni bordrolar v2 olur. | `insurance_gv_cap_oracle::taxable_road_allowance_is_included_in_monthly_insurance_limit_base`: 62.000 TL taban + 3.100 TL vergilendirilen yol × %15 = 9.765 TL; 20.000 TL sağlık primi tavanı bağlar. Önceki uygulama 9.300 TL veriyordu. `G021` independent golden tavanı 9.450 TL'den 9.915 TL'ye güncellendi. V5 backup replay regresyonu tarihsel çıktının kuruşuna kadar korundu. Dayanak: GVK 63/3; GİB'nin açıklamasında vergilendirilen aylık ücret, prim, ikramiye, sosyal yardım ve zamlar tabana girer, gerçek gider karşılığı hariçtir. |
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

UAT’nin son yedeğindeki 40 normal bordro satırı bağımsız yeniden hesaplandı. GV dışı **12.184,31 TL artış** kalem kalem kapanıyor:

| GV dışı kesinti | Kasım 2027 | Aralık 2027 | Aralık − Kasım |
|---|---:|---:|---:|
| İşçi SGK | 305.833,44 | 315.938,23 | +10.104,79 |
| İşçi işsizlik | 21.845,24 | 22.566,96 | +721,72 |
| Damga vergisi | 11.566,58 | 12.114,38 | +547,80 |
| Sendika | 14.054,56 | 14.054,56 | 0,00 |
| BES/OKS | 25.181,00 | 25.991,00 | +810,00 |
| İcra | 3.150,00 | 3.150,00 | 0,00 |
| Nafaka | 3.200,00 | 3.200,00 | 0,00 |
| Kişi borcu | 350,00 | 350,00 | 0,00 |
| Doğum/askerlik borçlanması kesintisi | 250,00 | 250,00 | 0,00 |
| Hayat/sağlık kesintisi | 500,00 | 500,00 | 0,00 |
| Diğer kesinti | 100,00 | 100,00 | 0,00 |
| **GV dışı toplam** | **386.030,82** | **398.215,13** | **+12.184,31** |

Bu artış, GV’deki **310.742,94 TL azalışı** kısmen karşılar ve toplam çalışan kesintisini tam **298.558,63 TL** azaltır. Brüt artışı 78.176,64 TL olduğundan net ödeme 376.735,27 TL yükselir. Brüt farkın 62.389,20 TL’si günlük taban ücretin 20 kişi için bir ek ücret gününde ödenmesidir; kalan 15.787,44 TL, iş primi (+5.427,84), yol (+3.187,80) ve yemek (+7.171,80) artışlarından oluşur. UAT kaynak puantajında Kasım satırlarının her birinde 30 ücret günü (22 `Ç` + 8 `T`), Aralıkta 31 ücret günü (23 `Ç` + 8 `T`) vardır. Her iki dönemdeki günlük taban ücret 3.119,460460108 TL’dir; satır başına kuruşa yarım yukarı yuvarlayınca taban brüt Kasım 93.583,81 TL, Aralık 96.703,27 TL çıkar. Bu nedenle 31 günlük hizmet dönemindeki bir ek gün, tanımlı günlük ücret kuralına göre beklenen girdidir; tek başına anomali değildir.

Vergi düşüşünün yönü, hizmet dönemi ile vergi ödeme ayı ayrımına uyumludur: `2027-11` hizmeti 15 Kasım–14 Aralık ve ödeme tarihi 14 Aralık 2027 olduğundan vergi ayı 2027/12’dir; `2027-12` hizmeti 15 Aralık–14 Ocak ve ödeme tarihi 14 Ocak 2028 olduğundan 2028/1’dir. Sentetik dilimlerde Kasım kümülatif matrahlarının tümü en üst %40 marjinal dilimdedir. Ocakta yeni vergi yılı başladığı için GV ve asgari ücret istisnası kümülatifleri sıfırlanır; aylık vergi matrahı ilk %15 diliminde hesaplanır. Bağımsız örnek satırda Kasım/Aralık GV **25.958,69 / 10.195,82 TL** çıkar ve uygulamayla kuruşuna kadar eşleşir.

`crates/payroll-core/tests/fixtures/uat_2027_financial_cohort.json` kaynak kazançları, puantaj kodu sayımlarını, sentetik parametreleri, önceki kümülatifleri ve uygulama sonuçlarını ayrı alanlarda tutar. `uat_november_december_rows_reconcile_to_independent_oracle` uygulama hesaplama fonksiyonlarını çağırmadan her 40 satırda günlük ücret × ücret günü, brüt, PEK, çalışan ve işveren primleri, GV matrahı, artan oranlı GV, asgari ücret istisnası, damga vergisi, sendika, BES ve her doğrudan kişisel kesintiyi yeniden hesaplar. Test 30/31 günlük dönem uzunluğunu ve ödeme tarihiyle vergi yılı/ayını da doğrular. Kasım için bağımsız brüt GV/istisna/kesilen GV toplamları **736.516,61 / 224.604,00 / 511.912,61 TL**, Aralık için **285.396,27 / 84.226,60 / 201.169,67 TL**; satır ve dönem toplamları uygulamayla kuruşuna kadar eşleşir.

Bu açıklama vergi parametrelerinin resmî 2027/28 olduğu anlamına gelmez. Ayrıca Ekim 2027 tüm 20 kişi için yuvarlama-politika hatasıyla bloke olduğundan UAT’nin 2027 kümülatif tarihçesi kesintisiz değildir. Kasım/Aralık karşılaştırması kendi kayıtlı vergi kümülatiflerinin aritmetiğini doğrular; eksik Ekim bordrosunu veya yıllık tam vergi zincirini onaylamaz. UAT’de 2028 Şubat ücret artışında yol tutarı ve yuvarlama modu değişimi gözlendi, Ekim 2027 ve Ocak 2028 hesaplamaları tamamlanamadı.

## Kategori kapsamı ve durumu

`PASSED` yalnızca tabloda yazılı oracle/regresyon örneklerinin geçtiğini belirtir; modülün her hukuki/sözleşmesel varyantı için genel uygunluk beyanı değildir.

| Kategori | Durum | Bağımsız kanıt ve açık sınır |
|---|---|---|
| Brüt, günlük ücret, kısmi ay, çalışma günü | Kısmi | Core goldens ve normal-payroll oracle taban/gün/kazanç matematiğini sınar. Kasım/Aralık UAT’sinde 40 satır için kaynak puantaj kodları, hizmet tarihleri ve snapshot günlük ücretinden 30/31 ücret günü yeniden hesaplandı; taban brüt 40/40 satırda kuruşuna eşleşti. Bütün ücret politika değişimi ve kısmi ay bileşimleri exhaustive değildir. |
| Çalışan/işveren SGK ve işsizlik | Kısmi | UAT’de 40 satırın işçi/işveren SGK ve işsizlik primleri oran × bağımsız PEK ile eşleşti. Eylül 2028 SGK dosyası 20 normal satır için de bağımsız eşleşti. Oran ve 2027/28 UAT limitleri sentetik; teşvik/işveren indirimi ve retro ek bildirgesi doğrulanmadı. |
| PEK alt/üst sınırı, istisna ve devreden PEK | Kısmi | Bağımsız PEK oracle ve 2026 sınır golden’ları alt/üst sınır, carry kapasitesi, iki ay ömrü ve aynı ödeme ayı event’lerini kapsar. UAT 40 satır PEK’i yeniden hesaplar ancak bu satırlarda taban/tavan bağlayıcı değildir. Bazı sıfır gün/ödeme kalemi sınıflandırmaları sözleşme yorumuna bağlıdır (aşağıya bakın). |
| GV matrahı, kümülatif GV, dilim, asgari ücret istisnası, sigorta primi indirimi | Kısmi | UAT’de 40 satırın GV matrahı, kümülatif dilim farkı, asgari ücret istisnası ve GV’si bağımsız parametrelerle yeniden hesaplandı. GV sigorta indirimi aday, aylık %15 tabanı ve yıllık tavan için ayrı bağlayıcı sınır testi vardır; vergilendirilen yol ve yalnız gerçek yemek istisnası tavan tabanında kullanılır. İki hizmet döneminin 2027/12 → 2028/1 vergi ödeme ayı geçişi doğrulandı. 2027/28 değerleri sentetik; tüm 24 aylık kohortta kesintisiz doğrulanmış zincir yoktur. |
| Damga vergisi ve istisnası | Kısmi | UAT’de 40 brüt damga ve kesilen damga değeri, ayrıca oran × matrah/istisna half-kuruş golden’ları integer-rational oracle’la doğrulandı. Her ödeme türü ve dönem için dış vergi beyanı yoktur. |
| Ara işlem yuvarlama ve kuruş farkı | **PASSED (test kapsamı)** | Half-up tutar helper’ı, SGK/GV/DV/iş primi bağlantıları ve öğün tarih segmenti ayrımı için exact kuruş regression’ları var. İş kuralı yuvarlamanın hangi ara adımda uygulanacağını belirleyen yerde o sözleşme korunmuştur. |
| Zam, bileşik artış, yol/yemek ve iş primi | Kısmi | Ücret politika zaman çizelgesi, oracle ile iş primi/meal sınırı, mevcut migration/parity testleri ve UAT’nin görülen tarihlerdeki dört yol artışı var. 10/10 yol tarihi UAT’de kaydedilmemiş; 2027–28 oranları sentetik. |
| Sendika, BES/OKS, nafaka, icra, kişi borcu, avans, diğer kesinti ve çoklu sıra | Kısmi | Kasım/Aralık UAT oracle’ı 40 satırda sabit/oranlı BES, sendika, nafaka, icra, kişi borcu, doğum/askerlik, hayat/sağlık ve diğer kesintileri tek tek kontrol eder; sabit BES’i doğrudan kesinti toplamında çift saymaz ve neti yeniden kurar. Bütün kesinti/mahsup önceliklerinin golden matrisi yok; icra/nafaka emir belgeleri ve işveren uygulama kuralı ayrıca gerekir. |
| Puantaj kodları, 28/29/30/31, artık yıl ve hastalık raporu | Kısmi | 7 kod, 15–14 dönem uzunlukları, artık yıl, 30/31 UAT ücret günü, ilk beş raporun ilk iki günü ve altıncı rapor için bağımsız tarih/episode oracle’ları vardır. UAT rapor kaydı/düzeltme arayüz akışını çalıştırmadı; kurumca ödeme kuralının sözleşme/işveren kaynağı yoktur. |
| Tediye, TİS, ek tahakkuk sırası | Kısmi | Golden ve event zinciri testleri aynı ay PEK/vergi istisnası taşımasını; UAT bir Tediye ve bir TİS’i hesapladı. Tüm farklı tahakkuk tarih/sıraları ve UI’de ek ödeme kapsamı tamamlanmadı. |
| Retro pozitif/negatif/sıfır, tekrar ve kesinleşmiş dönem | Kısmi | Retro oracle’ı kaynak dönem farkları, pozitif/negatif, sıfır tekrar, deterministic replay ve immutable baseline regression’larını test eder. Eksik normal source period artık 0 kabul edilmez, fail-closed testle korunur (`bug_retro_001_test_b_missing_normal_inside_window_fails_closed`, `native_bug_retro_001_missing_normal_source_period_is_rejected_not_zero`). UAT UI’de pozitif örnek dışında negatif/sıfır tüm akışları tamamlamadı. |
| Dönemler arası net banka/SGK/kesinti mutabakatı | Kısmi | `Banka_Odeme_Listesi_2028-09_Tumu.xlsx` 20 normal net toplamını 1.509.349,19 TL, `...after-retro.xlsx` +1.486,02 TL retro ile 1.510.835,21 TL gösterir; yedekteki satırlar ve toplamlar kuruşuna kadar eşleşir. `SGK_Prim_Kontrolu_2028-09.xlsx` 20 normal bordroda PEK 2.530.164,90 TL’yi ve dört prim bileşenini (toplam 980.439,02 TL) eşler. Retro satırının PEK/prim artışı bu SGK dosyasında yoktur; retro sonrası SGK toplamı doğrulanmadı. Diğer 19 dönem için dışa aktarım dosyası yoktur. |

## Açık ve çözülemeyen mali/iş kuralı farkları

1. **0 çalışma günü ve ücret kalemlerinin PEK sınıfı — UNRESOLVED, P1 aday.** UAT source audit’te 29 ücretsiz `R` günlü örnekte 4.025,00 TL brüt kalem bulunmasına karşın uygulama SGK/işsizlik primini 0 ve sonraki aya PEK carry’sini 200,00 gösteriyor; brüt kalemlerin kalan 3.825,00 TL PEK karşılığı açıklanamıyor. Bu, genel olarak bütün 0 gün ödemeler PEK’e tabidir diye düzeltilemez: SGK 2026 rehberi ücret dışı PEK’e tabi ödeme, prim günü/ücret bulunmayan ayda takip eden en fazla iki aya aktarılabilir diyor; ücret, TİS yardımı ve diğer kalemlerin niteliği ayrı sınıflandırılmalıdır. Örnek kalemlerin iş sözleşmesi/TİS ve parametre türü bu denetim paketinde yoktur. Kesin kök neden ve beklenen prim onaylanmadı; PASS değildir. [SGK 2026 PEK açıklaması](https://www.sgk.gov.tr/Content/Post/2e0c9e1a-2cfe-4456-af10-49d3de0c58ba/Prime-Esas-Kazanc-Miktarlari-2026-01-14-10-35-39)
2. **Eylül 2028 retro satırının SGK kapsamı — UNRESOLVED.** `Banka_Odeme_Listesi_2028-09_Tumu_after-retro.xlsx` normal banka toplamı üzerine 1.486,02 TL retro netini ekler ve yedekteki satırla eşleşir. `SGK_Prim_Kontrolu_2028-09.xlsx` yalnız 20 normal çalışanı içerir; yedekteki retro kayıtta +178,66 TL PEK, +38,86 TL işveren SGK ve +3,57 TL işveren işsizlik primi vardır; çalışma kitabındaki retro/tamamlayıcı prim hücreleri sıfırdır. SGK’nın 2026 açıklaması kamu personeli ve TİS kapsamındaki geriye dönük ücret farklarının ilgili olduğu ayların PEK’ine dahil edilmesini öngörür; bu nedenle sadece Eylül normal tahakkuk dosyası retro kapsamını kapatmaz. İlgili aylar için ek bildirim/tahakkuk belgeleri elde olmadığından, uygulama hatası olup olmadığını bu dosyalarla sınıflandıramıyoruz. [SGK, Prime Esas Kazanç Miktarları 2026](https://www.sgk.gov.tr/Content/Post/2e0c9e1a-2cfe-4456-af10-49d3de0c58ba/Prime-Esas-Kazanc-Miktarlari-2026-01-14-10-35-39)
3. **2027–2028 mevzuat uygunluğu — NOT VERIFIED.** Bu rakamlar sentetik test değerleri. Matematik o parametreler karşısında denetlenmiştir; resmî oran olarak sunulmamıştır.
4. **UAT akış bütünlüğü — PARTIAL.** 18/20 dönem ve 360/400 normal bordro hesabı tamamlandı; Ekim 2027/Ocak 2028 40 kayıt bloke oldu. 24 profilin tamamı, tüm rapor kuralları, ek ödeme ve tüm retro türlerinin arayüz akışı tamamlanmadı.

## Test ve CI durumu

Yerel doğrulamalar:

- Tam `cargo test --workspace`: geçti; son koşu bağımsız UAT oracle’ı, insurance-limit oracle’ı, tüm Rust integration/regression testleri ve doc-tests’i içerdi.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, `node --check scripts/test-tauri-webdriver.mjs` ve `git diff --check`: geçti.
- `node scripts/build-wasm.mjs`, WASM package allowlist/freshness ve `node scripts/verify-web-build.mjs`: geçti. Release WASM, düzeltmeleri tarayıcı motoruna taşımak için yenilendi.
- TypeScript typecheck, production graph, Rust→TypeScript contract ve privacy boundary: geçti.
- Vite production build ve Playwright tarayıcı E2E: geçti; 34/34 senaryo. Netlify runtime smoke: geçti; CSP, WASM yükleme, IndexedDB ve hesaplama kontrolleri.
- macOS native derlemesi `cargo build --manifest-path src-tauri/Cargo.toml --bin bordro-programi --features native-webdriver,custom-protocol`: geçti. Yerel WebView E2E uygulamanın görünür sayfa başlığını, storage yüklenmesini ve `get_periods` IPC yanıtını doğruladı.
- Bu makinede Bun çalıştırılabilir dosyası bulunmadığından `bun run test:ci` yerel olarak başlatılamadı. TypeScript Bun test/coverage, WASM adapter ve platform runner kontrolleri GitHub Actions’ta çalışır; sonuç commit’in teslim raporuyla birlikte verilir.

GitHub Actions durumu push ve run tamamlandıktan sonra eklenecektir. Bu rapor tüm mevzuat/sözleşme varyantlarının PASS olduğu iddiası değildir.
