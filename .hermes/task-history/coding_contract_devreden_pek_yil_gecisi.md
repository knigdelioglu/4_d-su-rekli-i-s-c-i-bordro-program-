# Görev: Devreden PEK Yıl Geçişi Düzeltmesi (5510 m.80/d)

- Tarih: 2026-08-08
- Backend: Antigravity (agy 1.1.11) — kullanıcı explicit seçim
- Model: Gemini 3.6 Flash (High) — smoke test PASS
- Policy: WORKSPACE (agy STRICT sağlayamaz — doğrulanmış; belgeli karar, NEEDS_APPROVAL gerekmedi: kullanıcı açıkça agy istedi)
- Routing gerekçesi: capabilities.json taze (2026-08-08); FAST PATH kısmi (task type+backend+model explicit, policy yok → routing araştırması atlandı, kullanıcı tercihi esas)

## Contract özeti
- Denetim: devreden PEK alanları, kalanAySayisi semantiği, önceki bordro bulma filtreleri, yıl geçişi, tavan kullanımı, taxYear bağımsızlığı
- 5 regression testi (A-E): Aralık→Ocak, Aralık→Ocak→Şubat 2 ay ömür, Ocak'ın kendi tavanı, yıl içi davranış, taxYear bağımsızlığı
- Kapsam dışı: GV taxYear/taxMonth, kümülatif GV, asgari ücret GV istisnası, SGK alt sınırı/yuvarlama, GÇ/GÇT, rapor, iş primi, yemek, Tediye/TİS, isPrimeGroup.test.ts, git commit

## Sonuç
- **Denetim bulgusu: mevcut kod yıl sınırında devri YAKMIYOR** — Rust `calculate_incoming_devreden_pek` ve TS `calculateIncomingDevredenPek` kronolojik (yil*12+ay) filtre kullanıyor; PEK motoru 2 aylık ömrü doğru yönetiyor; dönem başına kurum değerleri yeni yıl tavanını sağlıyor; taxYear PEK'e karışmıyor
- **Üretim kodu değişmedi** (yalnız `calculate_incoming_devreden_pek` → `pub fn`, test erişimi için)
- Değişen dosyalar (2): `src-tauri/src/services/payroll_service.rs` (+1 satır pub), `src-tauri/tests/domain_tests.rs` (+301 satır, 5 test)
- Doğrulama (Hermes, ajan raporuna güvenilmedi):
  - cargo test: **51 passed, 0 failed** (+1 smoke) — 46 mevcut + 5 yeni
  - bun test: **28 pass, 0 failed**
  - bun run build: **✓ 3.58s**
  - lint: tek hata isPrimeGroup.test.ts (baseline, dokunulmadı)
- Git: commit yok; HEAD değişmedi; kapsam dışı dosyalara dokunulmadı
