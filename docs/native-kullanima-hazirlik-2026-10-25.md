# Native uygulama kullanıma hazırlık denetimi — 25 Ekim 2026

**Karar: Kurulu macOS uygulaması için üretime hazır onayı henüz verilemez.** SQLite'ın yapısal ve temel aritmetik kontrolleri geçti; kurulu uygulamanın güncel düzeltmeleri taşıdığı ve native uçtan uca akışın çalıştığı doğrulanmadı.

Kapsam kullanıcı açıklamasıyla native Tauri ürününe daraltıldı. Deneme amaçlı web dağıtımının Netlify/WASM kontrolleri bu kararın engelleri değildir. React arayüzünün native WebView içinde kullanılan ortak bileşenleri kapsamda kalır.

## İncelenen gerçek paketler

| Artefakt | Gözlem |
|---|---|
| `/Applications/4D Bordro Programı.app` | Sürüm 0.1.0, arm64, ad-hoc imza |
| Proje `app/4D Bordro Programı.app` | Kurulu uygulamayla aynı executable SHA-256 |
| Her iki executable dosyasının değişiklik zamanı | 8 Ekim 2026, 11:28 +0300 |
| `target/aarch64-apple-darwin/release/bordro-programi` | 15 Ekim tarihli; kurulu executable ile farklı hash |
| İncelenen kaynak | `566109cb598a0802e988f291d6edb86d7fcc7422`, 24 Ekim 2026 |

Kurulu/proje uygulaması executable SHA-256: `c3dfeef3092d50c65cda5991394ef31ca6b736b4b7c96fdaf892fcc3af1507d5`.
Release hedefindeki executable SHA-256: `e9d397486ce1e6731952b2242bdd856e5da161342ed0a1090b6e1b9acda6d1e8`.

**P1 doğrulama açığı — paket/kaynak eşleşmesi:** Son kaynak commit'i retro STALE invalidation, retro yemek istisnası, negatif retro primleri, replay bağımlılığı ve tarih listesi düzeltmeleri içeriyor. Kurulu paket için bu commit'e bağlanan build kimliği bulunmadı. Dosya tarihi tek başına hangi düzeltmenin binary'de bulunduğunu kanıtlamaz; fakat güncel kaynağı inceleyerek eski tarihli pakete hazır onayı vermek de mümkün değildir. Ayrıca 15 Ekim tarihli hedef executable'ın varlığı, `/Applications` paketinin güncellendiği anlamına gelmez.

`codesign --verify --deep --strict` hem proje paketinde hem kurulu pakette geçti. Ad-hoc imza yerel paket bütünlüğü kontrolüdür; Apple Developer kimliği/notarizasyon kanıtı değildir. Başka Mac'lere dağıtım hazırlığı ayrıca doğrulanmalıdır.

## Gerçek native veritabanında salt okunur kontroller

Veritabanı `~/.4d_bordro/4d_bordro_data/bordro.sqlite`, SQLite URI `mode=ro` ve `PRAGMA query_only=ON` ile açıldı. İsim, T.C., IBAN veya kişi bazlı maaş bilgisi rapora alınmadı.

| Kontrol | Sonuç |
|---|---|
| SQLite `quick_check` | `ok` |
| `foreign_key_check` | 0 ihlal |
| Şema `user_version` | 24 |
| Kayıt sayıları | 25 personel, 34 dönem, 481 puantaj, 437 bordro |
| Bordro durumları | 429 CALCULATED, 8 FINALIZED |
| Retro batch | 2 CALCULATED |
| Decimal ile brüt − kesinti = net kontrolü | 437 satırda 0 uyuşmazlık |
| Geçersiz/sonlu olmayan finansal toplam | 0 |
| Hesap metadata alanı | 8 satırda eksik veya JSON nesnesi olarak okunamıyor; geçmiş uyumluluk kurallarıyla ayrıca değerlendirilmelidir |

Bu kontroller kayıtlı toplamların kendi içindeki tutarlılığını gösterir. Brüt kazançları, vergi/prim matrahlarını, oranları, kümülatif zinciri veya sözleşme uygunluğunu bağımsız olarak yeniden hesaplamaz. CALCULATED sayısı ödeme yapılabilir kayıt sayısı değildir; banka listesi yalnız FINALIZED kayıtları seçer.

## Native kaynak incelemesi

