import { describe, expect, test } from 'bun:test';
import type {
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  RetroAdjustmentBatch,
  RetroAllocation,
} from '../../types/payroll';
import {
  amountToKurus,
  buildSgkPrimKontroluExcelPayload,
  compareSgkPrimTotals,
  getSgkPrimKontroluRows,
  getSgkPrimKontroluRateLabels,
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

const completeAmounts = {
  isverenSgkPrimi: 100,
  isverenIssizlikPrimi: 20,
  isciSgkPrimi: 70,
  isciIssizlikPrimi: 10,
};

function payroll(
  personelId: string,
  accrualId: string,
  status: BordroKaydi['status'],
  amounts: {
    isverenSgkPrimi?: number | null;
    isverenIssizlikPrimi?: number | null;
    isciSgkPrimi?: number | null;
    isciIssizlikPrimi?: number | null;
    pekAltSinirTamamlamaIsverenPrimi?: number | null;
    sgkIsverenOraniYuzde?: number | null;
    isverenIssizlikOraniYuzde?: number | null;
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
      pekAltSinirTamamlamaIsverenPrimi: amounts.pekAltSinirTamamlamaIsverenPrimi,
      sgkIsverenOraniYuzde: amounts.sgkIsverenOraniYuzde,
      isverenIssizlikOraniYuzde: amounts.isverenIssizlikOraniYuzde,
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
      pekAltSinirTamamlamaIsverenPrimi: 0,
      dortPrimToplami: 555,
      sgkMutabakatToplami: 555,
      genelPrimToplami: 555,
      hazirOlmayanPersonelSayisi: 0,
      reconciliationReady: true,
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

    expect(rows.map((row) => row.status)).toEqual(['draft', 'stale', 'notCalculated']);
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

describe('SGK prim kontrolü güvenilirlik sınırı', () => {
  test('kaynak bordro yokken retro ledger tek başına SGK mutabakatını authoritative yapmaz', () => {
    const batch: RetroAdjustmentBatch = {
      id: 'retro-batch-without-source',
      revisionId: 'revision-1',
      personnelId: 'p-1',
      paymentDate: '2026-10-05',
      status: 'CALCULATED',
      settlementStatus: 'UNSETTLED',
      totalGrossDelta: 100,
    };
    const allocation: RetroAllocation = {
      id: 'retro-allocation-without-source',
      batchId: batch.id,
      personnelId: 'p-1',
      sourcePeriodId: period.id,
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: 0,
      previousAuthoritativeRetroAmount: 0,
      targetAmount: 100,
      deltaAmount: 100,
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
      originalPek: 1000,
      retroPekDelta: 100,
      adjustedPek: 1100,
      workerSgkDelta: 14,
      workerUnemploymentDelta: 1,
      employerSgkDelta: 20,
      employerUnemploymentDelta: 2,
    };

    const rows = getSgkPrimKontroluRows(
      period,
      [person1],
      [],
      [batch],
      [allocation]
    );

    expect(rows[0].status).toBe('notCalculated');
    expect(rows[0].toplam).toBe(0);
    expect(getSgkPrimKontroluTotals(rows).reconciliationReady).toBe(false);
  });

  test('authoritative kaynak snapshot üzerine retro kaynak PEK ve prim ledgerı eklenir', () => {
    const batch: RetroAdjustmentBatch = {
      id: 'retro-batch-source-ledger',
      revisionId: 'revision-1',
      personnelId: 'p-1',
      paymentDate: '2026-10-05',
      status: 'FINALIZED',
      settlementStatus: 'PAID',
      totalGrossDelta: 100,
    };
    const allocation: RetroAllocation = {
      id: 'retro-allocation-source-ledger',
      batchId: batch.id,
      personnelId: 'p-1',
      sourcePeriodId: period.id,
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: 0,
      previousAuthoritativeRetroAmount: 0,
      targetAmount: 100,
      deltaAmount: 100,
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
      originalPek: 1000,
      retroPekDelta: 100,
      adjustedPek: 1100,
      workerSgkDelta: 14,
      workerUnemploymentDelta: 1,
      employerSgkDelta: 20,
      employerUnemploymentDelta: 2,
    };
    const sourcePayroll = payroll('p-1', 'source-payroll', 'FINALIZED', {
      isverenSgkPrimi: 100,
      isverenIssizlikPrimi: 10,
      isciSgkPrimi: 70,
      isciIssizlikPrimi: 5,
    });

    const rows = getSgkPrimKontroluRows(
      period,
      [person1],
      [sourcePayroll],
      [batch],
      [allocation]
    );

    expect(rows[0].status).toBe('authoritative');
    expect({
      retroPekDelta: rows[0].retroPekDelta,
      isverenSgkPrimi: rows[0].isverenSgkPrimi,
      isciSgkPrimi: rows[0].isciSgkPrimi,
      toplam: rows[0].toplam,
    }).toEqual({
      retroPekDelta: 100,
      isverenSgkPrimi: 120,
      isciSgkPrimi: 84,
      toplam: 222,
    });
  });

  test('FINALIZED + STALE aynı kişiyi güvenilmez yapar ve authoritative kısmı toplamaz', () => {
    const staleTediye = payroll('p-1', 'tediye-stale', 'STALE', completeAmounts);
    staleTediye.accrualType = 'TEDIYE';

    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'normal-finalized', 'FINALIZED', completeAmounts),
      staleTediye,
    ]);

    expect(rows[0].status).toBe('stale');
    expect(rows[0].toplam).toBe(0);
    expect({
      sgkMutabakatToplami: getSgkPrimKontroluTotals(rows).sgkMutabakatToplami,
      hazirOlmayanPersonelSayisi: getSgkPrimKontroluTotals(rows).hazirOlmayanPersonelSayisi,
    }).toEqual({
      sgkMutabakatToplami: 0,
      hazirOlmayanPersonelSayisi: 1,
    });
  });

  test('FINALIZED + DRAFT aynı kişiyi toplama dahil etmez', () => {
    const draftTediye = payroll('p-1', 'tediye-draft', 'DRAFT', completeAmounts);
    draftTediye.accrualType = 'TEDIYE';

    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'normal-finalized', 'FINALIZED', completeAmounts),
      draftTediye,
    ]);

    expect(rows[0].status).toBe('draft');
    expect(rows[0].toplam).toBe(0);
    expect(getSgkPrimKontroluTotals(rows).sgkMutabakatToplami).toBe(0);
  });

  test('CALCULATED ama işveren SGK snapshot alanı eksikse missingSnapshot olur', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'missing-employer-sgk', 'CALCULATED', {
        isverenIssizlikPrimi: 20,
        isciSgkPrimi: 70,
        isciIssizlikPrimi: 10,
      }),
    ]);

    expect(rows[0].status).toBe('missingSnapshot');
    expect(getSgkPrimKontroluTotals(rows).sgkMutabakatToplami).toBe(0);
  });

  test('CALCULATED ama işveren işsizlik snapshot alanı eksikse missingSnapshot olur', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'missing-employer-unemployment', 'CALCULATED', {
        isverenSgkPrimi: 100,
        isciSgkPrimi: 70,
        isciIssizlikPrimi: 10,
      }),
    ]);

    expect(rows[0].status).toBe('missingSnapshot');
    expect(getSgkPrimKontroluTotals(rows).sgkMutabakatToplami).toBe(0);
  });

  test('CALCULATED ama işçi SGK snapshot alanı null ise missingSnapshot olur', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'missing-worker-sgk', 'CALCULATED', {
        isverenSgkPrimi: 100,
        isverenIssizlikPrimi: 20,
        isciSgkPrimi: null,
        isciIssizlikPrimi: 10,
      }),
    ]);

    expect(rows[0].status).toBe('missingSnapshot');
    expect(getSgkPrimKontroluTotals(rows).sgkMutabakatToplami).toBe(0);
  });

  test('CALCULATED ama işçi işsizlik snapshot alanı null ise missingSnapshot olur', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'missing-worker-unemployment', 'CALCULATED', {
        isverenSgkPrimi: 100,
        isverenIssizlikPrimi: 20,
        isciSgkPrimi: 70,
        isciIssizlikPrimi: null,
      }),
    ]);

    expect(rows[0].status).toBe('missingSnapshot');
    expect(getSgkPrimKontroluTotals(rows).sgkMutabakatToplami).toBe(0);
  });

  test('negatif SGK snapshot değeri geçersiz sayılır ve toplama girmez', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'negative-sgk', 'CALCULATED', {
        ...completeAmounts,
        isverenSgkPrimi: -1,
      }),
    ]);

    expect(rows[0].status).toBe('missingSnapshot');
    expect(getSgkPrimKontroluTotals(rows).sgkMutabakatToplami).toBe(0);
  });

  test('PEK alt sınır işveren tamamlama mutabakat toplamına eklenir', () => {
    const item = payroll('p-1', 'with-lower-bound', 'FINALIZED', {
      isverenSgkPrimi: 500,
      isverenIssizlikPrimi: 200,
      isciSgkPrimi: 200,
      isciIssizlikPrimi: 100,
    });
    item.pekDetay!.pekAltSinirTamamlamaIsverenPrimi = 150;

    const rows = getSgkPrimKontroluRows(period, [person1], [item]);
    const totals = getSgkPrimKontroluTotals(rows);

    expect(rows[0].dortPrimToplami).toBe(1000);
    expect(rows[0].pekAltSinirTamamlamaIsverenPrimi).toBe(150);
    expect(rows[0].toplam).toBe(1150);
    expect({
      dortPrimToplami: totals.dortPrimToplami,
      pekAltSinirTamamlamaIsverenPrimi: 150,
      sgkMutabakatToplami: totals.sgkMutabakatToplami,
    }).toEqual({
      dortPrimToplami: 1000,
      pekAltSinirTamamlamaIsverenPrimi: 150,
      sgkMutabakatToplami: 1150,
    });
  });

  test('PEK dahil program mutabakatı 1150, SGK 1140 ise fark +10 olur', () => {
    const item = payroll('p-1', 'with-lower-bound-comparison', 'FINALIZED', {
      isverenSgkPrimi: 500,
      isverenIssizlikPrimi: 200,
      isciSgkPrimi: 200,
      isciIssizlikPrimi: 100,
      pekAltSinirTamamlamaIsverenPrimi: 150,
    });
    const totals = getSgkPrimKontroluTotals(getSgkPrimKontroluRows(period, [person1], [item]));

    expect(totals.dortPrimToplami).toBe(1000);
    expect(totals.sgkMutabakatToplami).toBe(1150);
    expect(compareSgkPrimTotals(totals.sgkMutabakatToplami, '1.140,00')).toEqual({
      status: 'programHigher',
      sgkTutarKurus: 114000,
      farkKurus: 1000,
    });
  });

  test('eksik personel ve sıfır fark olsa bile karşılaştırma incomplete olur', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'stale-for-comparison', 'STALE', completeAmounts),
    ]);
    const totals = getSgkPrimKontroluTotals(rows);

    expect(totals.reconciliationReady).toBe(false);
    expect(compareSgkPrimTotals(totals.sgkMutabakatToplami, '0,00', totals.reconciliationReady)).toEqual({
      status: 'incomplete',
      sgkTutarKurus: 0,
      farkKurus: 0,
    });
  });
});

