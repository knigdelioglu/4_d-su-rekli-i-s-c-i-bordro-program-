# Kullanıma hazırlık denetimi — 25 Ekim 2026

> Kapsam düzeltmesi: Web arayüzü kullanıcı tarafından deneme amaçlı olarak tanımlandı. Native ürün hakkındaki güncel değerlendirme `native-kullanima-hazirlik-2026-10-25.md` dosyasındadır. Bu belgedeki web/WASM maddeleri native kullanım kararı için doğrudan engel sayılmamalıdır.

**Karar: Mevcut kanıtlarla gerçek bordro üretiminde genel kullanıma hazır onayı verilemez.** Sentetik veriyle kontrollü kabul testi yapılabilecek bir altyapı mevcut; bu oturumda uygulamanın çalışması ve mali sonuçları yeniden doğrulanmadı.

İncelenen sürüm: `566109cb598a0802e988f291d6edb86d7fcc7422` (24 Ekim 2026). İnceleme başlangıcında çalışma ağacı temizdi. Uygulama kodu, veritabanı, kurulu uygulama ve dış servisler değiştirilmedi. Bu rapor tek yeni dosyadır.

## Kapsam ve kanıt sınırı

Kaynak üzerinden ortak Rust/WASM mimarisi, son commit'in değişiklikleri, bordro/kesinleştirme sınırları, tarayıcı persistence ve yedekleme, native yedekleme, SGK/banka çıktıları, CI ve paketleme akışları incelendi. Önceki güvence ve finansal denetim belgeleri tarihsel kanıt olarak değerlendirildi. Bu, eksiksiz güvenlik denetimi veya güncel mevzuata uygunluk onayı değildir.

Bu oturumda depo betikleri, testler, build veya uygulama çalıştırılmadı. Kullanılan security-audit becerisi hedef kodun çalıştırılması için ağsız, boş/izinli ortam değişkenleriyle, salt okunur hedef ve yalnız scratch yazımıyla, CPU/bellek/süre/süreç/dosya/disk limitleriyle OS düzeyinde yalıtım istiyor. Sunulan ortamda bu kontrollerin tamamı garanti edilemedi. Kontroller yalnız güvenilir araçlarla kaynak/dosya okuması ve ayrıca yazılmış küçük bir dosya bütünlüğü denetimiyle sınırlı tutuldu.

## Kaynakta doğrulanan bulgular

Öncelikler sürüm güvenilirliği öncelikleridir; güvenlik açığı derecelendirmesi değildir.

### 1. P1 — Windows paketi test CI sonucuna bağlanmadan yayımlanıyor

- Kaynak: `.github/workflows/windows-release.yml:3`, `:22`, `:110`.
- Her `main` push'u bu workflow'u tetikliyor. Yayın koşulu push olayında doğru; job'da test CI sonucunu bekleyen bir kapı yok. Akış derleme ve WASM paket kontrolleri yapıyor, ancak bordro regresyonları ve uçtan uca testleri çalıştırmıyor.
- Sonuç: Test CI başarısız olsa bile derlenebilen bir paket yayımlanabilir. Aynı konfigürasyon sürümü kullanıldığında mevcut release asset'i silinip yenisi yükleniyor; tag'in işaret ettiği commit ile indirilen paketin kaynak commit'i ayrışabilir.
- En küçük çözüm: Yayını aynı commit için zorunlu testlerin başarı durumuna bağlamak; sürüm/tag/commit eşleşmesini doğrulamak ve yayımlanmış sürümü yerinde değiştirmek yerine yeni sürüm üretmek.
- Kabul kanıtı: Başarısız test durumu olan bir commit'in yayın adımına ulaşamadığı ve paket kaynağının release tag'iyle eşleştiği izole CI doğrulaması.

### 2. P2 — WASM binary farkının yalnız metadata olduğu doğrulanmıyor

