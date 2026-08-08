# CODING CONTRACT — taxYear/taxMonth Kümülatif GV Zinciri (E) + Vergi Metadata Kilidi (H/I) + Snapshot (J)

- Tarih: 2026-08-08
- Tür: coding (backend ajan + Hermes doğrulama), KOD DEĞİŞİKLİĞİ VAR, git commit YOK
- Backend: opencode 1.18.15 (`~/.opencode/bin`), model `opencode-go/deepseek-v4-flash` (`opencode/deepseek-v4-flash-free` o gün "Unexpected server error" veriyordu — kota/kesinti; aynı günün ilk saatlerinde çalışıyordu)
- Policy: STRICT — repo kökü `opencode.json`: `*` allow, bash/webfetch deny, `external_directory` {tool-output/var-folders allow, gerisi deny}
- Önceki agy yolculuğu: agy 1.1.11 + gemini-3.6-flash-high STRICT 6 denemede de path eşleşmesi bozuk (Unicode NFC/NFD + agy'nin settings.json yerine config.json okuması + modelin mutlak path ısrarı) → kullanıcı onayıyla opencode'a geçildi; agy config'leri yedeklerden geri yüklendi (mevlid pipeline'ı korundu)
- NFC/NFD dersi (kalıcı): Downloads'ta aynı isimde iki girdi (NFD gerçek repo + NFC kısmi kopya) APFS lookup'ı kararsızlaştırıyordu; çözüm: her terminal komutunda `REALROOT="$(python3 -c "...os.listdir(...)startswith('4_d')[0]...")"` — readdir'den gerçek byte adı
- Prompt: `/tmp/repo-engineer/4d_taxyear_fix/task_prompt.md` (9471B — görev metni + contract + PATH PROTOKOLÜ)
- Run: `opencode run "$(cat task_prompt.md)" --model opencode-go/deepseek-v4-flash` → pid 29333, log `/tmp/repo-engineer/4d_taxyear_fix/run5.log` (48404B)

## Yapılan değişiklikler (8 dosya: 7 M + 1 yeni; 494+/58-)

1. `src-tauri/src/services/cumulative_tax_service.rs` — E: `get_previous_cumulative_gv` tamamen taxYear/taxMonth domainine geçirildi: opening araması `active_period.taxYear`; collision guard `taxYear==opening.year && taxMonth<start_tax_month`; önceki bordro filtresi `taxYear==aktif.taxYear && taxMonth<aktif.taxMonth`; start_tax_month effective_period'dan (farklı vergi yılındaysa 1)
2. `src-tauri/src/services/period_service.rs` — YENİ: `PeriodService::save_period` — mevcut period okunur; taxYear/taxMonth farklıysa ve o donemId için HERHANGİ bordro (DRAFT/CALCULATED/FINALIZED) varsa `ValidationError("...Vergi Yılı/Ayı değiştirilemez.")`; bordrosuzsa güncelleme serbest
3. `src-tauri/src/commands/period_cmd.rs` — save_period artık PeriodService üzerinden (repo UPSERT'e güvenilmez)
4. `src-tauri/src/repositories/payroll_repo.rs` — J: `oncekiKumulatifAsgariGvMatrahi` gv_snapshot_json'dan kayıpsız türetme `(asgariUcretReferansKumulatifMatrahi − asgariUcretGvMatrahi).max(0)` — yeni kolon/migration YOK
5. `src-tauri/src/services/mod.rs` — period_service modül kaydı
6. `src/utils/payrollUtils.ts` — `effectiveTaxOf()` (legacy fallback: ay+1, Aralık→Ocak), `checkDevredenGvMatrahConflict` / `calculatePreviousCumulativeGvMatrah` / `calculatePreviousCumulativeAsgariUcretGvMatrah` taxYear/taxMonth authoritative (PEK devreden fonksiyonu dokunulmadı)
7. `src/components/BordroHesaplama.tsx` — açılış kaydı `year: aktifDonem.taxYear` + `effectiveFromPeriodId`
8. `src/components/PeriodManagerModal.tsx` — vergi yılı/ayı seçicisi korundu; bordrolu dönemde değişiklik denemesinde uyarı bandı (güvenlik yalnız UI'a bağlı değil)

## Hermes tarafından düzeltilen model sözdizimi hataları (test dosyası)

- `domain_tests.rs:1608` — `test_previous_asgari_gv_uses_tax_year_month` eksik kapanış (aktif_tax6 assertion'ları + `Ok(())` + `}` eklendi; prev_6=140377.50)
- `domain_tests.rs:1960` — `bordro_kaydi()` helper: `gelirToplam,` shorthand → `gelirToplam: gelir_toplam,`

## Doğrulama (Hermes bizzat koştu — agent raporuna güvenilmedi)

- `cargo test` (src-tauri): **47 passed, 0 failed** (yeni: test_a..h + previous_asgari_gv_uses_tax_year_month; mevcut regressionlar dahil)
- `bun test`: **28 passed, 0 failed**
- `bun run build`: ✓ (3.04s; chunk boyut uyarısı mevcut/önemsiz)
- `bun run lint`: 1 hata — `isPrimeGroup.test.ts(38,25)` `.not` matcher tipi; **baseline'dan beri mevcut** (dosya değişmedi; iş primi testi — kapsam dışı, dokunulmadı)
- Kapsam dışı taraması: diff'te yalnız test fixture'larında geçiyor (BordroDonemi verileri, pekDetay:None vb.) — üretim kodunda PEK/SGK/yemek/GÇ/GÇT/rapor/iş primi/GV tarifesi/Tediye/TİS değişikliği YOK

## Sonuç

- E: DÜZELTİLDİ (Rust + TS senkron)
- H/I: DÜZELTİLDİ (backend guard zorunlu — service katmanı; DRAFT/CALCULATED/FINALIZED ayrımsız)
- J: DÜZELTİLDİ (duplicate kolon yok; authoritative snapshot'tan türetme — Test H doğruluyor)
- Test C–H + yıl geçişi senaryoları eklendi ve geçti
- Git: HEAD 4bb747b, commit YOK; untracked: `opencode.json` (STRICT config — run sonrası repo kökünde duruyor, kullanıcı kararına bırakıldı) + bu dosya
- Migration gerekmedi (J türetme ile çözüldü)