describe('SGK prim kontrolü oran başlıkları ve Excel payload', () => {
  const institutionSettings: Partial<DönemselKurumDegerleri> = {
    donemId: period.id,
    sgkIsverenOraniYuzde: 19.5,
    issizlikIsverenOraniYuzde: 1.5,
    sgkIsciOraniYuzde: 13,
    issizlikIsciOraniYuzde: 0.5,
  };

  test('aktif kurum oranları başlıklara yansır', () => {
    const rows = getSgkPrimKontroluRows(period, [person1], [
      payroll('p-1', 'period-rate', 'FINALIZED', completeAmounts),
    ]);

    expect(getSgkPrimKontroluRateLabels(rows, institutionSettings)).toEqual({
      isverenSgk: 'SGK İşveren %19,5',
      isverenIssizlik: 'İşveren İşsizlik %1,5',
      isciSgk: 'SGK İşçi %13',
      isciIssizlik: 'İşçi İşsizlik %0,5',
    });
  });

  test('işveren snapshot oranı aktif kurum oranına tercih edilir', () => {
    const item = payroll('p-1', 'snapshot-rate', 'FINALIZED', completeAmounts);
    item.pekDetay!.sgkIsverenOraniYuzde = 22;
    item.pekDetay!.isverenIssizlikOraniYuzde = 3;
    const rows = getSgkPrimKontroluRows(period, [person1], [item]);

    expect({
      isverenSgk: getSgkPrimKontroluRateLabels(rows, institutionSettings).isverenSgk,
      isverenIssizlik: getSgkPrimKontroluRateLabels(rows, institutionSettings).isverenIssizlik,
    }).toEqual({
      isverenSgk: 'SGK İşveren %22',
      isverenIssizlik: 'İşveren İşsizlik %3',
    });
  });

  test('farklı authoritative snapshot oranları başlıkta Değişken Oran olarak gösterilir', () => {
    const first = payroll('p-1', 'snapshot-rate-1', 'FINALIZED', completeAmounts);
    const second = payroll('p-1', 'snapshot-rate-2', 'CALCULATED', completeAmounts);
    first.pekDetay!.sgkIsverenOraniYuzde = 21;
    second.pekDetay!.sgkIsverenOraniYuzde = 22;
    const rows = getSgkPrimKontroluRows(period, [person1], [first, second]);

    expect(getSgkPrimKontroluRateLabels(rows, institutionSettings).isverenSgk).toBe(
      'SGK İşveren (Değişken Oran)'
    );
  });

  test('Excel payload PEK kolonunu, mutabakat özetini ve hazır olmayan kişi sayısını taşır', () => {
    const ready = payroll('p-1', 'excel-ready', 'FINALIZED', {
      ...completeAmounts,
      pekAltSinirTamamlamaIsverenPrimi: 150,
    });
    const notReady = payroll('p-2', 'excel-stale', 'STALE', completeAmounts);
    const rows = getSgkPrimKontroluRows(period, [person1, person2], [ready, notReady]);
    const totals = getSgkPrimKontroluTotals(rows);
    const rateLabels = getSgkPrimKontroluRateLabels(rows, institutionSettings);
    const comparison = compareSgkPrimTotals(totals.sgkMutabakatToplami, '350,00', false);
    const payload = buildSgkPrimKontroluExcelPayload(rows, totals, rateLabels, comparison);

    expect(payload.columns.map((column) => column.header).includes('PEK Alt Sınır İşveren Tamamlama')).toBe(true);
    expect({
      durum: payload.data[1].durum,
      isverenSgkPrimi: payload.data[1].isverenSgkPrimi,
      pekAltSinirTamamlamaIsverenPrimi: payload.data[1].pekAltSinirTamamlamaIsverenPrimi,
      toplam: payload.data[1].toplam,
    }).toEqual({
      durum: 'Yeniden hesaplanmalı',
      isverenSgkPrimi: '',
      pekAltSinirTamamlamaIsverenPrimi: '',
      toplam: '',
    });
    expect(payload.summaryRows.map((row) => row.adSoyad)).toEqual([
      'SGK İşveren Toplamı',
      'İşveren İşsizlik Toplamı',
      'SGK İşçi Toplamı',
      'İşçi İşsizlik Toplamı',
      'Dört Ana Prim Toplamı',
      'PEK Alt Sınır İşveren Tamamlama Toplamı',
      'SGK Mutabakat Toplamı',
      "SGK'dan Girilen Tutar",
      'Fark',
      'Hazır Olmayan Personel Sayısı',
    ]);
    expect(payload.summaryRows[6].toplam).toBe(350);
    expect(payload.summaryRows[9].toplam).toBe(1);
  });

  test('authoritative + stale farklı oran: stale satırın snapshot oranı header oranına sızmaz', () => {
    const personAItem = payroll('p-1', 'accrual-a', 'FINALIZED', {
      ...completeAmounts,
      sgkIsverenOraniYuzde: 21.75,
    });
    const personBItem1 = payroll('p-2', 'accrual-b1', 'FINALIZED', {
      ...completeAmounts,
      sgkIsverenOraniYuzde: 22,
    });
    const personBItem2 = payroll('p-2', 'accrual-b2', 'STALE', completeAmounts);

    const rows = getSgkPrimKontroluRows(
      period,
      [person1, person2],
      [personAItem, personBItem1, personBItem2]
    );

    expect(rows.find((r) => r.personel.id === 'p-1')?.status).toBe('authoritative');
    expect(rows.find((r) => r.personel.id === 'p-2')?.status).toBe('stale');
    expect(getSgkPrimKontroluRateLabels(rows, institutionSettings).isverenSgk).toBe(
      'SGK İşveren %21,75'
    );
  });

  test('iki authoritative satırda farklı oran varsa Değişken Oran üretir', () => {
    const personAItem = payroll('p-1', 'accrual-a', 'FINALIZED', {
      ...completeAmounts,
      sgkIsverenOraniYuzde: 21.75,
    });
    const personBItem = payroll('p-2', 'accrual-b', 'FINALIZED', {
      ...completeAmounts,
      sgkIsverenOraniYuzde: 22,
    });

    const rows = getSgkPrimKontroluRows(
      period,
      [person1, person2],
      [personAItem, personBItem]
    );

    expect(getSgkPrimKontroluRateLabels(rows, institutionSettings).isverenSgk).toBe(
      'SGK İşveren (Değişken Oran)'
    );
  });

  test('hiç authoritative satır yoksa aktif dönem kurum değerindeki oran kullanılır', () => {
    const stalePayroll = payroll('p-1', 'accrual-stale', 'STALE', {
      ...completeAmounts,
      sgkIsverenOraniYuzde: 25,
    });
    const draftPayroll = payroll('p-2', 'accrual-draft', 'DRAFT', {
      ...completeAmounts,
      sgkIsverenOraniYuzde: 25,
    });

    const rows = getSgkPrimKontroluRows(
      period,
      [person1, person2, person3],
      [stalePayroll, draftPayroll]
    );

    expect(rows.map((r) => r.status)).toEqual(['stale', 'draft', 'notCalculated']);
    expect(
      getSgkPrimKontroluRateLabels(rows, { sgkIsverenOraniYuzde: 22 }).isverenSgk
    ).toBe('SGK İşveren %22');
  });

  test('hiç authoritative satır ve kurum değeri yoksa DEFAULT_KURUM_DEGERLERI fallback çalışır', () => {
    const stalePayroll = payroll('p-1', 'accrual-stale', 'STALE', completeAmounts);
    const draftPayroll = payroll('p-2', 'accrual-draft', 'DRAFT', completeAmounts);

    const rows = getSgkPrimKontroluRows(
      period,
      [person1, person2, person3],
      [stalePayroll, draftPayroll]
    );

    const labelsWithoutSettings = getSgkPrimKontroluRateLabels(rows, undefined);
    expect(labelsWithoutSettings).toEqual({
      isverenSgk: 'SGK İşveren %21,75',
      isverenIssizlik: 'İşveren İşsizlik %2',
      isciSgk: 'SGK İşçi %14',
      isciIssizlik: 'İşçi İşsizlik %1',
    });

    const labelsWithEmptySettings = getSgkPrimKontroluRateLabels(rows, {});
    expect(labelsWithEmptySettings).toEqual({
      isverenSgk: 'SGK İşveren %21,75',
      isverenIssizlik: 'İşveren İşsizlik %2',
      isciSgk: 'SGK İşçi %14',
      isciIssizlik: 'İşçi İşsizlik %1',
    });
  });
});
