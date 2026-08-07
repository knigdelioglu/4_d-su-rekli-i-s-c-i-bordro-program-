# 2026-08-07 — Asgari Ücret GV İstisnası / Kümülatif Matrah Düzeltmesi (repo-engineer)

## Özet
İlk denetim 5. maddesi uygulandı. Discovery: agy `gemini-3.6-flash-high` (plan modu, read-only). Coding: opencode `opencode/deepseek-v4-flash-free` STRICT (permission: catch-all deny + READ/EDIT_SCOPE; grep mutlak yol istediği için ikinci turda config'e mutlak pattern eklendi).

## Ana bulgu
`get_previous_cumulative_asgari_gv` kişi bordrosu varlığına (`has_payroll`) bağlıydı → yıl ortası başlayan personelde istisna Ocak seviyesinde kalıyordu. Takvim bazlı referansa çevrildi. Ek bulgu: `round2` (MidpointNearestEven) 4.211,325 tie'ında 4.211,32 üretiyordu → `round_gv_amount` (MidpointAwayFromZero) eklendi.

## Sonuç
- cargo test: 36/36 + smoke; bun test: 24/24; bun run build OK; lint: tek önceden var olan kapsam dışı hata (isPrimeGroup.test.ts `.not` typing).
- İstisnalar: Ocak 4.211,33 · Temmuz 4.537,75 · Ağustos 5.615,10 (motor türetir, hardcode yok).
- 12 dosya değişti (hepsi EDIT_SCOPE), commit yok, kullanıcı değişikliği (.gitignore) korundu.

## Notlar
- Dizin adı NFD/NFC ikizi üretti; NFC kopyası temizlendi, repo NFD: `/Users/kadir/Downloads/4_d-sürekli-i̇şçi-bordro-programı`.
- Açık kapı (kapsam dışı, raporlandı): isPrimeGroup.test.ts lint hatası; istenirse ayrı iş.
