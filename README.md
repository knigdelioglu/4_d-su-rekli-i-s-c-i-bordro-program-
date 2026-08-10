# 4/D Sürekli İşçi Bordro Programı

React + TypeScript arayüzü, Tauri/Rust servisleri ve SQLite yerel veritabanı kullanan 4/D sürekli işçi bordro uygulamasıdır. Üretim bordro hesabı Rust tarafındaki tek hesap motorunda yapılır; tarayıcı modu kayıtları saklama, içe/dışa aktarma ve inceleme için kullanılabilir.

## Gereksinimler

- Bun
- Rust toolchain ve Cargo
- Tauri geliştirme bağımlılıkları

## Geliştirme ve doğrulama

```bash
bun install
bun run dev
bun test
bun run lint
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Tauri geliştirme akışı için `bun tauri dev` kullanılabilir. SQLite verisi kullanıcı uygulama dizinindeki `4d_bordro_data/bordro.sqlite` dosyasında tutulur.

## Veri güvenliği

JSON yedekleri sürüm 2 sözleşmesiyle dönem, personel, kurum ayarları, puantaj, bordro, vergi açılışları, hastalık raporu kayıtları ve yıllık bordro parametrelerini birlikte içerir. İçe aktarma ve örnek veri sıfırlama, mevcut domain verisini tek SQLite transaction içinde silip yükler. Kesinleştirilen (`FINALIZED`) bordrolar yeniden hesaplanamaz veya geriye alınamaz.

Vergi açılışında authoritative kaynak `personnel_tax_opening` tablosudur. Personel formundaki devir alanları yalnızca bu tabloda aynı vergi yılı için ayrı açılış bulunmadığında geriye dönük uyumluluk fallback'idir; iki kaynak çakışırsa tablo kaydı önceliklidir.
