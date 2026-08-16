import { describe, expect, test } from 'bun:test';
import { DönemselKurumDegerleri, PuantajOzeti } from '../types/payroll';
import {
  autoFillGelirlerFromPuantaj,
  DEFAULT_KURUM_DEGERLERI,
} from './payrollUtils';

describe('Tediye/TİS manual-only browser contract', () => {
  test('aktif legacy listeler browser helper içinde otomatik gelir üretmemeli', () => {
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      ...DEFAULT_KURUM_DEGERLERI,
      tediyeListesi: [
        {
          id: 1,
          ad: 'Legacy aktif Tediye',
          odemeAyi: 'Haziran',
          gunSayisi: 13,
          aktifDonemdeOdensin: true,
          sabitTutar: 9999,
        },
      ],
      tisIkramiyeListesi: [
        {
          id: 1,
          ad: 'Legacy aktif TİS',
          odemeAyi: 'Haziran',
          gunSayisi: 30,
          aktifDonemdeOdensin: true,
          sabitTutar: 8888,
        },
      ],
    };
    const puantaj: PuantajOzeti = {
      Ç: 1,
      T: 0,
      G: 0,
      İ: 0,
      GÇ: 0,
      GÇT: 0,
      R: 0,
    };

    const gelirler = autoFillGelirlerFromPuantaj(puantaj, kurum, 1, '1. Grup');
    expect(gelirler.tediye).toBe(null);
    expect(gelirler.tisIkramiyesi).toBe(null);
  });
});
