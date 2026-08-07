import { expect, test, describe } from 'bun:test';
import { calculatePreviousCumulativeGvMatrah, createBordroDonemi } from './payrollUtils';
import { BordroDonemi, BordroKaydi, Personel } from '../types/payroll';

describe('Kümülatif GV Matrahı Devri ve İlerlemesi Regression Testi', () => {
  const person: Personel = {
    id: 'test-p1',
    tcNo: '11111111111',
    ad: 'Ahmet',
    soyad: 'Test',
    grup: '1. Grup',
    sgkSicilNo: '12345',
    iban: 'TR000',
    hizmetYili: 5,
    devirKumulatifGvMatrahi: 120000,
    devirKumulatifGvMatrahiYili: 2026,
    devirKumulatifGvMatrahiBaslangicAyi: 5,
  };

  const mayis2026 = createBordroDonemi(2026, 5);
  const haziran2026 = createBordroDonemi(2026, 6);
  const temmuz2026 = createBordroDonemi(2026, 7);
  const ocak2027 = createBordroDonemi(2027, 1);

  const donemler: BordroDonemi[] = [mayis2026, haziran2026, temmuz2026, ocak2027];

  test('1. Mayıs 2026 önceki kümülatif matrahı başlangıç devir tutarı (120.000 TL) olmalıdır', () => {
    const prevGvMayis = calculatePreviousCumulativeGvMatrah('test-p1', mayis2026, [], donemler, person);
    expect(prevGvMayis).toBe(120000);
  });

  test('2. Haziran 2026 önceki kümülatif matrahı (120.000 devir + 65.000 Mayıs = 185.000 TL) olmalıdır', () => {
    const mayisBordro: BordroKaydi = {
      id: 'test-p1_2026-05',
      personelId: 'test-p1',
      donemId: '2026-05',
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 70000 } as any,
      gelirToplam: 70000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any, // 70000 - 5000 = 65000 net GV matrahı
      kesintiToplam: 5000,
      netOdeme: 65000,
      olusturulmaTarihi: '',
      sonGuncellemeTarihi: '',
    };

    const bordrolar = [mayisBordro];
    const prevGvHaziran = calculatePreviousCumulativeGvMatrah('test-p1', haziran2026, bordrolar, donemler, person);
    expect(prevGvHaziran).toBe(185000);
  });

  test('3. Temmuz 2026 önceki kümülatif matrahı (120.000 devir + 65.000 Mayıs + 70.000 Haziran = 255.000 TL) olmalıdır', () => {
    const mayisBordro: BordroKaydi = {
      id: 'test-p1_2026-05',
      personelId: 'test-p1',
      donemId: '2026-05',
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 70000 } as any,
      gelirToplam: 70000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any, // 65000 matrah
      kesintiToplam: 5000,
      netOdeme: 65000,
      olusturulmaTarihi: '',
      sonGuncellemeTarihi: '',
    };

    const haziranBordro: BordroKaydi = {
      id: 'test-p1_2026-06',
      personelId: 'test-p1',
      donemId: '2026-06',
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 75000 } as any,
      gelirToplam: 75000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any, // 75000 - 5000 = 70000 matrah
      kesintiToplam: 5000,
      netOdeme: 70000,
      olusturulmaTarihi: '',
      sonGuncellemeTarihi: '',
    };

    const bordrolar = [mayisBordro, haziranBordro];
    const prevGvTemmuz = calculatePreviousCumulativeGvMatrah('test-p1', temmuz2026, bordrolar, donemler, person);
    expect(prevGvTemmuz).toBe(255000);
  });

  test('4. 2027 Ocak önceki kümülatif matrahı 0 TL olmalıdır (2026 devri taşınmamalıdır)', () => {
    const mayisBordro: BordroKaydi = {
      id: 'test-p1_2026-05',
      personelId: 'test-p1',
      donemId: '2026-05',
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 70000 } as any,
      gelirToplam: 70000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any,
      kesintiToplam: 5000,
      netOdeme: 65000,
      olusturulmaTarihi: '',
      sonGuncellemeTarihi: '',
    };

    const bordrolar = [mayisBordro];
    const prevGvOcak2027 = calculatePreviousCumulativeGvMatrah('test-p1', ocak2027, bordrolar, donemler, person);
    expect(prevGvOcak2027).toBe(0);
  });

  test('5. Çakışma Senaryosu: Devirden önceki döneme (ör. Ocak 2026) ait kayıtlı bordro varken devir girilirse açık ÇAKIŞMA UYARISI hatası vermelidir', () => {
    const ocak2026 = createBordroDonemi(2026, 1);
    const donemlerWithOcak = [ocak2026, ...donemler];

    const ocakBordro: BordroKaydi = {
      id: 'test-p1_2026-01',
      personelId: 'test-p1',
      donemId: '2026-01',
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 40000 } as any,
      gelirToplam: 40000,
      kesintiler: { isciSgkPrimi: 3000, isciIssizlikPrimi: 300 } as any,
      kesintiToplam: 3300,
      netOdeme: 36700,
      olusturulmaTarihi: '',
      sonGuncellemeTarihi: '',
    };

    // Both devir for May (120,000) AND saved Jan 2026 bordro exist -> Conflict!
    expect(() => {
      calculatePreviousCumulativeGvMatrah('test-p1', mayis2026, [ocakBordro], donemlerWithOcak, person);
    }).toThrow('ÇAKIŞMA UYARISI');
  });
});
