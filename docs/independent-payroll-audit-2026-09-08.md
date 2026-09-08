# Bağımsız bordro denetimi — 8 Eylül 2026

Durum: denetim ve yeni regresyon doğrulamaları sürüyor. Önceki raporlar kanıt kabul edilmedi; mevcut testler kullanıcı talimatıyla çalıştırılmadı.

| Gereksinim | Sahip / akış | Beklenen / hata davranışı | Yeni kanıt |
|---|---|---|---|
| Gelir, puantaj, rapor | core calculations → payroll_engine statutory snapshot | Hak gününden gelir, normalize prim günü; geçersiz girdi reddi | independent_audit_20260908 |
| PEK, prim, BES | core normal / supplementary / retro branches | Tavan, devir, tek sabit kesinti, oranlı kesinti tutarlılığı | bağımsız senaryo ve regresyon |
| GV/DV/istisnalar | core tax functions → event snapshots | Artan tarife, yemek ve asgari istisnalarının ayrı uygulanması | bağımsız kuruş hesabı |
| Opening, yıl, ödeme sırası | previous_gv / previous_asgari_gv / incoming_devreden | Vergi yılı izolasyonu; aynı ay hak bir kez | zincir senaryoları |
| Retro / TİS | retro replay → allocation ledger → payment | Kaynak prim ve ödeme vergisi ayrımı | inceleme / yeni senaryolar |
| Persistence | native service → SQLite repositories; browser → IndexedDB CAS | Atomiklik, yeniden okuma, eski sonuçların geçersizleşmesi | yeni SQLite doğrulaması |

Mevzuat: yemek DV istisnası 322 sayılı GV Genel Tebliği, madde 4(6): https://resmigazete.gov.tr/eskiler/2022/12/20221230M2-12.htm . OKS ek ücret / ikramiye katkısı: https://www.egm.org.tr/isverenler/isveren-bilgilendirme-rehberi/ .