- Kaynak: `.github/workflows/ci.yml:102–119`.
- `awk`, `payroll_wasm_bg.wasm` için her `M` kaydını kabul ediyor. Binary içeriği veya değişen section incelenmeden farkın yalnız runner metadata'sı olduğu yazılıyor.
- Sonuç: Depoda tutulan WASM ile CI'da yeniden üretilen WASM arasında gerçek kod farkı da bu kapıdan geçebilir. CI sonraki kontrollerde yeni üretilen dosyayı kullanırken Netlify build'i depodaki dosyayı kullanabilir. Kaynak hash'i binary'nin semantik eşitliğini kanıtlamaz.
- En küçük çözüm: Sabit üretim ortamında birebir karşılaştırma veya yalnız izin verilen metadata farklarını ispatlayan section karşılaştırması; gerçek code/data farklarında hata.
- Kabul kanıtı: Metadata dışı kontrollü binary farkının kapıyı düşürdüğü regresyon kontrolü.
- Bu oturumda mevcut binary'nin hatalı olduğu gösterilmedi; bulgu doğrulama kapısının davranışıdır.

### 3. P2 — macOS güncellemesi eski kurulumu kopyalamadan önce siliyor

- Kaynak: `build_app.command:127–144`.
- Kurulu uygulama önce siliniyor; ardından yeni paket `/Applications` içine kopyalanıp doğrulanıyor.
- Sonuç: Kopyalama, disk veya imza doğrulaması hatasında çalışan eski kurulum kaybedilebilir. Bordro veritabanının silindiğine ilişkin bir bulgu yoktur.
- En küçük çözüm: Yeni paketi geçici kardeş dizine kopyalayıp doğrulamak; eski kurulumu geri dönüş kopyasıyla koruyarak değiştirmek ve hata halinde geri almak.
- Kabul kanıtı: İzole kurulum dizininde kopyalama/doğrulama hatası üretildiğinde önceki kurulumun çalışır kalması.

### 4. P3 — Banka listesinde kopyalama başarısı erken gösteriliyor

- Kaynak: `src/components/Listeler/BankaListesi.tsx:98–106`.
- `navigator.clipboard.writeText` beklenmeden `setCopied(true)` çağrılıyor; ret/hata işlenmiyor.
- Sonuç: Pano erişimi reddedildiğinde kullanıcı kopyalama tamamlandı sanabilir. Bu, banka aktarımının gerçekleştiğini göstermez; uygulama burada metin kopyalar.
- En küçük çözüm: Promise tamamlanınca başarı göstermek; API yokluğu veya ret durumunda görünür hata vermek.

## Olumlu kaynak bulguları

- Native ve tarayıcı bordro hesaplaması ortak `payroll-core` motoruna dayanıyor. Parasal yuvarlama `money.rs` içinde `MidpointAwayFromZero` ile tanımlanmış.
- Tarayıcı kayıtlarında şema doğrulaması, seri yazım kuyruğu, IndexedDB transaction ve revizyon karşılaştırması var. Bozuk snapshot ve sekmeler arası çakışmalar için görünür hata yolları bulunuyor.
- Native yedek dışa aktarımı SQLite kaynağından üretiliyor; dosya yazımında geçici dosya/atomik değiştirme yolu ve bunun testleri var. Geri yükleme transaction sınırları ve geçmiş snapshot doğrulamaları içeriyor.
- Banka listesi yalnız `FINALIZED` bordroları ve geçerli Türk IBAN'ı olan kayıtları seçiyor (`accrualListData.ts:95`).
- SGK kontrolünün güncel kodu kaynak dönem retro allocation'larını hesaba katıyor; eski finansal rapordaki retro SGK eksikliğini doğrudan bugünkü hata olarak yinelemek doğru olmaz.
- Golden corpus, bağımsız oracle, property testleri, mutation/coverage kapıları, tarayıcı E2E ve native WebView smoke altyapıları var. Varlıkları bu sürümde geçtikleri anlamına gelmez.

## Bu oturumda yapılan kontroller

