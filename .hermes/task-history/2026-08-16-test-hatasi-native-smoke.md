# 2026-08-16 — Test suite hatası düzeltmesi (agy / BUGFIX / WORKSPACE / STAGED)

## Contract özeti
- Task type: BUGFIX, execution: STAGED (00-plan → 01-fix → 99-final), backend: agy, model: Gemini 3.7 Flash (High), policy: WORKSPACE
- Routing gerekçesi: kullanıcı explicit backend/policy/model seçti; agy 1.1.13 capabilities.json'a yeniden detect edildi (kayıt eksikti); WORKSPACE uygun.

## Sonuç
- Kök neden: `src-tauri/tests/native_smoke_test.rs` ~satır 243: Mayıs GV matrah beklentisi `gelirToplam - isci_sgk - isci_issizlik` formülüyle hesaplanıyor, sendika aidatı (GVK 63/4, test personeli sendika üyesi, 1.656,86 TL) ihmal ediliyordu. Üretim motoru doğruydu (69.193,14).
- Düzeltme: beklenti authoritative snapshot'tan okunuyor (`gvDetay.cariGvMatrahi`), Haziran açılışı 189.193,14'e uyumlandı. Yalnız test dosyası (7+/4-).
- Doğrulama: cargo test 71/71, bun test 34/34, lint 0 hata, build başarılı. Kullanıcı verisine dokunulmadı.

## Hafıza güncelleme seviyesi
- L1 (localized): task-history kaydı; repo-index gerekmedi (yeni modül yok).
