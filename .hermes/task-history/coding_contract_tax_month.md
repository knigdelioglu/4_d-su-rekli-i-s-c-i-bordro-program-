# CODING CONTRACT — taxYear/taxMonth (ödeme/tahakkuk ayı) alanı

- Tarih: 2026-08-07
- Kaynak: DENETİM 15–14 vergi ayı eşlemesi (off-by-one) + kullanıcı kararı
- Backend: opencode (deepseek-v4-flash-free), STRICT yazma izinli config
- Tür: feature / schema (L4)

## Görev

`BordroDonemi.ay` alanı **dönem başlangıç ayını** (15'in bulunduğu ay) temsil eder ve **dönem adlandırması DEĞİŞMEYECEKTİR**. Ancak asgari ücret GV istisnası ve vergi hesapları bu `ay` alanını doğrudan vergi ayı olarak kullanmaktadır — bu yanlıştır. Çözüm:

1. `BordroDonemi`'ye ayrı **`taxYear` / `taxMonth`** (ödeme/tahakkuk ayı) alanları ekle. `ay` anlamı değişmez (başlangıç ayı; dönem adı/`id` aynı kalır).
2. Asgari ücret GV referans kümülatifi ve istisna hesabı **`taxMonth` (ve `taxYear`)** üzerinden yürütülsün. Gerçek GV kümülatif devri dönem sırasına göre kalır; vergi yılı/ayı seçimi `taxYear`/`taxMonth`'tan gelir.
3. Kullanıcı örneği (kabul kriteri): 15.06.2026–14.07.2026 dönemi `ay=6`, `taxMonth=7` ise → referans kümülatif 28.075,50 × 7 = **196.528,50**, GV istisnası **4.537,75** (Temmuz). `taxMonth=6` ise eski davranış (168.453,00 / 4.211,33) korunur.
4. UI: dönem oluşturma formunda "Ödeme/Tahakkuk Ayı" seçici. Varsayılan öneri = bitiş ayı (seçilen ay + 1; Aralık → Ocak, yıl +1). Kullanıcı değiştirebilir. Dönem adı/`id` yine seçilen (başlangıç) aydan türetilir.
5. DB migration: `payroll_periods` tablosuna `tax_year`, `tax_month` sütunları. Backfill (mevcut kayıtlar): `tax_month = (ay == 12) ? 1 : ay + 1`, `tax_year = yil + (ay == 12 ? 1 : 0)` — yani bitiş/ödeme ayı varsayımı.
6. GV snapshot (`gv_snapshot_json`) şeması **değişmez**; değerler artık doğru aya göre hesaplanıp kaydedilir. FINALIZED sonrası yeniden hesaplama engeli korunur.

## EDIT_SCOPE (yazma izinli dosyalar)

- `src-tauri/src/domain/models.rs` (BordroDonemi + serde)
- `src-tauri/src/db/migrations.rs` (tablo + backfill)
- `src-tauri/src/repositories/period_repo.rs` (CRUD)
- `src-tauri/src/services/cumulative_tax_service.rs` (referans kümülatif)
- `src-tauri/src/services/payroll_service.rs` (dönem → hesap köprüsü)
- `src-tauri/src/domain/calculations.rs` (istisna/vergi hesabı çağrıları)
- `src-tauri/tests/domain_tests.rs` (testler + yeni senaryo)
- `src/types/payroll.ts` (tip)
- `src/utils/payrollUtils.ts` (createBordroDonemi + TS fallback hesaplar)
- `src/utils/cumulativeGv.test.ts`
- `src/components/PeriodManagerModal.tsx` (ödeme ayı seçici)
- `src/utils/sampleData.ts` (gerekirse uyum)

Başka dosyaya yazma gerekirse: REQUEST_EDIT_SCOPE yapılandırılmış dönüşü yap, kapsamı kendin genişletme.

## READ_SCOPE

Tüm repo okunabilir (src/**, src-tauri/**, kök dosyalar). Kullanıcı veri dizinlerine (app DB dosyaları, ~/ dışındaki gerçek veriler) DOKUNMA; migration kod olarak yazılır, uygulama çalıştırılmaz.

## Kritik çalışma notları (takılma yaşandı — dikkat)

- **GÖRELİ yol kullan.** read/grep çağrılarında `filePath` olarak repo kökünden göreli yol ver: `src-tauri/src/domain/models.rs`, `src/utils/payrollUtils.ts` gibi. Çalışma dizinin (cwd) zaten proje köküdür.
- **MUTLAK yol YAZMA ve URL-encoded yol (`%C4%B1` gibi `%XX` dizileri) KESİNLİKLE kullanma** — ikisi de geçersizdir, permission reddi alırsın.
- **`cd` komutu KULLANMA** — zaten proje kökündesin; gerekirse `pwd` ile doğrula.
- İlk adım: `src-tauri/src/domain/models.rs`'i (göreli yol) oku; `BordroDonemi` yapısını gör.
- **Permission hatası görürsen (auto-reject / "user rejected")**: bu, görevi bırakman gerektiği anlamına GELMEZ. O çağrıyı atla, alternatif yolla (göreli yol, başka dosya) devam et, görevi tamamla.
- `src-tauri/src/db/mod.rs` okumak **zorunlu değil** — `migrations.rs` yeterli; gerekirse `src-tauri/src/db/` altındaki dosyaları doğrudan oku.
- Zincir komutlar (`&&`) reddedilebilir: tek komut kullan (`git status`, `git log --oneline -5` ayrı ayrı).

## Varsayılan constraint'ler (kaldırılamaz)

- `git commit/push/reset/checkout/restore/stash/clean` YOK — `git status`/`git diff` serbest.
- Kullanıcının mevcut değişikliklerini koru (`.gitignore` dahil).
- Gereksiz dependency yok; ilgisiz refactor yok; test silme / validation zayıflatma yok.
- `ay` alanı anlamı, dönem `id`/adlandırma, Rust authoritative mimarisi, `round_gv_amount` (half-up) yalnız GV kuralı DEĞİŞMEZ.
- SGK, PEK, iş primi, tediye/TİS, rapor konuları kapsam DIŞI.

## Kabul kriterleri

1. `cargo test` yeşil (mevcut 36 + yeni testler).
2. `bun run lint` ve `bun run build` yeşil (kapsam dışı mevcut `isPrimeGroup.test.ts:38` hatası hariç tutulabilir — dokunma).
3. Yeni test: 15.06–14.07 dönemi taxMonth=7 → 196.528,50 / 4.537,75; taxMonth=6 → 168.453,00 / 4.211,33 (regresyon).
4. Migration backfill kuralı uygulandı (kod seviyesinde).
5. TS fallback ile Rust authoritative aynı taxYear/taxMonth alanlarını kullanıyor (divergence yok).

## STATUS formatı (yanıtının sonunda)

```
STATUS: DONE | NEED_CONTEXT | REQUEST_EDIT_SCOPE | NEEDS_APPROVAL
- Değişen dosyalar: ...
- Test sonuçları: cargo test (X/Y), bun test (X/Y), lint, build
- Backfill/migration: ...
- Kalan riskler: ...
```
