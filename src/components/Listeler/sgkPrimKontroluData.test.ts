import { describe, expect, test } from 'bun:test';
import type { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import {
  amountToKurus,
  compareSgkPrimTotals,
  getSgkPrimKontroluRows,
  getSgkPrimKontroluTotals,
  parseSgkTutarToKurus,
} from './sgkPrimKontroluData';

const period: BordroDonemi = {
  id: '2026-09',
  yil: 2026,
  ay: 9,
  baslangicTarihi: '2026-09-15',
  bitisTarihi: '2026-10-14',
  donemAdi: 'Eylül 2026',
  taxYear: 2026,
  taxMonth: 10,
};

const person1 = {
  id: 'p-1',
  tcNo: '10000000001',
  ad: 'Ali',
  soyad: 'Yılmaz',
  sgkSicilNo: 'SGK-1',
} as Personel;

const person2 = {
  id: 'p-2',
  tcNo: '10000000002',
  ad: 'Ayşe',
  soyad: 'Kaya',
  sgkSicilNo: 'SGK-2',
} as Personel;

const person3 = {
  id: 'p-3',
  tcNo: '10000000003',
  ad: 'Mehmet',
  soyad: 'Demir',
  sgkSicilNo: 'SGK-3',
} as Personel;

function payroll(
  personelId: string,
  accrualId: string,
  status: BordroKaydi['status'],
  amounts: {
    isverenSgkPrimi?: number;
    isverenIssizlikPrimi?: number;
    isciSgkPrimi?: number;
    isciIssizlikPrimi?: number;
  }
): BordroKaydi {
  return {
    id: accrualId,
    personelId,
    donemId: period.id,
    accrualId,
    accrualType: 'NORMAL',
    paymentDate: '2026-10-13',
    sequence: 0,
    status,
    pekDetay: {
      isverenSgkPrimi: amounts.isverenSgkPrimi,
      isverenIssizlikPrimi: amounts.isverenIssizlikPrimi,
    },
    kesintiler: {
      isciSgkPrimi: amounts.isciSgkPrimi ?? null,
      isciIssizlikPrimi: amounts.isciIssizlikPrimi ?? null,
    },
  } as unknown as BordroKaydi;
}

describe('SGK prim kontrolü dataset', () => {
  test('tek authoritative NORMAL bordroyu dört prim alanıyla gösterir', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'normal-1', 'CALCULATED', {
        isverenSgkPrimi: 217.5,
        isverenIssizlikPrimi: 20,
        isciSgkPrimi: 140,
        isciIssizlikPrimi: 10,
      }),
    ]);

    expect(rows.length).toBe(1);
    expect({
      status: rows[0].status,
      isverenSgkPrimi: rows[0].isverenSgkPrimi,
      isverenIssizlikPrimi: rows[0].isverenIssizlikPrimi,
      isciSgkPrimi: rows[0].isciSgkPrimi,
      isciIssizlikPrimi: rows[0].isciIssizlikPrimi,
      toplam: rows[0].toplam,
    }).toEqual({
      status: 'authoritative',
      isverenSgkPrimi: 217.5,
      isverenIssizlikPrimi: 20,
      isciSgkPrimi: 140,
      isciIssizlikPrimi: 10,
      toplam: 387.5,
    });
    expect(getSgkPrimKontroluTotals(rows).genelPrimToplami).toBe(387.5);
  });

  test('aynı kişinin NORMAL ve TEDIYE authoritative tahakkuklarını tek satırda toplar', () => {
    const tediye = payroll('p-1', 'tediye-1', 'FINALIZED', {
      isverenSgkPrimi: 50,
      isverenIssizlikPrimi: 5,
      isciSgkPrimi: 30,
      isciIssizlikPrimi: 2,
    });
    tediye.accrualType = 'TEDIYE';

    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'normal-1', 'CALCULATED', {
        isverenSgkPrimi: 217.5,
        isverenIssizlikPrimi: 20,
        isciSgkPrimi: 140,
        isciIssizlikPrimi: 10,
      }),
      tediye,
    ]);

    expect(rows.length).toBe(1);
    expect({
      isverenSgkPrimi: rows[0].isverenSgkPrimi,
      isverenIssizlikPrimi: rows[0].isverenIssizlikPrimi,
      isciSgkPrimi: rows[0].isciSgkPrimi,
      isciIssizlikPrimi: rows[0].isciIssizlikPrimi,
      toplam: rows[0].toplam,
    }).toEqual({
      isverenSgkPrimi: 267.5,
      isverenIssizlikPrimi: 25,
      isciSgkPrimi: 170,
      isciIssizlikPrimi: 12,
      toplam: 474.5,
    });
  });

  test('birden fazla personelin kolon toplamlarını doğru üretir', () => {
    const rows = getSgkPrimKontroluRows(period, [person1, person2], [
      payroll('p-1', 'normal-1', 'CALCULATED', {
        isverenSgkPrimi: 100,
        isverenIssizlikPrimi: 10,
        isciSgkPrimi: 70,
        isciIssizlikPrimi: 5,
      }),
      payroll('p-2', 'normal-2', 'FINALIZED', {
        isverenSgkPrimi: 200,
        isverenIssizlikPrimi: 20,
        isciSgkPrimi: 140,
        isciIssizlikPrimi: 10,
      }),
    ]);

    expect(getSgkPrimKontroluTotals(rows)).toEqual({
      isverenSgkPrimi: 300,
      isverenIssizlikPrimi: 30,
      isciSgkPrimi: 210,
      isciIssizlikPrimi: 15,
      genelPrimToplami: 555,
    });
  });

  test('DRAFT ve STALE toplamdan çıkarılır; tüm personel uyarıyla görünür', () => {
    const rows = getSgkPrimKontroluRows(period, [person1, person2, person3], [
      payroll('p-1', 'draft-1', 'DRAFT', {
        isverenSgkPrimi: 100,
        isverenIssizlikPrimi: 10,
        isciSgkPrimi: 70,
        isciIssizlikPrimi: 5,
      }),
      payroll('p-2', 'stale-2', 'STALE', {
        isverenSgkPrimi: 200,
        isverenIssizlikPrimi: 20,
        isciSgkPrimi: 140,
        isciIssizlikPrimi: 10,
      }),
    ]);

    expect(rows.map((row) => row.status)).toEqual(['notCalculated', 'stale', 'notCalculated']);
    expect(rows.map((row) => row.toplam)).toEqual([0, 0, 0]);
    expect(getSgkPrimKontroluTotals(rows).genelPrimToplami).toBe(0);
  });
});