| Kontrol | Sonuç ve sınır |
|---|---|
| Git sürüm/çalışma ağacı | Commit kaydedildi; başlangıç temiz |
| `git diff --check` | Geçti; başlangıçta değişiklik yoktu |
| WASM kaynak provenance özeti | Bağımsız yazılmış Node kontrolünde eşleşti; 26 kaynak/manifest girdisi |
| WASM paket allowlist | Geçti; beklenen dört dosya mevcut |
| WASM magic/version | Geçti; bu yalnız dosya başlığı kontrolüdür |
| JSON yapılandırma parse | `package.json`, Tauri config ve WASM package JSON geçerli |
| GitHub commit status sorgusu | Boş status listesi döndü; başarı kanıtı değildir |
| GitHub workflow sorgusu | Boş sonuç; araç yalnız PR tetiklemeli ilk sayfayı getiriyor, push CI durumunu kanıtlamaz |
| Güncel test/build/UI çalıştırması | Çalıştırılmadı; yalıtım sınırı yukarıda açıklandı |

Kaynak özeti: `830dea4312e18bbdefb972cebd095706c26f2a20deae85cd10bc11d2f609475a`.
WASM SHA-256: `cc817d24322caa09f01b53658c25c3939ddb9a1451af1667fceb46e20bc0c084`; boyut 2.608.654 bayt. Bu hash, dosyanın işlevsel doğruluğunu kanıtlamaz.

## Üretim öncesi kapanması gereken kanıt boşlukları

1. **Güncel commit'in test kanıtı:** `bun run test:ci`, lint, Rust workspace testleri, WASM adapter/freshness, production build, privacy, browser E2E ve hedef işletim sistemi native smoke sonuçları aynı commit'e bağlanmalı. 19/23 Eylül ve 8 Ekim tarihli raporlar, 24 Ekim'deki retro/vergi/kalıcılık değişikliklerinin geçtiğine kanıt değildir.
2. **Sıfır prim günlü kazanç sınıflandırması:** Önceki `docs/payroll-financial-audit-stage2.md:95` kaydındaki 4.025 TL örneğinin sözleşme/kalem niteliği kapanmamış. Güncel `calculations.rs:78–140` sosyal yardım, yemek, yol ve giyim gibi kalemleri `Wage` sınıfına alıyor; `:1315–1387` tavanı aşan yalnız `NonWage` kısmı devrediyor. Sıfır gün için bu ayrımın beklenen sonucu kurumun ilgili sözleşme ve dönem parametreleriyle bağımsız doğrulanmalı. Bu rapor yeni bir mali hata veya hukuki sonuç ilan etmez.
3. **Tam kabul turu:** Önceki UAT 18/20 dönem ve 360/400 normal hesapla kısmi kalmış. Sonraki düzeltmelerin ardından eksik dönemler, yıl sınırı, tediye/TİS/ek ödeme, rapor günleri ve retro zinciri tamamlanmalı; eski eksiklerin hâlen üretim hatası olduğu varsayılmamalı.
4. **Kurtarma ve çıktı mutabakatı:** Sentetik kurumda yedek al → boş izole profile geri yükle → yeniden aç → bordro/retro/banka/SGK toplamlarını kuruş bazında karşılaştır turu yapılmalı. Gerçek kurum verisi kullanılmadan başlanmalı.
5. **Lisanslı dağıtım hedefleniyorsa:** Mevcut native varsayılan lisans modu `optional`; release workflow zorunlu mod değişkenlerini tanımlamıyor. Bu README'de açıkça belgelenmiş bir dağıtım tercihidir, tek başına güvenlik açığı sayılmadı. Zorunlu lisans isteniyorsa imzalama/aktivasyon/iptal/çevrimdışı kabul turu ayrıca gerekli.

## Kullanıma geçiş ölçütü

Üç sürüm güvenilirliği bulgusu kapatılmadan, güncel commit için testler ve sentetik uçtan uca kabul/kurtarma turu doğrulanmadan uygulamayı tek başına gerçek bordro üretimi ve ödeme talimatı için hazır kabul etmiyorum. Kaynakta ciddi bir güvence altyapısı bulunuyor; mevcut engel hem somut dağıtım kusurları hem güncel çalışma kanıtının eksikliği. Mevcut inceleme yeni bir hesap hatası veya veri kaybı olayı yaşandığını kanıtlamıyor.
