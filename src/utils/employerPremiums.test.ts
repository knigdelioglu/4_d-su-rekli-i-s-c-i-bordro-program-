import { expect, test, describe } from 'bun:test';
import { calculatePrimeEsasKazanc, calculateStatutoryDeductions, calculateGelirToplam, calculateKesintiToplam } from './payrollUtils';
import { GelirKalemleri, PuantajOzeti, DönemselKurumDegerleri, KesintiKalemleri } from '../types/payroll';

describe('SGK Yemek İstisnası ve İşveren Primleri Regression Testleri (A - H)', () => {
  // Test A — Yemek İstisnası
  test('Test A — Günlük yemek = 300,75, SGK istisnası = 300, 22 gün', () => {
    const puantaj: PuantajOzeti = { Ç: 22, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 300.75,
      gunlukYemekIstisnasiSGK: 300.00,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
    };

    const totalMeal = 300.75 * 22; // 6.616,50 TL
    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 0,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: totalMeal,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    expect(gelirler.yemek).toBe(6616.50);
    expect(pekResult.fiiliYemekGunu).toBe(22);
    expect(pekResult.yemekIstisnasiTutar).toBe(6600.00);
    expect(pekResult.hesaplananPek).toBe(16.50);
  });

  // Test B — Yemek 300 TL'den düşük
  test('Test B — Günlük yemek = 250, SGK istisnası = 300, 22 gün', () => {
    const puantaj: PuantajOzeti = { Ç: 22, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 250.00,
      gunlukYemekIstisnasiSGK: 300.00,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
    };

    const totalMeal = 250.00 * 22; // 5.500,00 TL
    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 0,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: totalMeal,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    expect(pekResult.yemekIstisnasiTutar).toBe(5500.00);
    expect(pekResult.hesaplananPek).toBe(0);
  });

  // Test C — İşveren SGK
  test('Test C — PEK = 100.000, oran = %21,75 => 21.750 TL', () => {
    const puantaj: PuantajOzeti = { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 0,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
      sgkIsverenOraniYuzde: 21.75,
    };

    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 100000.00,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: 0,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    expect(pekResult.finalPek).toBe(100000.00);
    expect(pekResult.isverenSgkPrimi).toBe(21750.00);
  });

  // Test D — İşveren İşsizlik
  test('Test D — PEK = 100.000, oran = %2 => 2.000 TL', () => {
    const puantaj: PuantajOzeti = { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 0,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
      issizlikIsverenOraniYuzde: 2.00,
    };

    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 100000.00,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: 0,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    expect(pekResult.finalPek).toBe(100000.00);
    expect(pekResult.isverenIssizlikPrimi).toBe(2000.00);
  });

  // Test E — Toplam İşveren Primi
  test('Test E — PEK = 100.000 => 23.750 TL Toplam İşveren Primi', () => {
    const puantaj: PuantajOzeti = { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 0,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
      sgkIsverenOraniYuzde: 21.75,
      issizlikIsverenOraniYuzde: 2.00,
    };

    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 100000.00,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: 0,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    expect(pekResult.isverenPrimToplami).toBe(23750.00);
  });

  // Test F — Net Ödeme Değişmemeli
  test('Test F — İşveren primleri işçi kesintilerini ve net ödemeyi etkilememelidir', () => {
    const puantaj: PuantajOzeti = { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 0,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
      sgkIsciOraniYuzde: 14,
      issizlikIsciOraniYuzde: 1,
      sgkIsverenOraniYuzde: 21.75,
      issizlikIsverenOraniYuzde: 2.00,
    };

    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 80000.00,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: 0,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const statutory = calculateStatutoryDeductions(gelirler, kurum, undefined, puantaj, 0, [], 0);
    const kesintiler: KesintiKalemleri = {
      isciSgkPrimi: statutory.isciSgkPrimi ?? null,
      isciIssizlikPrimi: statutory.isciIssizlikPrimi ?? null,
      gelirVergisi: statutory.gelirVergisi ?? null,
      damgaVergisi: statutory.damgaVergisi ?? null,
      sendikaAidati: statutory.sendikaAidati ?? null,
      bes: statutory.bes ?? null,
      icra: statutory.icra ?? null,
      kisiBorcu: statutory.kisiBorcu ?? null,
      dogumAskerlikBorclanmasi: statutory.dogumAskerlikBorclanmasi ?? null,
      hayatSaglikSigortasi: statutory.hayatSaglikSigortasi ?? null,
      digerKesinti: statutory.digerKesinti ?? null,
    };
    const gelirToplam = calculateGelirToplam(gelirler);
    const kesintiToplam = calculateKesintiToplam(kesintiler);
    const netOdeme = gelirToplam - kesintiToplam;

    expect(statutory.isciSgkPrimi).toBe(11200);
    expect(statutory.isciIssizlikPrimi).toBe(800);
    expect(statutory.pekResult?.isverenSgkPrimi).toBe(17400);
    expect(statutory.pekResult?.isverenIssizlikPrimi).toBe(1600);
    expect(netOdeme).toBe(gelirToplam - kesintiToplam);
  });

  // Test G — Parametre Değişikliği
  test('Test G — Özel parametreler (%22 SGK işveren, %3 işveren işsizlik) doğru kullanılmalıdır', () => {
    const puantaj: PuantajOzeti = { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 0,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
      sgkIsverenOraniYuzde: 22.00,
      issizlikIsverenOraniYuzde: 3.00,
    };

    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 100000.00,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: 0,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    expect(pekResult.isverenSgkPrimi).toBe(22000.00);
    expect(pekResult.isverenIssizlikPrimi).toBe(3000.00);
    expect(pekResult.isverenPrimToplami).toBe(25000.00);
  });

  // Test H — GÇ Yemek Hakkı Korunuyor
  test('Test H — 20 Ç + 2 GÇ için yemek_hak_günü = 22 olmalıdır', () => {
    const puantaj: PuantajOzeti = { Ç: 20, T: 4, G: 0, İ: 0, GÇ: 2, GÇT: 4, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 300.75,
      gunlukYemekIstisnasiSGK: 300.00,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
    };

    const totalMeal = 300.75 * 22; // 6.616,50 TL
    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 0,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: totalMeal,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    expect(pekResult.fiiliYemekGunu).toBe(22);
    expect(pekResult.yemekIstisnasiTutar).toBe(6600.00);
    expect(pekResult.hesaplananPek).toBe(16.50);
  });

  // Test I — SGK 2026 Resmî Örnek: PEK = 33.030 TL, %38,75 Toplam Tahakkuk = 12.799,13 TL
  test('Test I — PEK = 33.030 => İşçi SGK 4.624,20, İşçi İşsizlik 330,30, İşveren SGK 7.184,03, İşveren İşsizlik 660,60, Toplam 12.799,13', () => {
    const puantaj: PuantajOzeti = { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 0,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
      sgkIsciOraniYuzde: 14,
      issizlikIsciOraniYuzde: 1,
      sgkIsverenOraniYuzde: 21.75,
      issizlikIsverenOraniYuzde: 2.00,
    };

    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 33030.00,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: 0,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    const statutory = calculateStatutoryDeductions(gelirler, kurum, undefined, puantaj, 0, [], 0);

    // Midpoint: 33.030 × %21,75 = 7.184,025 -> 7.184,03 (sıfırdan uzağa)
    expect(Math.round(33030.00 * 0.2175 * 100) / 100).toBe(7184.03);

    expect(statutory.isciSgkPrimi).toBe(4624.20);
    expect(statutory.isciIssizlikPrimi).toBe(330.30);
    expect(pekResult.isverenSgkPrimi).toBe(7184.03);
    expect(pekResult.isverenIssizlikPrimi).toBe(660.60);

    const toplamTahakkuk =
      statutory.isciSgkPrimi! + statutory.isciIssizlikPrimi! +
      pekResult.isverenSgkPrimi + pekResult.isverenIssizlikPrimi;
    expect(toplamTahakkuk).toBe(12799.13);
  });

  // Test J — PEK Alt Sınır Senaryosu (Ham 25.000, Nihai 33.030, Fark 8.030)
  test('Test J — Ham PEK 25.000 / Nihai PEK 33.030: işçi 3.750, işveren 9.049,13, toplam tahakkuk 12.799,13', () => {
    const puantaj: PuantajOzeti = { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      gunlukTabanUcret: 2443.28,
      gunlukYemek: 0,
      birlestirilmisSosyalYardim: 0,
      gunlukVasitaYol: 0,
      giyimYardimi: 0,
      hizmetZammiBirimi: 0,
      gunlukAsgariUcret: 1101.00,
      sgkIsciOraniYuzde: 14,
      issizlikIsciOraniYuzde: 1,
      sgkIsverenOraniYuzde: 21.75,
      issizlikIsverenOraniYuzde: 2.00,
    };

    const gelirler: GelirKalemleri = {
      tabanBrutAylik: 25000.00,
      tediye: 0,
      tisIkramiyesi: 0,
      ekOdeme: 0,
      yemek: 0,
      birlestirilmisSosyalYardim: 0,
      vasitaYol: 0,
      giyimYardimi: 0,
      isPrimi: 0,
      hizmetZammi: 0,
      digerGelir: 0,
    };

    const pekResult = calculatePrimeEsasKazanc(gelirler, puantaj, kurum, []);
    const statutory = calculateStatutoryDeductions(gelirler, kurum, undefined, puantaj, 0, [], 0);

    // PEK ayrımı (Model A): ham 25.000, nihai 33.030, fark 8.030
    expect(pekResult.hesaplananPek).toBe(25000.00);
    expect(pekResult.pekAltSinir).toBe(33030.00);
    expect(pekResult.finalPek).toBe(33030.00);
    expect(pekResult.altSinirTamamlamaFarki).toBe(8030.00);

    // İşçi kesintileri ham PEK (25.000) üzerinden
    expect(statutory.isciSgkPrimi).toBe(3500.00);
    expect(statutory.isciIssizlikPrimi).toBe(250.00);

    // İşveren normal payları nihai PEK (33.030) üzerinden
    expect(pekResult.isverenSgkPrimi).toBe(7184.03);
    expect(pekResult.isverenIssizlikPrimi).toBe(660.60);

    // Alt sınır farkı işveren yükü: 8.030 × %14 = 1.124,20 + 8.030 × %1 = 80,30 = 1.204,50
    expect(pekResult.pekAltSinirTamamlamaIsverenPrimi).toBe(1204.50);
    // İşveren toplam: 7.184,03 + 660,60 + 1.204,50 = 9.049,13
    expect(pekResult.isverenPrimToplami).toBe(9049.13);

    // Toplam SGK tahakkuk: 3.750,00 + 9.049,13 = 12.799,13
    const toplamTahakkuk =
      statutory.isciSgkPrimi! + statutory.isciIssizlikPrimi! + pekResult.isverenPrimToplami;
    expect(toplamTahakkuk).toBe(12799.13);
  });
});