describe('SGK prim kontrolü kuruş ve karşılaştırma sınırı', () => {
  test('kuruş toplamı binary floating point sapmasını taşımaz', () => {
    expect(amountToKurus(0.1 + 0.2)).toBe(30);
    expect(parseSgkTutarToKurus('757.876,39')).toBe(75787639);
  });

  test('aynı tutar Uyumlu sonucunu verir', () => {
    expect(compareSgkPrimTotals(1000, '1.000,00')).toEqual({
      status: 'compatible',
      sgkTutarKurus: 100000,
      farkKurus: 0,
    });
  });

  test('program toplamı 1000, SGK tutarı 990 olduğunda fark +10 olur', () => {
    expect(compareSgkPrimTotals(1000, '990,00')).toEqual({
      status: 'programHigher',
      sgkTutarKurus: 99000,
      farkKurus: 1000,
    });
  });

  test('program toplamı 990, SGK tutarı 1000 olduğunda fark -10 olur', () => {
    expect(compareSgkPrimTotals(990, '1.000,00')).toEqual({
      status: 'programLower',
      sgkTutarKurus: 100000,
      farkKurus: -1000,
    });
  });

  test('boş ve geçersiz SGK girdileri karşılaştırmayı çalıştırmaz', () => {
    expect(compareSgkPrimTotals(1000, '')).toEqual({
      status: 'empty',
      sgkTutarKurus: null,
      farkKurus: null,
    });
    expect(compareSgkPrimTotals(1000, 'abc')).toEqual({
      status: 'invalid',
      sgkTutarKurus: null,
      farkKurus: null,
    });
  });
});
