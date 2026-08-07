import { expect, test, describe } from 'bun:test';
import {
  autoFillGelirlerFromPuantaj,
  calculateIsPrimiDetayi,
  DEFAULT_KURUM_DEGERLERI,
  getGrupIsPrimiOrani,
  resolveIsPrimiGrubu,
} from './payrollUtils';
import { IsPrimiGrupItem, PuantajOzeti } from '../types/payroll';

describe('Is Primi Grup/Oran Kaynagi ve Tek Yuvarlama (Denetim #6 ve #10)', () => {
  const aktifGruplar: IsPrimiGrupItem[] = [
    { id: '1. Grup', ad: '1. Grup', oran: 9, aktif: true },
    { id: '2. Grup', ad: '2. Grup', oran: 8, aktif: true },
    { id: '3. Grup', ad: '3. Grup', oran: 7, aktif: true },
  ];

  test('oran kaynagi yalnizca personel grubudur; isPrimiYuzde yok sayilir', () => {
    const kurum = {
      ...DEFAULT_KURUM_DEGERLERI,
      donemId: '2026-05',
      gunlukTabanUcret: 1000,
      isPrimiYuzde: 50,
      isPrimiGruplari: [{ id: '2. Grup', ad: '2. Grup', oran: 8, aktif: true }],
    };
    const puantaj: PuantajOzeti = { Ç: 26, T: 0, G: 0, İ: 0, GÇ: 4, GÇT: 0, R: 0 };
    const gelirler = autoFillGelirlerFromPuantaj(puantaj, kurum, 0, '2. Grup');
    expect(gelirler.isPrimi).toBe(2400);
  });

  test('tek final yuvarlama: 27.55 taban %3 3 gun = 2.48 (cift yuvarlama 2.49 degil)', () => {
    const ucGruplar: IsPrimiGrupItem[] = [
      { id: '1. Grup', ad: '1. Grup', oran: 3, aktif: true },
    ];
    const detay = calculateIsPrimiDetayi(27.55, 3, '1. Grup', ucGruplar);
    expect(detay.tutar).toBe(2.48);
    expect(detay.gunlukIsPrimi).toBe(0.83);
    expect(detay.tutar).not.toBe(2.49);
  });

  test('hak gunu: yalnizca C + GC; GCT ve T dahil edilmez', () => {
    const detay = calculateIsPrimiDetayi(1000, 26, '1. Grup', aktifGruplar);
    expect(detay.hakGunu).toBe(26);
    const kurum = {
      ...DEFAULT_KURUM_DEGERLERI,
      donemId: '2026-05',
      gunlukTabanUcret: 1000,
      isPrimiGruplari: aktifGruplar,
    };
    const puantaj: PuantajOzeti = { Ç: 24, T: 2, G: 0, İ: 0, GÇ: 2, GÇT: 2, R: 0 };
    const gelirler = autoFillGelirlerFromPuantaj(puantaj, kurum, 0, '1. Grup');
    expect(gelirler.isPrimi).toBe(2340);
  });

  test('tanimsiz grup acik hata firlatir; sessiz %9 fallback yok', () => {
    expect(() => resolveIsPrimiGrubu(undefined, aktifGruplar)).toThrow(/Personelin iş primi grubu/);
    expect(() => getGrupIsPrimiOrani('5. Grup', aktifGruplar)).toThrow(/geçersiz/);
    const pasif: IsPrimiGrupItem[] = [
      { id: '1. Grup', ad: '1. Grup', oran: 9, aktif: false },
    ];
    expect(() => getGrupIsPrimiOrani('1. Grup', pasif)).toThrow(/pasif/i);
  });

  test('1. Grup %9: 1000 taban 30 hak gunu = 2700', () => {
    const kurum = {
      ...DEFAULT_KURUM_DEGERLERI,
      donemId: '2026-05',
      gunlukTabanUcret: 1000,
      isPrimiGruplari: aktifGruplar,
    };
    const puantaj: PuantajOzeti = { Ç: 26, T: 0, G: 0, İ: 0, GÇ: 4, GÇT: 0, R: 0 };
    const detay = calculateIsPrimiDetayi(
      kurum.gunlukTabanUcret,
      30,
      '1. Grup',
      aktifGruplar
    );
    expect(detay.grupAd).toBe('1. Grup');
    expect(detay.oran).toBe(9);
    expect(detay.tutar).toBe(2700);
    const gelirler = autoFillGelirlerFromPuantaj(puantaj, kurum, 0, '1. Grup');
    expect(gelirler.isPrimi).toBe(2700);
  });
});
