import { expect, test, describe } from 'bun:test';
import {
  calculateAylikAsgariUcretGvMatrahi,
  calculateGvHesapDetayi,
  calculatePreviousCumulativeAsgariUcretGvMatrah,
  calculatePreviousCumulativeGvMatrah,
  createBordroDonemi,
} from './payrollUtils';
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
      accrualId: 'test-p1_2026-05',
      accrualType: 'NORMAL',
      paymentDate: '2026-06-14',
      sequence: 0,
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 70000 } as any,
      gelirToplam: 70000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any, // 70000 - 5000 = 65000 net GV matrahı
      kesintiToplam: 5000,
      netOdeme: 65000,
      status: 'CALCULATED',
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
      accrualId: 'test-p1_2026-05',
      accrualType: 'NORMAL',
      paymentDate: '2026-06-14',
      sequence: 0,
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 70000 } as any,
      gelirToplam: 70000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any, // 65000 matrah
      kesintiToplam: 5000,
      netOdeme: 65000,
      status: 'CALCULATED',
      olusturulmaTarihi: '',
      sonGuncellemeTarihi: '',
    };

    const haziranBordro: BordroKaydi = {
      id: 'test-p1_2026-06',
      personelId: 'test-p1',
      donemId: '2026-06',
      accrualId: 'test-p1_2026-06',
      accrualType: 'NORMAL',
      paymentDate: '2026-07-14',
      sequence: 0,
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 75000 } as any,
      gelirToplam: 75000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any, // 75000 - 5000 = 70000 matrah
      kesintiToplam: 5000,
      netOdeme: 70000,
      status: 'CALCULATED',
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
      accrualId: 'test-p1_2026-05',
      accrualType: 'NORMAL',
      paymentDate: '2026-06-14',
      sequence: 0,
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 70000 } as any,
      gelirToplam: 70000,
      kesintiler: { isciSgkPrimi: 4500, isciIssizlikPrimi: 500 } as any,
      kesintiToplam: 5000,
      netOdeme: 65000,
      status: 'CALCULATED',
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
      accrualId: 'test-p1_2026-01',
      accrualType: 'NORMAL',
      paymentDate: '2026-02-14',
      sequence: 0,
      puantajOzeti: { Ç: 30, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
      gelirler: { tabanBrutAylik: 40000 } as any,
      gelirToplam: 40000,
      kesintiler: { isciSgkPrimi: 3000, isciIssizlikPrimi: 300 } as any,
      kesintiToplam: 3300,
      netOdeme: 36700,
      status: 'CALCULATED',
      olusturulmaTarihi: '',
      sonGuncellemeTarihi: '',
    };

    expect(() => {
      calculatePreviousCumulativeGvMatrah('test-p1', mayis2026, [ocakBordro], donemlerWithOcak, person);
    }).toThrow('ÇAKIŞMA UYARISI');
  });
});

describe('Asgari Ücret GV İstisnası (takvim referansı) Regression Testi', () => {
  const aylik = 28075.5;

  test('A. Ocak — aylık matrah 28.075,50 ve istisna 4.211,33 TL', () => {
    const aylikMatrah = calculateAylikAsgariUcretGvMatrahi(1101, 0.14, 0.01);
    expect(aylikMatrah).toBeCloseTo(28075.5, 2);

    const det = calculateGvHesapDetayi(28075.5, 0, aylikMatrah, 0);
    expect(det.asgariUcretGvMatrahi).toBeCloseTo(28075.5, 2);
    expect(det.asgariUcretGvIstisnasi).toBeCloseTo(4211.33, 2);
    expect(det.uygulananGvIstisnasi).toBeCloseTo(4211.33, 2);
    expect(det.kesilenGelirVergisi).toBeCloseTo(0, 2);
  });

  test('B. Temmuz — referans kümülatif takvimden (6 × 28.075,50), istisna 4.537,75 TL', () => {
    const det = calculateGvHesapDetayi(aylik, 0, aylik, aylik * 6);
    expect(det.asgariUcretReferansKumulatifMatrahi).toBeCloseTo(196528.5, 2);
    expect(det.asgariUcretGvIstisnasi).toBeCloseTo(4537.75, 2);
  });

  test('C. Ağustos — referans 7 × 28.075,50, istisna 5.615,10 TL', () => {
    const det = calculateGvHesapDetayi(aylik, 0, aylik, aylik * 7);
    expect(det.asgariUcretReferansKumulatifMatrahi).toBeCloseTo(224604, 2);
    expect(det.asgariUcretGvIstisnasi).toBeCloseTo(5615.1, 2);
  });

  test('D. Gerçek kümülatif aynı ay istisneyi değiştirmez; referans takvimden gelir', () => {
    const low = calculateGvHesapDetayi(65000, 5000, aylik, 0);
    const high = calculateGvHesapDetayi(65000, 300000, aylik, 0);
    expect(low.asgariUcretGvIstisnasi).toBeCloseTo(high.asgariUcretGvIstisnasi, 2);
    expect(low.brutGelirVergisi).toBeLessThan(high.brutGelirVergisi);
  });
});

describe('taxMonth / taxYear (ödeme-tahakkuk ayı) Kabul Kriteri', () => {
  test('A. createBordroDonemi varsayılan taxMonth = bitiş ayı (ay+1), taxYear aynı', () => {
    const haziran = createBordroDonemi(2026, 6);
    expect(haziran.ay).toBe(6);
    expect(haziran.taxMonth).toBe(7);
    expect(haziran.taxYear).toBe(2026);
  });

  test('B. Aralık dönemi varsayılan taxMonth=1, taxYear=yıl+1; dönem id/ad değişmez', () => {
    const aralik = createBordroDonemi(2026, 12);
    expect(aralik.id).toBe('2026-12');
    expect(aralik.ay).toBe(12);
    expect(aralik.taxMonth).toBe(1);
    expect(aralik.taxYear).toBe(2027);
  });

  test('C. Kullanıcı üstüne yazmışsa taxMonth korunur', () => {
    const donem = createBordroDonemi(2026, 6, 2026, 6);
    expect(donem.ay).toBe(6);
    expect(donem.taxMonth).toBe(6);
    expect(donem.taxYear).toBe(2026);
    expect(donem.id).toBe('2026-06');
  });

  test('D. 15.06–14.07 taxMonth=7 → 196.528,50 / 4.537,75; taxMonth=6 → 168.453,00 / 4.211,33', () => {
    const aylik = 28075.5;

    const d7 = calculateGvHesapDetayi(aylik, 0, aylik, aylik * 6);
    expect(d7.asgariUcretReferansKumulatifMatrahi).toBeCloseTo(196528.5, 2);
    expect(d7.asgariUcretGvIstisnasi).toBeCloseTo(4537.75, 2);

    const d6 = calculateGvHesapDetayi(aylik, 0, aylik, aylik * 5);
    expect(d6.asgariUcretReferansKumulatifMatrahi).toBeCloseTo(168453, 2);
    expect(d6.asgariUcretGvIstisnasi).toBeCloseTo(4211.33, 2);
  });
});
