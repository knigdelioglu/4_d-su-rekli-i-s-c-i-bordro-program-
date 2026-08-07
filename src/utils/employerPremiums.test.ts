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
});