- `src-tauri/src/lib.rs`: Native açılış SQLite bağlantısı kurup Tauri state'ine veriyor. Bordro, personel, dönem, puantaj, retro, yedek ve dosya komutları handler'a kayıtlı.
- `src-tauri/src/services/payroll_service.rs:114`: Hesaplama ve persistence transaction sınırında. `:718` kesinleştirme, kayıtlı girdiyi transaction içinde yeniden kurup core doğrulaması ve dependency invalidation ile kaydediyor.
- `src-tauri/src/repositories/payroll_repo.rs:789`: FINALIZED geri dönüşü ve STALE kesinleştirmesi reddediliyor; önceki vergi zinciri kontrol ediliyor.
- `src-tauri/src/commands/personnel_cmd.rs`: Personel ve vergi açılışı birlikte transaction içinde kaydediliyor.
- `src-tauri/src/commands/backup_cmd.rs`: Native JSON yedeği arayüzdeki kopya yerine authoritative SQLite'tan üretiliyor; dosya yazımı geçici dosya, sync ve rename kullanıyor. Geri yükleme migration servisinin transaction akışına giriyor.
- Lisans hesaplama/kesinleştirme komutlarında native tarafta kontrol ediliyor. Varsayılan derleme modu `optional`; ücretli/zorunlu lisans dağıtımı isteniyorsa bu ayrıca bir yayın koşuludur.

## Native kullanımı etkileyen somut kusurlar

**P2 — macOS güncellemesinde geri dönüş yok.** `build_app.command:130–144`, eski `/Applications` paketini silip sonra kopyalıyor. Yeni kopyalama veya imza doğrulaması başarısız olursa önceki çalışan kurulum kaybedilebilir. Veritabanının silindiğine dair bulgu yok. Yeni paketi geçici kardeş dizine kopyalayıp doğrulamak, eski kurulumu koruyarak değiştirmek ve hata halinde geri dönmek gerekir.

**P3 — native WebView banka kopyalama bildirimi hatalı olabilir.** Ortak `BankaListesi.tsx:98–106` pano Promise'ini beklemeden başarı gösteriyor. Native WebView pano erişimi başarısız olduğunda görünür hata verilmeli.

Windows native dağıtımı da hedefleniyorsa önceki rapordaki test CI'dan bağımsız yayın ve aynı sürüm asset'ini değiştirme bulgusu geçerlidir (`.github/workflows/windows-release.yml:110`). Bu, macOS yerel hesaplama hatası değildir.

## Çalıştırılmayan kontroller ve geçiş koşulları

Uygulama envanterinde çalışan native Bordro uygulaması görülmedi; uygulama başlatılmadı. UI'da hesaplama, kesinleştirme, yedek geri yükleme, Excel/PDF ve yazdırma turu yapılmadı. Gerçek kayıtlar değiştirilmedi; uygulama kurulmadı veya güncellenmedi.

Çalıştırma sınırı: security-audit becerisinin OS düzeyinde tam yalıtım koşulları bu ortamda garanti edilemedi. macos-reverse-engineering becerisi de paket incelemesinde hedef uygulamayı çalıştırmayı kapsam dışı tutuyor. Buradaki executable incelemesi statiktir. Önceki test raporları güncel paket testi olarak sunulmadı.

Native kullanıma geçmeden önce:

1. Güncel kaynak commit'inden üretilen paketin build kimliği/hash'i kaydedilmeli; kurulu paket bununla eşleştirilmeli.
2. Aynı commit'te Rust core/native regresyonları ve Tauri IPC kontrolleri geçmeli. Özellikle 24 Ekim retro ve vergi düzeltmeleri doğrulanmalı.
3. Gerçek DB'yi kullanmayan izole profilde açılış → personel/puantaj → normal/tediye/TİS/ek ödeme → retro → kesinleştirme → yeniden açılış turu tamamlanmalı.
4. Native JSON yedek/geri yükleme ve banka/SGK/Excel/PDF çıktıları bağımsız toplamlarla karşılaştırılmalı. Önceki mali denetimde açık kalan sıfır prim günlü kazanç sınıflandırması sözleşme bazında kapatılmalı.
5. macOS güncellemesinde hata halinde çalışan eski paket korunmalı.

Sonuç: Mevcut SQLite'ta incelenen kontroller veri bozulmasına işaret etmiyor. Üretime hazır kararını engelleyen ana konu, kurulu native sürümün güncel kaynak ve test kanıtıyla eşleştirilememesi; ardından native uçtan uca doğrulama ve güvenli güncelleme eksikliği.
