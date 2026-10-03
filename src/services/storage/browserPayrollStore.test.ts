import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  browserPayrollStore,
  BrowserSnapshotConflictError,
  canonicalizeLegacyBackupPayload,
  isMigratableBackupPayload,
  meetsSnapshotRevisionFloor,
  SerializedWriteQueue,
  shouldAdoptRemoteSnapshot,
} from './browserPayrollStore';
import {
  isSupportedLegacyBackupPayload,
  parseCurrentBrowserSnapshot,
  parseImportedBackup,
  parseLegacyBackup,
  repairAndCanonicalizeBackup,
  repairLegacyPersonTaxOpenings,
  verifyCurrentPayrollBackupReplay,
} from './payrollPayload';
import { serializePayrollStorage, type PayrollStorageDto } from '../payrollEngine/decimalBoundary';
import type { PayrollEngine } from '../payrollEngine/types';

type TestRecord = Record<string, unknown>;

// Keep the representative fixture coupled to the current DTO. A newly
// required TypeScript model field makes this fixture fail at typecheck until
// the runtime validator/test contract is updated deliberately.
function makeRealisticSnapshot(): PayrollStorageDto {
  const period = {
    id: '2026-01',
    yil: 2026,
    ay: 1,
    baslangicTarihi: '2026-01-15',
    bitisTarihi: '2026-02-14',
    donemAdi: 'Ocak 2026',
    taxYear: 2026,
    taxMonth: 2,
  };
  const personnel = {
    id: 'person-1',
    tcNo: '10000000000',
    ad: 'Ada',
    soyad: 'Yılmaz',
    grup: '1. Grup',
    unvan: 'İşçi',
    sgkSicilNo: 'SGK-1',
    iban: 'TR000000000000000000000001',
    hizmetYili: 4,
    aciklama: 'Temsilî production kaydı',
    devirKumulatifGvMatrahi: '0.00',
    devirKumulatifGvMatrahiYili: 2026,
    devirKumulatifGvMatrahiBaslangicAyi: 1,
    devirKumulatifAsgariGvMatrahi: '0.00',
    devirKumulatifAsgariGvMatrahiYili: 2026,
    kesintiler: {
      sendikaUyesi: false,
      sabitSendikaAidati: '0.00',
      besUyesi: false,
      oksOraniYuzde: '0.00',
      sabitBesTutar: '0.00',
      icraTutar: '0.00',
      kisiBorcuTutar: '0.00',
      dogumAskerlikBorclanmasiTutar: '0.00',
      hayatSaglikSigortasiTutar: '0.00',
      digerKesintiTutar: '0.00',
      gvIndirimleri: {
        dogumAskerlikGvIndirimTutar: '0.00',
        hayatSigortasiPrimiTutar: '0.00',
        saglikSigortasiPrimiTutar: '0.00',
      },
    },
  };
  const settings = {
    donemId: period.id,
    gunlukTabanUcret: '100.00',
    gunlukYemek: '20.00',
    birlestirilmisSosyalYardim: '10.00',
    gunlukVasitaYol: '5.00',
    giyimYardimi: '0.00',
    hizmetZammiBirimi: '0.00',
    isPrimiYuzde: '9.00',
    isPrimiGruplari: [{ id: 'group-1', ad: '1. Grup', oran: '9.00', aktif: true }],
    geceCalismaPrimiYuzde: '0.00',
    geceCalismaTatiliPrimiYuzde: '0.00',
    ekOdeme: '0.00',
    digerGelirVarsayilan: '0.00',

    sgkIsciOraniYuzde: '14.00',
    issizlikIsciOraniYuzde: '1.00',
    gelirVergisiOraniYuzde: '15.00',
    damgaVergisiOraniBinde: '7.59',
    sendikaAidatiYuzde: '0.00',
    sabitSendikaAidati: '0.00',
    besOraniYuzde: '3.00',
    sabitBesTutar: '0.00',
    gunlukYemekIstisnasiSGK: '0.00',
    gunlukYemekIstisnasiGV: '0.00',
    pekTavanKatsayisi: '7.5',
    gunlukAsgariUcret: '100.00',
    sgkIsverenOraniYuzde: '21.75',
    issizlikIsverenOraniYuzde: '2.00',
  };
  const payroll: PayrollStorageDto['bordrolar'][number] = {
    id: 'person-1_2026-01',
    personelId: personnel.id,
    donemId: period.id,
    accrualId: 'person-1_2026-01',
    accrualType: 'NORMAL',
    paymentDate: '2026-02-14',
    sequence: 0,
    accrualDescription: null,
    puantajOzeti: { Ç: 1, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 },
    gelirler: {
      tabanBrutAylik: '3000.00',
      tediye: null,
      tisIkramiyesi: null,
      ekOdeme: '0.00',
      yemek: '0.00',
      birlestirilmisSosyalYardim: '0.00',
      vasitaYol: '0.00',
      giyimYardimi: '0.00',
      isPrimi: '0.00',
      geceCalismasiUcreti: null,
      geceCalismasiTatiliUcreti: null,
      hizmetZammi: '0.00',
      digerGelir: '0.00',
    },
    gelirToplam: '3000.00',
    kesintiler: {
      isciSgkPrimi: '0.00',
      isciIssizlikPrimi: '0.00',
      gelirVergisi: '0.00',
      damgaVergisi: '0.00',
      sendikaAidati: '0.00',
      bes: '0.00',
      icra: '0.00',
      kisiBorcu: '0.00',
      dogumAskerlikBorclanmasi: '0.00',
      hayatSaglikSigortasi: '0.00',
      digerKesinti: '0.00',
    },
    kesintiToplam: '0.00',
    netOdeme: '64179.78',
    status: 'CALCULATED',
    olusturulmaTarihi: '2026-02-14T10:00:00.000Z',
    sonGuncellemeTarihi: '2026-02-14T10:00:00.000Z',
    oncekiKumulatifGvMatrahi: '0.00',
    oncekiKumulatifAsgariGvMatrahi: '0.00',
    manuelKumulatifGvMatrahi: null,
    devredenPekGelen: [],
    sonrakiDevredenPek: [],
    pekDetay: {
      hesaplananPek: '3000.00',
      hamPek: '3000.00',
      devredenPekKullanilan: '0.00',
      primMatrahi: '3000.00',
      finalPek: '3000.00',
      devredenPekAşanTutar: '0.00',
      pekAltSinir: '0.00',
      pekUstSinir: '30000.00',
      altSinirTamamlamaFarki: '0.00',
      fiiliYemekGunu: 0,
      yemekIstisnasiTutar: '0.00',
      isverenSgkPrimi: '0.00',
      isverenIssizlikPrimi: '0.00',
      pekAltSinirTamamlamaIsverenPrimi: '0.00',
      isverenPrimToplami: '0.00',
      sgkIsverenOraniYuzde: '21.75',
      isverenIssizlikOraniYuzde: '2.00',
    },
    isPrimiDetay: {
      grupId: 'group-1',
      grupAd: '1. Grup',
      oran: '9.00',
      hakGunu: 1,
      gunlukIsPrimi: '0.00',
      tutar: '0.00',
    },
    gvDetay: {
      oncekiKumulatifGvMatrahi: '0.00',
      cariGvMatrahi: '3000.00',
      yeniKumulatifGvMatrahi: '3000.00',
      brutGelirVergisi: '450.00',
      asgariUcretGvMatrahi: '0.00',
      asgariUcretReferansKumulatifMatrahi: '0.00',
      asgariUcretGvIstisnasi: '0.00',
      ayniAyOncekiKullanilanGvIstisnasi: '0.00',
      tahakkukOncesiKalanGvIstisnasi: '0.00',
      uygulananGvIstisnasi: '0.00',
      tahakkukSonrasiKalanGvIstisnasi: '0.00',
      kesilenGelirVergisi: '0.00',
      dogumAskerlikGvIndirimi: '0.00',
      sigortaGvIndirimAdayi: '0.00',
      sigortaGvAylikLimiti: '0.00',
      sigortaGvYillikKalanLimiti: '0.00',
      uygulanabilirSigortaGvIndirimi: '0.00',
    },
    persistedGvBase: '3000.00',
    damgaDetay: {
      brutDamgaVergisi: '0.00',
      aylikDamgaIstisnaHakki: '0.00',
      ayniAyOncekiKullanilanDamgaIstisnasi: '0.00',
      uygulananDamgaIstisnasi: '0.00',
      kalanDamgaIstisnasi: '0.00',
      kesilenDamgaVergisi: '0.00',
    },
    statutorySnapshot: {
      segments: [
        {
          effectiveFrom: '2026-01-15',
          effectiveTo: '2026-02-14',
          sgkPrimGunSayisi: 1,
          fiiliYemekGunu: 0,
          gunlukAsgariUcret: '100.00',
          pekTavanKatsayisi: '7.5',
          gunlukYemekIstisnasiSGK: '0.00',
          gunlukYemekIstisnasiGV: '0.00',
        },
      ],
      sgkPrimGunSayisi: 1,
      pekAltSinir: '0.00',
      pekUstSinir: '30000.00',
      sgkYemekIstisnasiToplam: '0.00',
      gvYemekIstisnasiToplam: '0.00',
      gvReferansGunlukAsgariUcret: '100.00',
    },
    odenenRaporluGun: 0,
    raporluGun: 0,
  };

  return {
    backupVersion: 5,
    exportedAt: '2026-02-14T10:00:00.000Z',
    donemler: [period],
    aktifDonemId: period.id,
    personeller: [personnel],
    kurumDegerleriMap: { [period.id]: settings },
    puantajlar: [
      {
        id: `${personnel.id}_${period.id}`,
        personelId: personnel.id,
        donemId: period.id,
        gunler: { '2026-01-15': 'Ç' },
      },
    ],
    bordrolar: [payroll],
    taxOpenings: [
      {
        id: 'opening-1',
        personnelId: personnel.id,
        year: 2026,
        gvCumulativeOpening: '0.00',
        effectiveFromPeriodId: period.id,
      },
    ],
    sickLeaveRecords: [
      {
        id: 'sick-1',
        personnelId: personnel.id,
        startDate: '2026-01-20',
        endDate: '2026-01-20',
      },
    ],
    annualPayrollParameters: [
      {
        year: 2026,
        gelirVergisiDilimleri: [{ limit: '190000', oran: '0.15' }],
        sigortaGvYillikBrutAsgariUcretTavani: '396360.00',
      },
    ],
    zamAylari: [1, 7],
    compensationRevisions: [],
    compensationRevisionOverrides: [],
    retroBatches: [],
    retroAllocations: [],
  };
}

function makeV2Snapshot(netOdeme: unknown = '64179.78'): string {
  const snapshot = makeRealisticSnapshot() as unknown as TestRecord;
  const payroll = firstRecord(snapshot, 'bordrolar');
  const income = payroll.gelirler as TestRecord;
  income.tabanBrutAylik = netOdeme;
  payroll.gelirToplam = netOdeme;
  payroll.kesintiToplam = '0.00';
  firstRecord(snapshot, 'bordrolar').netOdeme = netOdeme;
  return JSON.stringify(snapshot);
}

function makeLegacyV2Snapshot(netOdeme: unknown = '64179.78'): string {
  const snapshot = parseTestSnapshot(makeV2Snapshot(netOdeme));
  snapshot.backupVersion = 2;
  const payroll = firstRecord(snapshot, 'bordrolar');
  delete payroll.accrualId;
  delete payroll.accrualType;
  delete payroll.paymentDate;
  delete payroll.sequence;
  delete payroll.accrualDescription;
  delete payroll.damgaDetay;
  const gv = payroll.gvDetay as TestRecord;
  delete gv.oncekiKumulatifGvMatrahi;
  delete gv.ayniAyOncekiKullanilanGvIstisnasi;
  delete gv.tahakkukOncesiKalanGvIstisnasi;
  delete gv.tahakkukSonrasiKalanGvIstisnasi;
  return JSON.stringify(snapshot);
}

function parseTestSnapshot(json: string): TestRecord {
  return JSON.parse(json) as TestRecord;
}

function firstRecord(snapshot: TestRecord, collection: string): TestRecord {
  return (snapshot[collection] as TestRecord[])[0];
}

function retroIncome(base: unknown): TestRecord {
  return {
    ...(base as TestRecord),
    tabanBrutAylik: '10.00',
    tediye: '0.00',
    tisIkramiyesi: '0.00',
    ekOdeme: '0.00',
    yemek: '0.00',
    birlestirilmisSosyalYardim: '0.00',
    vasitaYol: '0.00',
    giyimYardimi: '0.00',
    isPrimi: '0.00',
    geceCalismasiUcreti: '0.00',
    geceCalismasiTatiliUcreti: '0.00',
    hizmetZammi: '0.00',
    digerGelir: '0.00',
  };
}

function retroDeductions(base: unknown): TestRecord {
  return {
    ...(base as TestRecord),
    isciSgkPrimi: '0.00',
    isciIssizlikPrimi: '0.00',
    gelirVergisi: '0.00',
    damgaVergisi: '0.00',
    sendikaAidati: '0.00',
    bes: '0.00',
    icra: '0.00',
    kisiBorcu: '0.00',
    dogumAskerlikBorclanmasi: '0.00',
    hayatSaglikSigortasi: '0.00',
    digerKesinti: '0.00',
  };
}

function retroBatchSettlement(total: string): TestRecord {
  const negative = total.startsWith('-');
  return {
    payableSettlementAmount: negative ? '0.00' : total,
    offsetSettlementAmount: '0.00',
    recoveredAmount: '0.00',
    recoverableAmount: negative ? total.slice(1) : '0.00',
    outstandingReceivable: negative ? total.slice(1) : '0.00',
  };
}

function retroAllocationSettlement(delta: string): TestRecord {
  const negative = delta.startsWith('-');
  return {
    previousAuthoritativeRetroAmount: '0.00',
    originalPek: '0.00',
    retroPekDelta: '0.00',
    adjustedPek: '0.00',
    workerSgkDelta: '0.00',
    workerUnemploymentDelta: '0.00',
    employerSgkDelta: '0.00',
    employerUnemploymentDelta: '0.00',
    originalEmployerLowerBound: '0.00',
    targetEmployerLowerBound: '0.00',
    employerLowerBoundDelta: '0.00',
    employerLowerBoundPremiumDelta: '0.00',
    payableSettlementAmount: negative ? '0.00' : delta,
    offsetSettlementAmount: '0.00',
    recoverableAmount: negative ? delta.slice(1) : '0.00',
  };
}

function makeLegacyV1Snapshot(netOdeme: unknown = '64179.78'): TestRecord {
  const legacy = parseTestSnapshot(makeV2Snapshot(netOdeme));
  legacy.backupVersion = 1;
  return legacy;
}

describe('BrowserPayrollStore', () => {
  test('accepts and preserves the exported CALCULATED batch with a stale linked payment event', () => {
    const referenceJson = readFileSync(
      new URL('../../../tests/fixtures/session3-reference.json', import.meta.url),
      'utf8'
    );
    const parsed = parseImportedBackup(referenceJson);
    const batchId = 'retro-d8374d9a-fa2d-4c24-a9c5-edc0a1800c93';
    const batch = parsed.retroBatches.find((item) => item.id === batchId);
    const payment = parsed.bordrolar.find((item) => item.accrualId === batchId);

    expect(batch?.status).toBe('CALCULATED');
    expect(payment?.status).toBe('STALE');
    expect(payment?.gelirToplam).toBe('3289');
    expect(parseCurrentBrowserSnapshot(serializePayrollStorage(parsed))).toEqual(parsed);

    const forged = JSON.parse(referenceJson) as TestRecord;
    const linked = (forged.bordrolar as TestRecord[]).find((item) => item.accrualId === batchId)!;
    const linkedIncome = linked.gelirler as TestRecord;
    linkedIncome.tabanBrutAylik = '3101';
    linked.gelirToplam = '3290';
    linked.netOdeme = '2114.56';
    expect(() => parseImportedBackup(JSON.stringify(forged))).toThrow();

    for (const [field, value] of [
      ['personelId', 'p-forged'],
      ['paymentDate', '2027-03-15'],
      ['status', 'FINALIZED'],
    ] as const) {
      const mismatched = JSON.parse(referenceJson) as TestRecord;
      const event = (mismatched.bordrolar as TestRecord[]).find((item) => item.accrualId === batchId)!;
      event[field] = value;
      expect(() => parseImportedBackup(JSON.stringify(mismatched))).toThrow();
    }
  });

  test('current backup replay verifier accepts the canonical result and rejects a forged snapshot', async () => {
    const payload = parseCurrentBrowserSnapshot(makeV2Snapshot('3000.00'));
    const canonicalResult = payload.bordrolar[0];
    const engine = {
      kind: 'wasm',
      calculatePayroll: async () => canonicalResult,
    } as unknown as PayrollEngine;

    await verifyCurrentPayrollBackupReplay(payload, engine);

    const forged = structuredClone(payload);
    forged.bordrolar[0].persistedGvBase = '3001.00';
    forged.bordrolar[0].gvDetay = {
      ...forged.bordrolar[0].gvDetay!,
      cariGvMatrahi: '3001.00',
      yeniKumulatifGvMatrahi: '3001.00',
    };
    let error: unknown;
    try {
      await verifyCurrentPayrollBackupReplay(forged, engine);
    } catch (caught) {
      error = caught;
    }
    expect(String(error).includes('V5 backup replay')).toBe(true);
  });

  test('current backup replay accepts missing monthly PEK audit values but checks supplied values', async () => {
    const payload = parseCurrentBrowserSnapshot(makeV2Snapshot('3000.00'));
    const canonicalResult = structuredClone(payload.bordrolar[0]);
    canonicalResult.pekDetay = {
      ...canonicalResult.pekDetay!,
      aylikOncekiPekTuketimi: '1250.00',
      aylikSonrasiPekTuketimi: '4250.00',
    };
    const engine = {
      kind: 'wasm',
      calculatePayroll: async () => canonicalResult,
    } as unknown as PayrollEngine;

    for (const value of [null, undefined]) {
      const olderSnapshot = structuredClone(payload);
      const pekDetay = olderSnapshot.bordrolar[0].pekDetay!;
      pekDetay.aylikOncekiPekTuketimi = value;
      pekDetay.aylikSonrasiPekTuketimi = value;
      await verifyCurrentPayrollBackupReplay(olderSnapshot, engine);
      expect(pekDetay.aylikOncekiPekTuketimi).toBe(value);
      expect(pekDetay.aylikSonrasiPekTuketimi).toBe(value);
    }

    const matchingSnapshot = structuredClone(payload);
    matchingSnapshot.bordrolar[0].pekDetay!.aylikOncekiPekTuketimi = '1250.000';
    matchingSnapshot.bordrolar[0].pekDetay!.aylikSonrasiPekTuketimi = '4250';
    await verifyCurrentPayrollBackupReplay(matchingSnapshot, engine);

    const mismatchingSnapshot = structuredClone(payload);
    mismatchingSnapshot.bordrolar[0].pekDetay!.aylikSonrasiPekTuketimi = '4251.00';
    await expect(verifyCurrentPayrollBackupReplay(mismatchingSnapshot, engine)).rejects.toThrow(
      'V5 backup replay'
    );
  });

  test('does not fall back to localStorage when IndexedDB is unavailable', async () => {
    const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
    const originalLocalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    let localStorageReads = 0;
    let localStorageWrites = 0;

    Object.defineProperty(globalThis, 'window', {
      configurable: true,
      value: { indexedDB: undefined },
    });
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: {
        getItem: () => {
          localStorageReads += 1;
          return '{"backupVersion":2}';
        },
        setItem: () => {
          localStorageWrites += 1;
        },
      },
    });

    try {
      const loadError = await browserPayrollStore.loadPayload().catch((error) => error);
      const saveError = await browserPayrollStore.savePayload('{"backupVersion":2}').catch(
        (error) => error
      );
      expect(String(loadError).includes('IndexedDB')).toBe(true);
      expect(String(saveError).includes('IndexedDB')).toBe(true);
      expect(localStorageReads).toBe(0);
      expect(localStorageWrites).toBe(0);
    } finally {
      if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow);
      else delete (globalThis as { window?: unknown }).window;
      if (originalLocalStorage) {
        Object.defineProperty(globalThis, 'localStorage', originalLocalStorage);
      } else {
        delete (globalThis as { localStorage?: unknown }).localStorage;
      }
    }
  });

  test('rejects malformed or unsupported legacy payloads before migration', () => {
    expect(isMigratableBackupPayload('{not-json')).toBe(false);
    expect(isMigratableBackupPayload(JSON.stringify({ backupVersion: 4 }))).toBe(false);
    expect(isMigratableBackupPayload(JSON.stringify({ backupVersion: '2' }))).toBe(false);
    expect(isMigratableBackupPayload(JSON.stringify({ backupVersion: 1.5 }))).toBe(false);
    expect(
      isMigratableBackupPayload(JSON.stringify({ backupVersion: 2, donemler: [], personeller: [] }))
    ).toBe(false);
    expect(isMigratableBackupPayload(makeV2Snapshot(64179.78))).toBe(false);
  });

  test('canonicalizes valid legacy JSON and rejects malformed Decimal before any write', () => {
    const legacy = parseTestSnapshot(makeV2Snapshot(0.15));
    legacy.backupVersion = 1;
    delete legacy.exportedAt;
    delete legacy.aktifDonemId;
    delete legacy.kurumDegerleriMap;
    delete legacy.puantajlar;
    delete legacy.taxOpenings;
    delete legacy.sickLeaveRecords;
    delete legacy.annualPayrollParameters;
    delete legacy.zamAylari;
    const legacyPersonel = firstRecord(legacy, 'personeller');
    delete legacyPersonel.sgkSicilNo;
    delete legacyPersonel.iban;
    delete legacyPersonel.hizmetYili;
    delete firstRecord(legacy, 'bordrolar').status;

    const canonical = canonicalizeLegacyBackupPayload(JSON.stringify(legacy));
    const canonicalPayload = parseTestSnapshot(canonical);
    expect(firstRecord(canonicalPayload, 'bordrolar').netOdeme).toBe('0.15');
    expect(canonicalPayload.backupVersion).toBe(5);
    expect(firstRecord(canonicalPayload, 'personeller').sgkSicilNo).toBe('');
    expect(firstRecord(canonicalPayload, 'personeller').iban).toBe('');
    expect(firstRecord(canonicalPayload, 'personeller').hizmetYili).toBe(1);
    expect(firstRecord(canonicalPayload, 'bordrolar').status).toBe('CALCULATED');

    const malformed = parseTestSnapshot(makeV2Snapshot('not-a-decimal'));
    malformed.backupVersion = 1;
    expect(() => canonicalizeLegacyBackupPayload(JSON.stringify(malformed))).toThrow(
      'Geçersiz Decimal metni'
    );
  });

  test('accepts an exact, realistic current V5 snapshot', () => {
    const parsed = parseCurrentBrowserSnapshot(makeV2Snapshot());

    expect(parsed.backupVersion).toBe(5);
    expect(parsed.donemler.length).toBe(1);
    expect(parsed.personeller.length).toBe(1);
    expect(parsed.puantajlar.length).toBe(1);
    expect(parsed.bordrolar.length).toBe(1);
    expect(parsed.taxOpenings.length).toBe(1);
    expect(parsed.annualPayrollParameters.length).toBe(1);
    expect(parsed.bordrolar[0].netOdeme).toBe('64179.78');
  });

  test('persists a separate nafaka amount through the current browser backup contract', () => {
    const snapshot = parseTestSnapshot(makeV2Snapshot('3000.00'));
    const deductions = firstRecord(snapshot, 'personeller').kesintiler as TestRecord;
    deductions.nafakaTutar = '125.50';
    const payroll = firstRecord(snapshot, 'bordrolar');
    const payrollDeductions = payroll.kesintiler as TestRecord;
    payrollDeductions.nafaka = '125.50';
    payroll.kesintiToplam = '125.50';
    payroll.netOdeme = '2874.50';

    const parsed = parseCurrentBrowserSnapshot(JSON.stringify(snapshot));
    const restored = parseCurrentBrowserSnapshot(serializePayrollStorage(parsed));
    expect(restored.personeller[0].kesintiler?.nafakaTutar).toBe('125.50');
    expect(restored.bordrolar[0].kesintiler.nafaka).toBe('125.50');
    expect(restored.bordrolar[0].kesintiToplam).toBe('125.50');
    expect(restored.bordrolar[0].netOdeme).toBe('2874.50');
  });

  test('rejects semantically invalid annual parameters at the current storage boundary', () => {
    const invalid = parseTestSnapshot(makeV2Snapshot());
    const annual = firstRecord(invalid, 'annualPayrollParameters');
    annual.gelirVergisiDilimleri = [];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalid))).toThrow(
      '$.annualPayrollParameters[0].gelirVergisiDilimleri en az bir vergi dilimi içermelidir.'
    );
  });

  test('rejects a tampered normal financial snapshot', () => {
    const tampered = parseTestSnapshot(makeV2Snapshot());
    firstRecord(tampered, 'bordrolar').netOdeme = '64179.77';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(tampered))).toThrow(
      'finansal toplamları gelir/kesinti kalemleriyle eşleşmiyor'
    );
  });

  test('upgrades V3 retro payloads without dropping the retro graph and rejects incomplete V5', () => {
    const legacy = parseTestSnapshot(makeV2Snapshot());
    legacy.backupVersion = 3;
    const normal = firstRecord(legacy, 'bordrolar');
    (legacy.bordrolar as TestRecord[]).push({
      ...normal,
      id: 'payroll-v3-retro',
      accrualId: 'batch-v3',
      accrualType: 'RETRO_ADJUSTMENT',
      paymentDate: '2026-02-14',
      sequence: 1,
      gelirler: retroIncome(normal.gelirler),
      gelirToplam: '10.00',
      kesintiler: retroDeductions(normal.kesintiler),
      kesintiToplam: '0.00',
      netOdeme: '10.00',
    });
    legacy.compensationRevisions = [{
      id: 'revision-v3',
      reason: 'COLLECTIVE_AGREEMENT',
      title: 'V3 retro',
      effectiveFrom: '2026-01-15',
      status: 'CALCULATED',
      scope: 'SELECTED_PERSONNEL',
      personnelIds: ['person-1'],
    }];
    legacy.compensationRevisionOverrides = [];
    legacy.retroBatches = [{
      id: 'batch-v3',
      revisionId: 'revision-v3',
      personnelId: 'person-1',
      // Keep the legacy fixture internally consistent with its cloned
      // payment event; V5 cross-record validation must not accept a dangling
      // person/payment-date relationship during upgrade.
      paymentDate: '2026-02-14',
      status: 'CALCULATED',
      totalGrossDelta: '10.00',
      settlementStatus: 'UNSETTLED',
    }];
    legacy.retroAllocations = [{
      id: 'allocation-v3',
      batchId: 'batch-v3',
      personnelId: 'person-1',
      sourcePeriodId: '2026-01',
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: '0.00',
      targetAmount: '10.00',
      deltaAmount: '10.00',
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
    }];

    const upgraded = parseImportedBackup(JSON.stringify(legacy));
    expect(upgraded.backupVersion).toBe(5);
    expect(upgraded.compensationRevisions[0].id).toBe('revision-v3');
    expect(upgraded.retroBatches[0].id).toBe('batch-v3');
    expect(upgraded.retroAllocations[0].deltaAmount).toBe('10.00');
    expect(upgraded.retroBatches[0].payableSettlementAmount).toBe('10.00');
    expect(upgraded.retroAllocations[0].payableSettlementAmount).toBe('10.00');
    expect(upgraded.bordrolar.length).toBe(2);
    expect(upgraded.bordrolar[1].accrualType).toBe('RETRO_ADJUSTMENT');
    expect(upgraded.bordrolar[1].sequence).toBe(1);

    const incompleteCurrent = parseTestSnapshot(makeV2Snapshot());
    incompleteCurrent.backupVersion = 5;
    delete incompleteCurrent.retroAllocations;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(incompleteCurrent))).toThrow(
      '$.retroAllocations zorunlu alan eksik'
    );
  });

  test('validates retro batch totals in kuruş and preserves signed overpayment ledger fields', () => {
    const mismatched = parseTestSnapshot(makeV2Snapshot());
    mismatched.compensationRevisions = [{
      id: 'revision-ledger',
      reason: 'COLLECTIVE_AGREEMENT',
      title: 'Ledger',
      effectiveFrom: '2026-01-15',
      status: 'CALCULATED',
      scope: 'SELECTED_PERSONNEL',
      personnelIds: ['person-1'],
    }];
    mismatched.retroBatches = [{
      id: 'batch-ledger',
      revisionId: 'revision-ledger',
      personnelId: 'person-1',
      paymentDate: '2026-02-14',
      status: 'CALCULATED',
      settlementStatus: 'UNSETTLED',
      totalGrossDelta: '11.00',
      ...retroBatchSettlement('11.00'),
    }];
    mismatched.retroAllocations = [{
      id: 'allocation-ledger',
      batchId: 'batch-ledger',
      personnelId: 'person-1',
      sourcePeriodId: '2026-01',
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: '10.00',
      targetAmount: '20.00',
      deltaAmount: '10.00',
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
      ...retroAllocationSettlement('10.00'),
    }];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(mismatched))).toThrow(
      '$.retroBatches[0].totalGrossDelta allocation delta toplamı'
    );

    const overpayment = parseTestSnapshot(makeV2Snapshot());
    overpayment.compensationRevisions = mismatched.compensationRevisions;
    overpayment.retroBatches = [{
      ...(mismatched.retroBatches as TestRecord[])[0],
      totalGrossDelta: '-10.00',
      settlementStatus: 'OVERPAYMENT',
      ...retroBatchSettlement('-10.00'),
    }];
    overpayment.retroAllocations = [{
      ...(mismatched.retroAllocations as TestRecord[])[0],
      originalRecognizedAmount: '10.00',
      targetAmount: '0.00',
      deltaAmount: '-10.00',
      retroPekDelta: '-10.00',
      adjustedPek: '0.00',
      workerSgkDelta: '-1.40',
      workerUnemploymentDelta: '-0.10',
      employerSgkDelta: '-2.18',
      employerUnemploymentDelta: '-0.20',
      ...retroAllocationSettlement('-10.00'),
    }];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(overpayment))).not.toThrow();

    const inconsistentSettlement = parseTestSnapshot(makeV2Snapshot());
    inconsistentSettlement.compensationRevisions = mismatched.compensationRevisions;
    inconsistentSettlement.retroBatches = [{
      ...(mismatched.retroBatches as TestRecord[])[0],
      totalGrossDelta: '10.00',
      settlementStatus: 'PAID',
      ...retroBatchSettlement('10.00'),
    }];
    inconsistentSettlement.retroAllocations = [{
      ...(mismatched.retroAllocations as TestRecord[])[0],
      originalRecognizedAmount: '0.00',
      targetAmount: '10.00',
      deltaAmount: '10.00',
      ...retroAllocationSettlement('10.00'),
    }];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(inconsistentSettlement))).toThrow(
      'settlement statusı tutarsız'
    );

    const finalizedWithoutPayment = parseTestSnapshot(makeV2Snapshot());
    finalizedWithoutPayment.compensationRevisions = mismatched.compensationRevisions;
    finalizedWithoutPayment.retroBatches = [{
      ...(mismatched.retroBatches as TestRecord[])[0],
      status: 'FINALIZED',
      settlementStatus: 'PAID',
      totalGrossDelta: '10.00',
      ...retroBatchSettlement('10.00'),
    }];
    finalizedWithoutPayment.retroAllocations = inconsistentSettlement.retroAllocations;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(finalizedWithoutPayment))).toThrow(
      'FINALIZED retro batch tam olarak bir payment event'
    );
  });

  test('replays same-date receivables by batch creation chronology and rejects inconsistent balances', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const revisionId = 'revision-same-date-replay';
    const positiveBatchId = 'retro-b40a20f7-114c-447b-b138-8e5486150a31';
    const recoveryBatchId = 'retro-5b80adf8-da00-4820-a59e-85e8b49ffaad';
    payload.compensationRevisions = [{
      id: revisionId,
      reason: 'COLLECTIVE_AGREEMENT',
      title: 'Same date replay',
      effectiveFrom: '2027-01-15',
      status: 'CALCULATED',
      scope: 'SELECTED_PERSONNEL',
      personnelIds: ['person-1'],
      createdAt: '2026-09-29T21:00:00.000Z',
    }];
    const positiveBatch = {
      id: positiveBatchId,
      revisionId,
      personnelId: 'person-1',
      paymentDate: '2027-03-14',
      status: 'CALCULATED',
      settlementStatus: 'UNSETTLED',
      totalGrossDelta: '3268',
      createdAt: '2026-09-29T21:04:01.133Z',
      calculatedAt: '2026-09-29T21:04:01.133Z',
      ...retroBatchSettlement('3268'),
    };
    const recoveryBatch = {
      id: recoveryBatchId,
      revisionId,
      personnelId: 'person-1',
      paymentDate: '2027-03-14',
      status: 'CALCULATED',
      settlementStatus: 'OVERPAYMENT',
      totalGrossDelta: '-3268',
      createdAt: '2026-09-30T11:43:10.586Z',
      calculatedAt: '2026-09-30T11:43:10.586Z',
      ...retroBatchSettlement('-3268'),
    };
    payload.retroBatches = [recoveryBatch, positiveBatch];
    payload.retroAllocations = [
      {
        id: `${recoveryBatchId}_2027-01_BASE_WAGE`,
        batchId: recoveryBatchId,
        personnelId: 'person-1',
        sourcePeriodId: '2026-01',
        earningCode: 'BASE_WAGE',
        originalRecognizedAmount: '3268',
        targetAmount: '0',
        deltaAmount: '-3268',
        ...retroAllocationSettlement('-3268'),
        sgkTreatment: 'WAGE_SOURCE_MONTH',
        incomeTaxTreatment: 'TAXABLE',
        stampTaxTreatment: 'TAXABLE',
      },
      {
        id: `${positiveBatchId}_2027-01_BASE_WAGE`,
        batchId: positiveBatchId,
        personnelId: 'person-1',
        sourcePeriodId: '2026-01',
        earningCode: 'BASE_WAGE',
        originalRecognizedAmount: '0',
        targetAmount: '3268',
        deltaAmount: '3268',
        ...retroAllocationSettlement('3268'),
        sgkTreatment: 'WAGE_SOURCE_MONTH',
        incomeTaxTreatment: 'TAXABLE',
        stampTaxTreatment: 'TAXABLE',
      },
    ];

    const imported = parseImportedBackup(JSON.stringify(payload));
    expect(imported.retroBatches.find((batch) => batch.id === positiveBatchId)?.outstandingReceivable).toBe('0.00');
    expect(imported.retroBatches.find((batch) => batch.id === recoveryBatchId)?.outstandingReceivable).toBe('3268');
    expect(imported.retroBatches.map((batch) => batch.id)).toEqual([recoveryBatchId, positiveBatchId]);

    const mixedTimestamps = JSON.parse(JSON.stringify(payload)) as TestRecord;
    const mixedBatches = mixedTimestamps.retroBatches as TestRecord[];
    const mixedPositive = mixedBatches.find((batch) => batch.id === positiveBatchId)!;
    const mixedRecovery = mixedBatches.find((batch) => batch.id === recoveryBatchId)!;
    mixedPositive.id = 'retro-z-positive';
    mixedRecovery.id = 'retro-a-recovery';
    delete mixedPositive.createdAt;
    const mixedAllocations = mixedTimestamps.retroAllocations as TestRecord[];
    mixedAllocations.forEach((allocation) => {
      if (allocation.batchId === positiveBatchId) {
        allocation.batchId = 'retro-z-positive';
        allocation.id = 'retro-z-positive_2027-01_BASE_WAGE';
      } else {
        allocation.batchId = 'retro-a-recovery';
        allocation.id = 'retro-a-recovery_2027-01_BASE_WAGE';
      }
    });
    expect(() => parseImportedBackup(JSON.stringify(mixedTimestamps))).not.toThrow();

    const inconsistent = JSON.parse(JSON.stringify(payload)) as TestRecord;
    const malformedRecovery = (inconsistent.retroBatches as TestRecord[]).find(
      (batch) => batch.id === recoveryBatchId
    )!;
    malformedRecovery.outstandingReceivable = '0';
    expect(() => parseImportedBackup(JSON.stringify(inconsistent))).toThrow(
      'outstanding receivable replay sonucu ile eşleşmiyor'
    );
  });

  test('accepts runtime receivable replay with zero recovery and eventless overpayments', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const zeroRecoveryId = 'retro-0cbb12f3-4999-43d6-8215-9b710093c5d4';
    const priorOverpaymentId = 'retro-43d8c24e-6c31-416b-b203-c334568697c1';
    const laterOverpaymentId = 'retro-f0000000-0000-4000-8000-000000000001';
    const revisionId = 'revision-receivable-replay';
    payload.compensationRevisions = [{
      id: revisionId,
      reason: 'COLLECTIVE_AGREEMENT',
      title: 'Receivable replay',
      effectiveFrom: '2026-01-15',
      status: 'CALCULATED',
      scope: 'SELECTED_PERSONNEL',
      personnelIds: ['person-1'],
    }];
    const priorOverpayment = {
      id: priorOverpaymentId,
      revisionId,
      personnelId: 'person-1',
      paymentDate: '2027-03-14',
      status: 'CALCULATED',
      settlementStatus: 'OVERPAYMENT',
      totalGrossDelta: '-160872.98',
      createdAt: undefined,
      ...retroBatchSettlement('-160872.98'),
    };
    const zeroRecovery = {
      id: zeroRecoveryId,
      revisionId,
      personnelId: 'person-1',
      paymentDate: '2027-03-14',
      status: 'CALCULATED',
      settlementStatus: 'UNSETTLED',
      totalGrossDelta: '0.00',
      createdAt: '2026-09-30T11:43:10.586Z',
      calculatedAt: '2026-09-30T11:43:10.586Z',
      ...retroBatchSettlement('0.00'),
      outstandingReceivable: '160872.98',
    };
    const laterOverpayment = {
      id: laterOverpaymentId,
      revisionId,
      personnelId: 'person-1',
      paymentDate: '2027-03-14',
      status: 'CALCULATED',
      settlementStatus: 'OVERPAYMENT',
      totalGrossDelta: '-10.00',
      createdAt: '2026-10-01T11:43:10.586Z',
      calculatedAt: '2026-10-01T11:43:10.586Z',
      ...retroBatchSettlement('-10.00'),
      outstandingReceivable: '160882.98',
    };
    payload.retroBatches = [zeroRecovery, laterOverpayment, priorOverpayment];
    payload.retroAllocations = [
      {
        id: `${priorOverpaymentId}_2026-01_BASE_WAGE`,
        batchId: priorOverpaymentId,
        personnelId: 'person-1',
        sourcePeriodId: '2026-01',
        earningCode: 'BASE_WAGE',
        originalRecognizedAmount: '160872.98',
        targetAmount: '0.00',
        deltaAmount: '-160872.98',
        ...retroAllocationSettlement('-160872.98'),
        sgkTreatment: 'WAGE_SOURCE_MONTH',
        incomeTaxTreatment: 'TAXABLE',
        stampTaxTreatment: 'TAXABLE',
      },
      {
        id: `${zeroRecoveryId}_2026-01_BASE_WAGE`,
        batchId: zeroRecoveryId,
        personnelId: 'person-1',
        sourcePeriodId: '2026-01',
        earningCode: 'BASE_WAGE',
        originalRecognizedAmount: '0.00',
        targetAmount: '0.00',
        deltaAmount: '0.00',
        ...retroAllocationSettlement('0.00'),
        sgkTreatment: 'WAGE_SOURCE_MONTH',
        incomeTaxTreatment: 'TAXABLE',
        stampTaxTreatment: 'TAXABLE',
      },
      {
        id: `${laterOverpaymentId}_2026-01_BASE_WAGE`,
        batchId: laterOverpaymentId,
        personnelId: 'person-1',
        sourcePeriodId: '2026-01',
        earningCode: 'BASE_WAGE',
        originalRecognizedAmount: '10.00',
        targetAmount: '0.00',
        deltaAmount: '-10.00',
        ...retroAllocationSettlement('-10.00'),
        sgkTreatment: 'WAGE_SOURCE_MONTH',
        incomeTaxTreatment: 'TAXABLE',
        stampTaxTreatment: 'TAXABLE',
      },
    ];

    const imported = parseImportedBackup(JSON.stringify(payload));
    const roundTripped = parseCurrentBrowserSnapshot(serializePayrollStorage(imported));
    const byId = (id: string) => roundTripped.retroBatches.find((batch) => batch.id === id)!;
    expect(byId(zeroRecoveryId).status).toBe('CALCULATED');
    expect(byId(zeroRecoveryId).settlementStatus).toBe('UNSETTLED');
    expect(byId(zeroRecoveryId).totalGrossDelta).toBe('0.00');
    expect(byId(zeroRecoveryId).payableSettlementAmount).toBe('0.00');
    expect(byId(zeroRecoveryId).outstandingReceivable).toBe('160872.98');
    expect(byId(priorOverpaymentId).settlementStatus).toBe('OVERPAYMENT');
    expect(byId(priorOverpaymentId).recoverableAmount).toBe('160872.98');
    expect(byId(priorOverpaymentId).outstandingReceivable).toBe('160872.98');
    expect(byId(laterOverpaymentId).settlementStatus).toBe('OVERPAYMENT');
    expect(byId(laterOverpaymentId).recoverableAmount).toBe('10.00');
    expect(byId(laterOverpaymentId).outstandingReceivable).toBe('160882.98');
    expect(roundTripped.retroAllocations.find((allocation) => allocation.batchId === zeroRecoveryId)?.payableSettlementAmount)
      .toBe('0.00');
    expect(roundTripped.bordrolar.filter((payroll) => payroll.accrualType === 'RETRO_ADJUSTMENT').length).toBe(0);

    const corrupted = JSON.parse(JSON.stringify(payload)) as TestRecord;
    const corruptedZero = (corrupted.retroBatches as TestRecord[]).find((batch) => batch.id === zeroRecoveryId)!;
    corruptedZero.outstandingReceivable = '0.00';
    expect(() => parseImportedBackup(JSON.stringify(corrupted))).toThrow(
      'outstanding receivable replay sonucu ile eşleşmiyor'
    );

    const sameInstant = JSON.parse(JSON.stringify(payload)) as TestRecord;
    const sameInstantBatches = sameInstant.retroBatches as TestRecord[];
    const sameInstantZero = sameInstantBatches.find((batch) => batch.id === zeroRecoveryId)!;
    const sameInstantPrior = sameInstantBatches.find((batch) => batch.id === priorOverpaymentId)!;
    sameInstantZero.outstandingReceivable = '0.00';
    sameInstantPrior.outstandingReceivable = '160872.98';
    sameInstantPrior.createdAt = '2026-09-30T14:43:10.586+03:00';
    expect(() => parseImportedBackup(JSON.stringify(sameInstant))).not.toThrow();
    sameInstantBatches.reverse();
    expect(() => parseImportedBackup(JSON.stringify(sameInstant))).not.toThrow();
  });

  test('keeps retro batch lifecycle and linked payment event status consistent', () => {
    const valid = parseTestSnapshot(makeV2Snapshot());
    const normal = firstRecord(valid, 'bordrolar');
    valid.bordrolar = [
      normal,
      {
        ...normal,
        id: 'retro-payment-lifecycle',
        accrualId: 'batch-lifecycle',
        accrualType: 'RETRO_ADJUSTMENT',
        sequence: 1,
        gelirler: retroIncome(normal.gelirler),
        gelirToplam: '10.00',
        kesintiler: retroDeductions(normal.kesintiler),
        kesintiToplam: '0.00',
        netOdeme: '10.00',
        status: 'CALCULATED',
      },
    ];
    valid.compensationRevisions = [{
      id: 'revision-lifecycle',
      reason: 'COLLECTIVE_AGREEMENT',
      title: 'Lifecycle',
      effectiveFrom: '2026-01-15',
      status: 'CALCULATED',
      scope: 'SELECTED_PERSONNEL',
      personnelIds: ['person-1'],
    }];
    valid.retroBatches = [{
      id: 'batch-lifecycle',
      revisionId: 'revision-lifecycle',
      personnelId: 'person-1',
      paymentDate: '2026-02-14',
      status: 'CALCULATED',
      settlementStatus: 'UNSETTLED',
      totalGrossDelta: '10.00',
      ...retroBatchSettlement('10.00'),
    }];
    valid.retroAllocations = [{
      id: 'allocation-lifecycle',
      batchId: 'batch-lifecycle',
      personnelId: 'person-1',
      sourcePeriodId: '2026-01',
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: '0.00',
      targetAmount: '10.00',
      deltaAmount: '10.00',
      ...retroAllocationSettlement('10.00'),
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
    }];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(valid))).not.toThrow();

    const inconsistentFinancialSnapshot = JSON.parse(JSON.stringify(valid)) as TestRecord;
    (inconsistentFinancialSnapshot.bordrolar as TestRecord[])[1].netOdeme = '9.99';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(inconsistentFinancialSnapshot))).toThrow(
      'finansal toplamları gelir/kesinti kalemleriyle eşleşmiyor'
    );

    const mismatched = JSON.parse(JSON.stringify(valid)) as TestRecord;
    (mismatched.bordrolar as TestRecord[])[1].status = 'FINALIZED';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(mismatched))).toThrow(
      'lifecycle durumu ile bağlı payment event'
    );

    const staleWithPayment = JSON.parse(JSON.stringify(valid)) as TestRecord;
    (staleWithPayment.retroBatches as TestRecord[])[0].status = 'STALE';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(staleWithPayment))).toThrow(
      'lifecycle durumu ile bağlı payment event'
    );

    const legacyFinalizedWithoutPayment = JSON.parse(JSON.stringify(valid)) as TestRecord;
    legacyFinalizedWithoutPayment.backupVersion = 3;
    legacyFinalizedWithoutPayment.bordrolar = [normal];
    (legacyFinalizedWithoutPayment.retroBatches as TestRecord[])[0].status = 'FINALIZED';
    (legacyFinalizedWithoutPayment.retroBatches as TestRecord[])[0].settlementStatus = 'PAID';
    const downgraded = parseImportedBackup(JSON.stringify(legacyFinalizedWithoutPayment));
    expect(downgraded.retroBatches[0].status).toBe('STALE');
    expect(downgraded.retroBatches[0].settlementStatus).toBe('UNSETTLED');
  });

  test('accepts offset-only retro settlement without creating a payment event', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    payload.compensationRevisions = [{
      id: 'revision-offset-only',
      reason: 'COLLECTIVE_AGREEMENT',
      title: 'Offset only',
      effectiveFrom: '2026-01-15',
      status: 'CALCULATED',
      scope: 'SELECTED_PERSONNEL',
      personnelIds: ['person-1'],
    }];
    const openBatch = {
      id: 'batch-a-open-before-offset',
      revisionId: 'revision-offset-only',
      personnelId: 'person-1',
      paymentDate: '2026-02-14',
      status: 'CALCULATED',
      settlementStatus: 'OVERPAYMENT',
      totalGrossDelta: '-10.00',
      ...retroBatchSettlement('-10.00'),
    };
    const openAllocation = {
      id: 'allocation-a-open-before-offset',
      batchId: 'batch-a-open-before-offset',
      personnelId: 'person-1',
      sourcePeriodId: '2026-01',
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: '10.00',
      targetAmount: '0.00',
      deltaAmount: '-10.00',
      ...retroAllocationSettlement('-10.00'),
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
    };
    payload.retroBatches = [openBatch, {
      id: 'batch-z-offset-only',
      revisionId: 'revision-offset-only',
      personnelId: 'person-1',
      paymentDate: '2026-02-14',
      status: 'CALCULATED',
      settlementStatus: 'SETTLED_BY_OFFSET',
      totalGrossDelta: '10.00',
      ...retroBatchSettlement('10.00'),
      payableSettlementAmount: '0.00',
      offsetSettlementAmount: '10.00',
    }];
    payload.retroAllocations = [openAllocation, {
      id: 'allocation-z-offset-only',
      batchId: 'batch-z-offset-only',
      personnelId: 'person-1',
      sourcePeriodId: '2026-01',
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: '0.00',
      targetAmount: '10.00',
      deltaAmount: '10.00',
      ...retroAllocationSettlement('10.00'),
      payableSettlementAmount: '0.00',
      offsetSettlementAmount: '10.00',
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
    }];

    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(payload))).not.toThrow();

    const withFakePayment = JSON.parse(JSON.stringify(payload)) as TestRecord;
    const normal = firstRecord(withFakePayment, 'bordrolar');
    (withFakePayment.bordrolar as TestRecord[]).push({
      ...normal,
      id: 'fake-offset-payment',
      accrualId: 'batch-z-offset-only',
      accrualType: 'RETRO_ADJUSTMENT',
      sequence: 1,
      paymentDate: '2026-02-14',
      gelirler: retroIncome(normal.gelirler),
      gelirToplam: '10.00',
      kesintiler: retroDeductions(normal.kesintiler),
      kesintiToplam: '0.00',
      netOdeme: '10.00',
      status: 'CALCULATED',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(withFakePayment))).toThrow(
      'Payable settlement olmayan retro batch payment event ile eşleşemez'
    );
  });

  test('round-trips a finalized retro graph without changing identity or settlement state', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const normal = firstRecord(payload, 'bordrolar');
    payload.bordrolar = [
      normal,
      {
        ...normal,
        id: 'retro-payment-roundtrip',
        accrualId: 'batch-roundtrip',
        accrualType: 'RETRO_ADJUSTMENT',
        sequence: 1,
        gelirler: retroIncome(normal.gelirler),
        gelirToplam: '10.00',
        kesintiler: retroDeductions(normal.kesintiler),
        kesintiToplam: '0.00',
        netOdeme: '10.00',
        status: 'FINALIZED',
      },
    ];
    payload.compensationRevisions = [{
      id: 'revision-roundtrip',
      reason: 'COLLECTIVE_AGREEMENT',
      title: 'Round trip',
      effectiveFrom: '2026-01-15',
      status: 'FINALIZED',
      scope: 'SELECTED_PERSONNEL',
      personnelIds: ['person-1'],
    }];
    payload.retroBatches = [{
      id: 'batch-roundtrip',
      revisionId: 'revision-roundtrip',
      personnelId: 'person-1',
      paymentDate: '2026-02-14',
      status: 'FINALIZED',
      settlementStatus: 'PAID',
      totalGrossDelta: '10.00',
      ...retroBatchSettlement('10.00'),
    }];
    payload.retroAllocations = [{
      id: 'allocation-roundtrip',
      batchId: 'batch-roundtrip',
      personnelId: 'person-1',
      sourcePeriodId: '2026-01',
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: '0.00',
      targetAmount: '10.00',
      deltaAmount: '10.00',
      ...retroAllocationSettlement('10.00'),
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
    }];

    const before = parseCurrentBrowserSnapshot(JSON.stringify(payload));
    const after = parseCurrentBrowserSnapshot(serializePayrollStorage(before));
    expect(after.compensationRevisions[0].id).toBe('revision-roundtrip');
    expect(after.compensationRevisions[0].status).toBe('FINALIZED');
    expect(after.retroBatches[0].id).toBe('batch-roundtrip');
    expect(after.retroBatches[0].status).toBe('FINALIZED');
    expect(after.retroBatches[0].settlementStatus).toBe('PAID');
    expect(after.retroBatches[0].totalGrossDelta).toBe('10.00');
    expect(after.retroAllocations[0].id).toBe('allocation-roundtrip');
    expect(after.retroAllocations[0].batchId).toBe('batch-roundtrip');
    expect(after.retroAllocations[0].deltaAmount).toBe('10.00');
    expect(after.bordrolar.filter((payroll) => payroll.accrualType === 'RETRO_ADJUSTMENT').length)
      .toBe(1);
    const retroPayroll = after.bordrolar.find((payroll) => payroll.accrualId === 'batch-roundtrip');
    expect(retroPayroll?.status).toBe('FINALIZED');
    expect(retroPayroll?.netOdeme).toBe('10.00');
  });

  test('normalizes a pre-accrual V2 backup to one NORMAL accrual', () => {
    const parsed = parseImportedBackup(makeLegacyV2Snapshot());
    const payroll = parsed.bordrolar[0];
    expect(parsed.backupVersion).toBe(5);
    expect(payroll.accrualId).toBe(payroll.id);
    expect(payroll.accrualType).toBe('NORMAL');
    expect(payroll.paymentDate).toBe('2026-02-14');
    expect(payroll.sequence).toBe(0);
    expect(payroll.netOdeme).toBe('64179.78');
  });

  test('requires every current V5 top-level field and preserves unknown fields', () => {
    const missingCollection = parseTestSnapshot(makeV2Snapshot());
    delete missingCollection.taxOpenings;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(missingCollection))).toThrow(
      '$.taxOpenings zorunlu alan eksik'
    );

    const wrongSettingsType = parseTestSnapshot(makeV2Snapshot());
    wrongSettingsType.kurumDegerleriMap = [];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(wrongSettingsType))).toThrow(
      '$.kurumDegerleriMap plain object olmalıdır'
    );

    const withUnknownField = parseTestSnapshot(makeV2Snapshot());
    withUnknownField.forwardCompatibleMetadata = { source: 'future-version' };
    const parsed = parseCurrentBrowserSnapshot(JSON.stringify(withUnknownField)) as TestRecord;
    expect(parsed.forwardCompatibleMetadata).toEqual({ source: 'future-version' });
  });

  test('rejects numeric and malformed Decimal values in current IndexedDB snapshots', () => {
    const invalidValues = [
      ['numeric Decimal', 64179.78, 'JS number kabul edilmez'],
      ['locale-formatted Decimal', '64.179,78', 'exact plain formatta'],
      ['non-Decimal text', 'not-a-decimal', 'exact plain formatta'],
    ] as const;

    for (const [_label, value, message] of invalidValues) {
      expect(() => parseCurrentBrowserSnapshot(makeV2Snapshot(value))).toThrow(message);
    }
  });

  test('rejects missing required domain fields with a path-aware error', () => {
    const missingPersonId = parseTestSnapshot(makeV2Snapshot());
    delete firstRecord(missingPersonId, 'personeller').id;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(missingPersonId))).toThrow(
      '$.personeller[0].id zorunlu alan eksik'
    );

    const wrongPersonIdType = parseTestSnapshot(makeV2Snapshot());
    firstRecord(wrongPersonIdType, 'personeller').id = 123;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(wrongPersonIdType))).toThrow(
      '$.personeller[0].id string olmalıdır'
    );

    const missingPayrollField = parseTestSnapshot(makeV2Snapshot());
    delete firstRecord(missingPayrollField, 'bordrolar').netOdeme;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(missingPayrollField))).toThrow(
      '$.bordrolar[0].netOdeme zorunlu alan eksik'
    );
  });

  test('rejects malformed nested collections, objects, and enums', () => {
    const invalidAttendance = parseTestSnapshot(makeV2Snapshot());
    firstRecord(invalidAttendance, 'puantajlar').gunler = [1, 2, 3];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidAttendance))).toThrow(
      '$.puantajlar[0].gunler plain object olmalıdır'
    );

    const invalidIncome = parseTestSnapshot(makeV2Snapshot());
    firstRecord(invalidIncome, 'bordrolar').gelirler = 'not-object';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidIncome))).toThrow(
      '$.bordrolar[0].gelirler plain object olmalıdır'
    );

    const invalidDeductions = parseTestSnapshot(makeV2Snapshot());
    firstRecord(invalidDeductions, 'bordrolar').kesintiler = 'not-object';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidDeductions))).toThrow(
      '$.bordrolar[0].kesintiler plain object olmalıdır'
    );

    const invalidPek = parseTestSnapshot(makeV2Snapshot());
    firstRecord(invalidPek, 'bordrolar').pekDetay = [];
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidPek))).toThrow(
      '$.bordrolar[0].pekDetay plain object olmalıdır'
    );

    const missingGvField = parseTestSnapshot(makeV2Snapshot());
    const gvDetay = firstRecord(missingGvField, 'bordrolar').gvDetay as TestRecord;
    delete gvDetay.cariGvMatrahi;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(missingGvField))).toThrow(
      '$.bordrolar[0].gvDetay.cariGvMatrahi zorunlu alan eksik'
    );

    const invalidStatus = parseTestSnapshot(makeV2Snapshot());
    firstRecord(invalidStatus, 'bordrolar').status = 'DONE';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidStatus))).toThrow(
      '$.bordrolar[0].status geçersiz enum değeri: DONE'
    );
  });

  test('rejects missing tax-bracket fields, duplicate identities, and dangling references', () => {
    const missingRate = parseTestSnapshot(makeV2Snapshot());
    delete firstRecord(firstRecord(missingRate, 'annualPayrollParameters'), 'gelirVergisiDilimleri').oran;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(missingRate))).toThrow(
      '$.annualPayrollParameters[0].gelirVergisiDilimleri[0].oran zorunlu alan eksik'
    );

    const duplicatePeriod = parseTestSnapshot(makeV2Snapshot());
    (duplicatePeriod.donemler as TestRecord[]).push({
      ...(duplicatePeriod.donemler as TestRecord[])[0],
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicatePeriod))).toThrow(
      '$.donemler[1] duplicate id'
    );

    const danglingAttendance = parseTestSnapshot(makeV2Snapshot());
    firstRecord(danglingAttendance, 'puantajlar').personelId = 'missing-person';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(danglingAttendance))).toThrow(
      '$.puantajlar[0].personelId mevcut olmayan personel kimliği'
    );
  });

  test('keeps legacy migration and import compatibility only after full validation', () => {
    const legacyV1 = parseTestSnapshot(makeV2Snapshot(64179.78));
    legacyV1.backupVersion = 1;
    delete legacyV1.exportedAt;
    delete legacyV1.aktifDonemId;
    delete legacyV1.kurumDegerleriMap;
    delete legacyV1.puantajlar;
    delete legacyV1.taxOpenings;
    delete legacyV1.sickLeaveRecords;
    delete legacyV1.annualPayrollParameters;
    delete legacyV1.zamAylari;
    delete firstRecord(legacyV1, 'bordrolar').status;

    expect(isSupportedLegacyBackupPayload(JSON.stringify(legacyV1))).toBe(true);
    expect(parseLegacyBackup(JSON.stringify(legacyV1)).bordrolar[0].netOdeme).toBe('64179.78');
    expect(parseImportedBackup(JSON.stringify(legacyV1)).bordrolar[0].netOdeme).toBe('64179.78');

    const currentV2WithNumber = parseTestSnapshot(makeV2Snapshot(64179.78));
    expect(() => parseImportedBackup(JSON.stringify(currentV2WithNumber))).toThrow(
      '$.bordrolar[0].gelirler.tabanBrutAylik Decimal değeri string olmalıdır'
    );
    expect(isSupportedLegacyBackupPayload(JSON.stringify(currentV2WithNumber))).toBe(false);

    const currentV2WithPartialSummary = parseTestSnapshot(makeV2Snapshot());
    delete (firstRecord(currentV2WithPartialSummary, 'bordrolar').puantajOzeti as TestRecord).T;
    expect(() => parseImportedBackup(JSON.stringify(currentV2WithPartialSummary))).toThrow(
      '$.bordrolar[0].puantajOzeti.T zorunlu alan eksik'
    );

    const malformedCurrentShape = {
      backupVersion: 2,
      exportedAt: '2026-02-14T10:00:00.000Z',
      donemler: [],
      personeller: [],
      bordrolar: [{ netOdeme: 64179.78 }],
    };
    expect(isSupportedLegacyBackupPayload(JSON.stringify(malformedCurrentShape))).toBe(false);
    expect(() => parseImportedBackup(JSON.stringify(malformedCurrentShape))).toThrow(
      '$.bordrolar[0].id zorunlu alan eksik'
    );
  });

  test('canonicalizes legacy periods missing taxYear or taxMonth', () => {
    const legacy = parseTestSnapshot(makeLegacyV2Snapshot());
    const period = firstRecord(legacy, 'donemler');
    delete period.taxYear;
    delete period.taxMonth;

    const parsed = parseImportedBackup(JSON.stringify(legacy));
    expect(parsed.donemler[0].taxYear).toBe(2026);
    expect(parsed.donemler[0].taxMonth).toBe(2);
  });

  test('rejects orphan records in normal legacy imports and prunes them only in explicit repair mode', () => {
    const legacy = parseTestSnapshot(makeLegacyV2Snapshot());
    (legacy.bordrolar as unknown[]).push({
      id: 'ghost_2026-01',
      personelId: 'ghost-person',
      donemId: '2026-01',
      olusturulmaTarihi: '2026-01-15T00:00:00.000Z',
      sonGuncellemeTarihi: '2026-01-15T00:00:00.000Z',
      puantajOzeti: { 'Ç': 20, 'T': 4, 'G': 0, 'İ': 0, 'GÇ': 0, 'GÇT': 0, 'R': 0 },
      gelirler: { tabanBrutAylik: '30000.00' },
      gelirToplam: '30000.00',
      kesintiler: { isciSgkPrimi: '4200.00', isciIssizlikPrimi: '300.00', gelirVergisi: '3825.00', damgaVergisi: '227.70' },
      kesintiToplam: '8552.70',
      netOdeme: '21447.30',
      status: 'CALCULATED',
    });

    expect(() => parseImportedBackup(JSON.stringify(legacy))).toThrow(
      'mevcut olmayan personel kimliği'
    );
    const repaired = repairAndCanonicalizeBackup(legacy);
    expect(repaired.bordrolar.length).toBe(1);
    expect(repaired.bordrolar[0].personelId).toBe('person-1');
  });

  test('repairAndCanonicalizeBackup repairs a payload with numeric fields and missing accrual info', () => {
    const rawWithNumbers = parseTestSnapshot(makeV2Snapshot(64179.78));
    rawWithNumbers.backupVersion = 2;
    delete firstRecord(rawWithNumbers, 'bordrolar').accrualId;
    delete firstRecord(rawWithNumbers, 'donemler').taxYear;
    delete firstRecord(rawWithNumbers, 'donemler').taxMonth;

    const repaired = repairAndCanonicalizeBackup(rawWithNumbers);
    expect(repaired.backupVersion).toBe(5);
    expect(typeof repaired.bordrolar[0].netOdeme).toBe('string');
    expect(repaired.bordrolar[0].netOdeme).toBe('64179.78');
    expect(repaired.bordrolar[0].accrualType).toBe('NORMAL');
    expect(repaired.donemler[0].taxYear).toBe(2026);
  });

  test('serializes concurrent saves and lets the latest completed operation win', async () => {
    const queue = new SerializedWriteQueue();
    let persisted = '';
    let releaseFirst!: () => void;
    let markFirstStarted!: () => void;
    const firstStarted = new Promise<void>((resolve) => {
      markFirstStarted = resolve;
    });

    const first = queue.enqueue(
      () =>
        new Promise<void>((resolve) => {
          markFirstStarted();
          releaseFirst = () => {
            persisted = 'A';
            resolve();
          };
        })
    );
    const second = queue.enqueue(async () => {
      persisted = 'B';
    });

    expect(persisted).toBe('');
    await firstStarted;
    releaseFirst();
    await Promise.all([first, second]);
    expect(persisted).toBe('B');
  });

  test('continues serving later saves after one write fails', async () => {
    const queue = new SerializedWriteQueue();
    const error = await queue
      .enqueue(async () => Promise.reject(new Error('write failed')))
      .catch((caught) => caught);
    expect(String(error).includes('write failed')).toBe(true);

    let persisted = false;
    await queue.enqueue(async () => {
      persisted = true;
    });
    expect(persisted).toBe(true);
  });

  test('reconciles clean/dirty/pending browser tabs without silent last-write-wins', () => {
    expect(shouldAdoptRemoteSnapshot({
      currentRevision: 4,
      remoteRevision: 5,
      localDirty: false,
      pendingWrite: false,
    })).toBe(true);

    // A local edit or an async write in flight protects the local state. The
    // caller records the remote snapshot as a conflict and keeps the old CAS
    // baseline until the user explicitly reloads it.
    expect(shouldAdoptRemoteSnapshot({
      currentRevision: 4,
      remoteRevision: 5,
      localDirty: true,
      pendingWrite: false,
    })).toBe(false);
    expect(shouldAdoptRemoteSnapshot({
      currentRevision: 4,
      remoteRevision: 5,
      localDirty: false,
      pendingWrite: true,
    })).toBe(false);
  });

  test('does not accept a delayed load below an already observed revision', () => {
    expect(meetsSnapshotRevisionFloor(null, 0)).toBe(true);
    expect(meetsSnapshotRevisionFloor(null, 1)).toBe(false);
    expect(meetsSnapshotRevisionFloor(4, 5)).toBe(false);
    expect(meetsSnapshotRevisionFloor(5, 5)).toBe(true);
    expect(meetsSnapshotRevisionFloor(6, 5)).toBe(true);
  });

  test('keeps CAS conflicts explicit for a stale dirty tab', () => {
    const conflict = new BrowserSnapshotConflictError(4, 5);
    expect(conflict.expectedRevision).toBe(4);
    expect(conflict.actualRevision).toBe(5);
    expect(conflict.name).toBe('BrowserSnapshotConflictError');
  });
});

describe('SQLite persistence invariant parity', () => {
  test('accepts the unique realistic snapshot', () => {
    expect(() => parseCurrentBrowserSnapshot(makeV2Snapshot())).not.toThrow();
  });

  test('rejects negative ordinary income and deduction line items at the current boundary', () => {
    const negativeIncome = parseTestSnapshot(makeV2Snapshot('3000.00'));
    (firstRecord(negativeIncome, 'bordrolar').gelirler as TestRecord).digerGelir = '-0.01';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(negativeIncome))).toThrow(
      'ordinary payroll line item negatif olamaz'
    );

    const negativeDeduction = parseTestSnapshot(makeV2Snapshot('3000.00'));
    (firstRecord(negativeDeduction, 'bordrolar').kesintiler as TestRecord).icra = '-10.00';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(negativeDeduction))).toThrow(
      'ordinary payroll line item negatif olamaz'
    );
  });

  test('rejects invalid puantaj dates, codes, and period bounds', () => {
    const invalidDate = parseTestSnapshot(makeV2Snapshot());
    firstRecord(invalidDate, 'puantajlar').gunler = { '2026-02-30': 'Ç' };
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidDate))).toThrow(
      'puantaj tarihi YYYY-MM-DD biçiminde geçerli bir tarih olmalıdır: 2026-02-30'
    );

    const invalidCode = parseTestSnapshot(makeV2Snapshot());
    firstRecord(invalidCode, 'puantajlar').gunler = { '2026-01-15': 'INVALID_CODE' };
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidCode))).toThrow(
      'desteklenmeyen puantaj kodu: INVALID_CODE'
    );

    const outOfBounds = parseTestSnapshot(makeV2Snapshot());
    firstRecord(outOfBounds, 'puantajlar').gunler = { '2025-12-31': 'Ç' };
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(outOfBounds))).toThrow(
      'aralığı dışında'
    );
  });

  test('rejects invalid sick leave record dates, order, and overlapping ranges', () => {
    const invalidDate = parseTestSnapshot(makeV2Snapshot());
    (invalidDate.sickLeaveRecords as TestRecord[]).push({
      id: 'sick-invalid-date',
      personnelId: 'person-1',
      startDate: '2026-02-30',
      endDate: '2026-03-01',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidDate))).toThrow(
      'geçerli bir tarih olmalıdır: 2026-02-30'
    );

    const invalidOrder = parseTestSnapshot(makeV2Snapshot());
    (invalidOrder.sickLeaveRecords as TestRecord[]).push({
      id: 'sick-invalid-order',
      personnelId: 'person-1',
      startDate: '2026-02-10',
      endDate: '2026-02-05',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(invalidOrder))).toThrow(
      'Rapor başlangıç tarihi bitiş tarihinden sonra olamaz: 2026-02-10 > 2026-02-05'
    );

    const overlapping = parseTestSnapshot(makeV2Snapshot());
    (overlapping.sickLeaveRecords as TestRecord[]).push(
      {
        id: 'sick-overlap-1',
        personnelId: 'person-1',
        startDate: '2026-02-01',
        endDate: '2026-02-05',
      },
      {
        id: 'sick-overlap-2',
        personnelId: 'person-1',
        startDate: '2026-02-04',
        endDate: '2026-02-10',
      }
    );
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(overlapping))).toThrow(
      'Rapor tarihleri çakışıyor'
    );
  });

  test('RETRO allows signed worker SGK deltas but rejects negative ordinary income and deductions', () => {
    const retroPayload = parseTestSnapshot(makeV2Snapshot());
    const retroPayroll = {
      ...firstRecord(retroPayload, 'bordrolar'),
      id: 'payroll-retro-1',
      accrualId: 'batch-retro-1',
      accrualType: 'RETRO_ADJUSTMENT',
      paymentDate: '2026-02-14',
      sequence: 1,
      gelirler: {
        ...(firstRecord(retroPayload, 'bordrolar').gelirler as TestRecord),
        tabanBrutAylik: '100.00',
        digerGelir: '-50.00',
      },
      gelirToplam: '50.00',
      kesintiler: {
        ...(firstRecord(retroPayload, 'bordrolar').kesintiler as TestRecord),
        gelirVergisi: '20.00',
        isciSgkPrimi: '-14.00',
      },
      kesintiToplam: '6.00',
      netOdeme: '44.00',
      persistedGvBase: '100.00',
      gvDetay: {
        ...(firstRecord(retroPayload, 'bordrolar').gvDetay as TestRecord),
        cariGvMatrahi: '100.00',
      },
    };
    (retroPayload.bordrolar as TestRecord[]).push(retroPayroll);
    (retroPayload.retroBatches as TestRecord[]).push({
      id: 'batch-retro-1',
      revisionId: 'rev-1',
      personnelId: 'person-1',
      paymentDate: '2026-02-14',
      totalGrossDelta: '100.00',
      settlementStatus: 'UNSETTLED',
      status: 'CALCULATED',
      ...retroBatchSettlement('100.00'),
    });
    (retroPayload.retroAllocations as TestRecord[]).push({
      id: 'alloc-retro-1',
      batchId: 'batch-retro-1',
      personnelId: 'person-1',
      sourcePeriodId: '2026-01',
      earningCode: 'BASE_WAGE',
      originalRecognizedAmount: '0.00',
      targetAmount: '100.00',
      deltaAmount: '100.00',
      sgkTreatment: 'WAGE_SOURCE_MONTH',
      incomeTaxTreatment: 'TAXABLE',
      stampTaxTreatment: 'TAXABLE',
      ...retroAllocationSettlement('100.00'),
    });
    (retroPayload.compensationRevisions as TestRecord[]).push({
      id: 'rev-1',
      title: 'Rev 1',
      status: 'DRAFT',
      reason: 'COLLECTIVE_AGREEMENT',
      scope: 'ALL_PERSONNEL',
      effectiveFrom: '2026-01-01',
      targetYear: 2026,
      targetMonth: 1,
      parameters: {},
    });

    // Negative digerGelir in retro must fail
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(retroPayload))).toThrow(
      'ordinary payroll line item negatif olamaz'
    );

    // Fix digerGelir to 0, signed isciSgkPrimi (-14.00) must succeed
    (retroPayroll.gelirler as TestRecord).digerGelir = '0.00';
    retroPayroll.gelirToplam = '100.00';
    retroPayroll.netOdeme = '94.00';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(retroPayload))).not.toThrow();

    // Negative bes in retro must fail
    (retroPayroll.kesintiler as TestRecord).bes = '-10.00';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(retroPayload))).toThrow(
      'ordinary payroll line item negatif olamaz'
    );
  });

  test('rejects a current GV base scalar that disagrees with the snapshot detail', () => {
    const payload = parseTestSnapshot(makeV2Snapshot('3000.00'));
    firstRecord(payload, 'bordrolar').persistedGvBase = '3001.00';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(payload))).toThrow(
      'persisted GV matrahı ile GV snapshot cari matrahı eşleşmiyor'
    );
  });

  test('rejects a current authoritative snapshot without the persisted GV base', () => {
    const payload = parseTestSnapshot(makeV2Snapshot('3000.00'));
    delete firstRecord(payload, 'bordrolar').persistedGvBase;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(payload))).toThrow(
      'current authoritative snapshot persisted GV matrahı ile GV snapshot cari matrahını birlikte taşımalıdır'
    );
  });

  test('repairs an unambiguous legacy person opening into the canonical pair', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    payload.taxOpenings = [];
    const person = firstRecord(payload, 'personeller');
    person.devirKumulatifGvMatrahi = '185000';
    person.devirKumulatifGvMatrahiYili = 2026;
    person.devirKumulatifGvMatrahiBaslangicAyi = 1;

    const repaired = parseCurrentBrowserSnapshot(JSON.stringify(payload));
    expect(repaired.taxOpenings.length).toBe(1);
    expect(repaired.taxOpenings[0].personnelId).toBe('person-1');
    expect(repaired.taxOpenings[0].year).toBe(2026);
    expect(repaired.taxOpenings[0].gvCumulativeOpening).toBe('185000');
    expect(repaired.taxOpenings[0].effectiveFromPeriodId).toBe('2026-01');
  });

  test('repairs an unambiguous legacy asgari opening without inventing a normal pair', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    payload.taxOpenings = [];
    const person = firstRecord(payload, 'personeller');
    person.devirKumulatifAsgariGvMatrahi = '90000';
    person.devirKumulatifAsgariGvMatrahiYili = 2026;
    person.devirKumulatifGvMatrahi = '0.00';
    person.devirKumulatifGvMatrahiYili = 2026;
    person.devirKumulatifGvMatrahiBaslangicAyi = 1;

    const repaired = parseCurrentBrowserSnapshot(JSON.stringify(payload));
    expect(repaired.taxOpenings.length).toBe(1);
    expect(repaired.taxOpenings[0].asgariGvCumulativeOpening).toBe('90000');
    expect(repaired.taxOpenings[0].asgariGvEffectiveFromPeriodId).toBe('2026-01');
    expect(repaired.taxOpenings[0].gvCumulativeOpening).toBe(undefined);
    expect(repaired.taxOpenings[0].effectiveFromPeriodId).toBe(undefined);
  });

  test('does not overwrite an explicit canonical opening with legacy person fields', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const person = firstRecord(payload, 'personeller');
    person.devirKumulatifGvMatrahi = '185000';
    person.devirKumulatifGvMatrahiYili = 2026;
    person.devirKumulatifGvMatrahiBaslangicAyi = 1;

    const parsed = parseCurrentBrowserSnapshot(JSON.stringify(payload));
    expect(parsed.taxOpenings[0].gvCumulativeOpening).toBe('0.00');
    expect(parsed.taxOpenings[0].effectiveFromPeriodId).toBe('2026-01');
  });

  test('does not guess an ambiguous legacy opening period', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    payload.taxOpenings = [];
    (payload.donemler as TestRecord[]).push({
      ...firstRecord(payload, 'donemler'),
      id: '2026-01-ambiguous',
      taxMonth: 3,
    });
    const person = firstRecord(payload, 'personeller');
    person.devirKumulatifGvMatrahi = '185000';
    person.devirKumulatifGvMatrahiYili = 2026;
    person.devirKumulatifGvMatrahiBaslangicAyi = 1;

    const repaired = parseCurrentBrowserSnapshot(JSON.stringify(payload));
    expect(repaired.taxOpenings).toEqual([]);
    expect(repaired.personeller[0].devirKumulatifGvMatrahi).toBe('185000');
  });

  test('keeps an explicit zero canonical opening and its period', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const opening = firstRecord(payload, 'taxOpenings');
    opening.gvCumulativeOpening = '0';
    opening.effectiveFromPeriodId = '2026-01';

    const repaired = repairLegacyPersonTaxOpenings(
      parseCurrentBrowserSnapshot(JSON.stringify(payload))
    );
    expect(repaired.taxOpenings[0].gvCumulativeOpening).toBe('0');
    expect(repaired.taxOpenings[0].effectiveFromPeriodId).toBe('2026-01');
  });

  test('rejects duplicate personel.tcNo, including duplicate empty values', () => {
    const duplicate = parseTestSnapshot(makeV2Snapshot());
    (duplicate.personeller as TestRecord[]).push({
      ...firstRecord(duplicate, 'personeller'),
      id: 'person-2',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicate))).toThrow(
      '$.personeller[1].tcNo duplicate tcNo: 10000000000; ilk kayıt $.personeller[0].tcNo'
    );

    const emptyAllowed = parseTestSnapshot(makeV2Snapshot());
    firstRecord(emptyAllowed, 'personeller').tcNo = '';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(emptyAllowed))).not.toThrow();
    const duplicateEmpty = parseTestSnapshot(JSON.stringify(emptyAllowed));
    (duplicateEmpty.personeller as TestRecord[]).push({
      ...firstRecord(duplicateEmpty, 'personeller'),
      id: 'person-2',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicateEmpty))).toThrow(
      '$.personeller[1].tcNo duplicate tcNo: ; ilk kayıt $.personeller[0].tcNo'
    );
  });

  test('rejects duplicate attendance personelId+donemId', () => {
    const duplicate = parseTestSnapshot(makeV2Snapshot());
    (duplicate.puantajlar as TestRecord[]).push({
      ...firstRecord(duplicate, 'puantajlar'),
      id: 'attendance-b',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicate))).toThrow(
      '$.puantajlar[1] duplicate (personelId, donemId): person-1 / 2026-01; ilk kayıt $.puantajlar[0]'
    );
  });

  test('rejects duplicate payroll personelId+donemId', () => {
    const duplicate = parseTestSnapshot(makeV2Snapshot());
    (duplicate.bordrolar as TestRecord[]).push({
      ...firstRecord(duplicate, 'bordrolar'),
      id: 'payroll-b',
      accrualId: 'payroll-b',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicate))).toThrow(
      '$.bordrolar[1] duplicate (personelId, donemId): person-1 / 2026-01; ilk kayıt $.bordrolar[0]'
    );
  });

  test('allows multiple supplementary accruals but keeps normal and tie-breaker invariants', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const normal = firstRecord(payload, 'bordrolar');
    (payload.bordrolar as TestRecord[]).push({
      ...normal,
      id: 'payroll-tediye',
      accrualId: 'payroll-tediye',
      accrualType: 'TEDIYE',
      paymentDate: '2026-02-14',
      sequence: 1,
      gelirler: { ...(normal.gelirler as TestRecord), tabanBrutAylik: '0.00', tediye: '100.00' },
      gelirToplam: '100.00',
      netOdeme: '100.00',
    });
    (payload.bordrolar as TestRecord[]).push({
      ...normal,
      id: 'payroll-tis',
      accrualId: 'payroll-tis',
      accrualType: 'TIS_IKRAMIYE',
      paymentDate: '2026-02-14',
      sequence: 2,
      gelirler: { ...(normal.gelirler as TestRecord), tabanBrutAylik: '0.00', tisIkramiyesi: '100.00' },
      gelirToplam: '100.00',
      netOdeme: '100.00',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(payload))).not.toThrow();

    const duplicateTie = parseTestSnapshot(JSON.stringify(payload));
    const duplicate = firstRecord(duplicateTie, 'bordrolar');
    duplicate.id = 'payroll-duplicate-tie';
    duplicate.accrualId = 'payroll-duplicate-tie';
    duplicate.accrualType = 'TEDIYE';
    duplicate.paymentDate = '2026-02-14';
    duplicate.sequence = 1;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicateTie))).toThrow(
      'duplicate (personelId, paymentDate, sequence)'
    );
  });

  test('payment-event backup preserves TEDIYE seq0 before NORMAL seq1', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const normal = firstRecord(payload, 'bordrolar');
    normal.sequence = 1;
    (payload.bordrolar as TestRecord[]).push({
      ...normal, id: 'early-tediye', accrualId: 'early-tediye', accrualType: 'TEDIYE', sequence: 0,
    });
    const parsed = parseCurrentBrowserSnapshot(JSON.stringify(payload));
    expect(parsed.bordrolar.map((item) => item.sequence)).toEqual([1, 0]);
    expect(parsed.bordrolar.map((item) => item.accrualType)).toEqual(['NORMAL', 'TEDIYE']);
  });

  test('rejects duplicate tax opening personnelId+year', () => {
    const duplicate = parseTestSnapshot(makeV2Snapshot());
    (duplicate.taxOpenings as TestRecord[]).push({
      ...firstRecord(duplicate, 'taxOpenings'),
      id: 'opening-2',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicate))).toThrow(
      '$.taxOpenings[1] duplicate (personnelId, year): person-1 / 2026; ilk kayıt $.taxOpenings[0]'
    );
  });

  test('accepts an asgari-only tax opening without creating a normal opening', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    const opening = firstRecord(payload, 'taxOpenings');
    delete opening.gvCumulativeOpening;
    delete opening.effectiveFromPeriodId;
    opening.asgariGvCumulativeOpening = '100000.00';
    opening.asgariGvEffectiveFromPeriodId = '2026-01';

    const parsed = parseCurrentBrowserSnapshot(JSON.stringify(payload));
    expect(parsed.taxOpenings[0].gvCumulativeOpening).toBe(undefined);
    expect(parsed.taxOpenings[0].effectiveFromPeriodId).toBe(undefined);
    expect(parsed.taxOpenings[0].asgariGvCumulativeOpening).toBe('100000.00');
  });

  test('legacy shared asgari effective period is normalized only in restore', () => {
    const payload = parseTestSnapshot(makeV2Snapshot());
    payload.backupVersion = 4;
    const opening = firstRecord(payload, 'taxOpenings');
    opening.asgariGvCumulativeOpening = '100000.00';
    delete opening.asgariGvEffectiveFromPeriodId;

    const restored = parseLegacyBackup(JSON.stringify(payload));
    expect(restored.taxOpenings[0].asgariGvEffectiveFromPeriodId).toBe('2026-01');
    const currentPayload = { ...payload, backupVersion: 5 };
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(currentPayload))).toThrow(
      'asgari GV opening değeri ile asgari effectiveFromPeriodId birlikte tanımlanmalıdır'
    );
  });

  test('rejects duplicate annualPayrollParameters.year', () => {
    const duplicate = parseTestSnapshot(makeV2Snapshot());
    (duplicate.annualPayrollParameters as TestRecord[]).push({
      ...firstRecord(duplicate, 'annualPayrollParameters'),
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicate))).toThrow(
      '$.annualPayrollParameters[1].year duplicate year: 2026; ilk kayıt $.annualPayrollParameters[0].year'
    );
  });

  test('rejects duplicate period taxYear+taxMonth even with different ids', () => {
    const duplicate = parseTestSnapshot(makeV2Snapshot());
    (duplicate.donemler as TestRecord[]).push({
      ...firstRecord(duplicate, 'donemler'),
      id: '2026-02',
      yil: 2026,
      ay: 2,
      baslangicTarihi: '2026-02-15',
      bitisTarihi: '2026-03-14',
      donemAdi: 'Şubat 2026',
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicate))).toThrow(
      '$.donemler[1] duplicate (taxYear, taxMonth): 2026 / 2; ilk kayıt $.donemler[0]'
    );
  });

  test('rejects a dangling institution settings period key', () => {
    const dangling = parseTestSnapshot(makeV2Snapshot());
    const settings = (dangling.kurumDegerleriMap as TestRecord)['2026-01'] as TestRecord;
    dangling.kurumDegerleriMap = {
      ...(dangling.kurumDegerleriMap as TestRecord),
      'missing-period': { ...settings, donemId: 'missing-period' },
    };
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(dangling))).toThrow(
      '$.kurumDegerleriMap["missing-period"] mevcut olmayan dönem kimliği: missing-period'
    );
  });

  test('rejects a dangling active period but accepts the supported empty selection', () => {
    const dangling = parseTestSnapshot(makeV2Snapshot());
    dangling.aktifDonemId = 'missing-period';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(dangling))).toThrow(
      '$.aktifDonemId mevcut olmayan dönem kimliği: missing-period'
    );

    const empty = parseTestSnapshot(makeV2Snapshot());
    empty.aktifDonemId = '';
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(empty))).not.toThrow();
  });
});

describe('Legacy native Serde compatibility parity', () => {
  test('imports an older backup without nafaka and does not add nulls to its canonical output', () => {
    const legacy = makeLegacyV1Snapshot();
    const personDeductions = firstRecord(legacy, 'personeller').kesintiler as TestRecord;
    const payrollDeductions = firstRecord(legacy, 'bordrolar').kesintiler as TestRecord;
    delete personDeductions.nafakaTutar;
    delete payrollDeductions.nafaka;

    const parsed = parseLegacyBackup(JSON.stringify(legacy));
    expect(parsed.personeller[0].kesintiler?.nafakaTutar).toBeUndefined();
    expect(parsed.bordrolar[0].kesintiler.nafaka).toBeUndefined();

    const canonical = JSON.parse(serializePayrollStorage(parsed)) as TestRecord;
    expect(
      Object.prototype.hasOwnProperty.call(
        firstRecord(canonical, 'personeller').kesintiler as TestRecord,
        'nafakaTutar'
      )
    ).toBe(false);
    expect(
      Object.prototype.hasOwnProperty.call(
        firstRecord(canonical, 'bordrolar').kesintiler as TestRecord,
        'nafaka'
      )
    ).toBe(false);
  });

  test('defaults legacy V1 partial PuantajOzeti values to zero only in the legacy path', () => {
    const legacy = makeLegacyV1Snapshot();
    const summary = firstRecord(legacy, 'bordrolar').puantajOzeti as TestRecord;
    delete summary.T;
    delete summary.G;
    delete summary.İ;
    delete summary.GÇ;
    delete summary.GÇT;
    delete summary.R;

    const parsed = parseLegacyBackup(JSON.stringify(legacy));
    expect(parsed.bordrolar[0].puantajOzeti).toEqual({
      Ç: 1,
      T: 0,
      G: 0,
      İ: 0,
      GÇ: 0,
      GÇT: 0,
      R: 0,
    });

    const current = parseTestSnapshot(makeV2Snapshot());
    delete (firstRecord(current, 'bordrolar').puantajOzeti as TestRecord).T;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(current))).toThrow(
      '$.bordrolar[0].puantajOzeti.T zorunlu alan eksik'
    );
  });

  test('canonicalizes missing legacy optional Gelir/Kesinti fields to null', () => {
    const legacy = makeLegacyV1Snapshot();
    const bordro = firstRecord(legacy, 'bordrolar');
    const gelirler = bordro.gelirler as TestRecord;
    const kesintiler = bordro.kesintiler as TestRecord;
    [
      'tabanBrutAylik',
      'tediye',
      'tisIkramiyesi',
      'ekOdeme',
      'yemek',
      'birlestirilmisSosyalYardim',
      'vasitaYol',
      'giyimYardimi',
      'isPrimi',
      'geceCalismasiUcreti',
      'geceCalismasiTatiliUcreti',
      'hizmetZammi',
      'digerGelir',
    ].forEach((key) => delete gelirler[key]);
    [
      'isciSgkPrimi',
      'isciIssizlikPrimi',
      'gelirVergisi',
      'damgaVergisi',
      'sendikaAidati',
      'bes',
      'icra',
      'kisiBorcu',
      'dogumAskerlikBorclanmasi',
      'hayatSaglikSigortasi',
      'digerKesinti',
    ].forEach((key) => delete kesintiler[key]);

    const parsed = parseLegacyBackup(JSON.stringify(legacy));
    expect({
      tabanBrutAylik: parsed.bordrolar[0].gelirler.tabanBrutAylik,
      geceCalismasiUcreti: parsed.bordrolar[0].gelirler.geceCalismasiUcreti,
      digerGelir: parsed.bordrolar[0].gelirler.digerGelir,
    }).toEqual({
      tabanBrutAylik: null,
      geceCalismasiUcreti: null,
      digerGelir: null,
    });
    expect({
      isciSgkPrimi: parsed.bordrolar[0].kesintiler.isciSgkPrimi,
      hayatSaglikSigortasi: parsed.bordrolar[0].kesintiler.hayatSaglikSigortasi,
      digerKesinti: parsed.bordrolar[0].kesintiler.digerKesinti,
    }).toEqual({
      isciSgkPrimi: null,
      hayatSaglikSigortasi: null,
      digerKesinti: null,
    });
  });

  test('canonicalizes legacy Pek/GV serde(default) fields to Decimal zero', () => {
    const legacy = makeLegacyV1Snapshot();
    const bordro = firstRecord(legacy, 'bordrolar');
    const pek = bordro.pekDetay as TestRecord;
    const gv = bordro.gvDetay as TestRecord;
    ['hamPek', 'devredenPekKullanilan', 'primMatrahi', 'altSinirTamamlamaFarki'].forEach(
      (key) => delete pek[key]
    );
    [
      'dogumAskerlikGvIndirimi',
      'sigortaGvIndirimAdayi',
      'sigortaGvAylikLimiti',
      'sigortaGvYillikKalanLimiti',
      'uygulanabilirSigortaGvIndirimi',
    ].forEach((key) => delete gv[key]);

    const parsed = parseLegacyBackup(JSON.stringify(legacy));
    expect({
      hamPek: parsed.bordrolar[0].pekDetay?.hamPek,
      devredenPekKullanilan: parsed.bordrolar[0].pekDetay?.devredenPekKullanilan,
      primMatrahi: parsed.bordrolar[0].pekDetay?.primMatrahi,
      altSinirTamamlamaFarki: parsed.bordrolar[0].pekDetay?.altSinirTamamlamaFarki,
    }).toEqual({
      hamPek: '0',
      devredenPekKullanilan: '0',
      primMatrahi: '0',
      altSinirTamamlamaFarki: '0',
    });
    expect({
      dogumAskerlikGvIndirimi: parsed.bordrolar[0].gvDetay?.dogumAskerlikGvIndirimi,
      sigortaGvIndirimAdayi: parsed.bordrolar[0].gvDetay?.sigortaGvIndirimAdayi,
      sigortaGvAylikLimiti: parsed.bordrolar[0].gvDetay?.sigortaGvAylikLimiti,
      sigortaGvYillikKalanLimiti: parsed.bordrolar[0].gvDetay?.sigortaGvYillikKalanLimiti,
      uygulanabilirSigortaGvIndirimi: parsed.bordrolar[0].gvDetay?.uygulanabilirSigortaGvIndirimi,
    }).toEqual({
      dogumAskerlikGvIndirimi: '0',
      sigortaGvIndirimAdayi: '0',
      sigortaGvAylikLimiti: '0',
      sigortaGvYillikKalanLimiti: '0',
      uygulanabilirSigortaGvIndirimi: '0',
    });
  });

  test('matches native legacy annual-parameter defaults without making them a current FK rule', () => {
    const legacyWithoutParameters = makeLegacyV1Snapshot();
    delete legacyWithoutParameters.annualPayrollParameters;
    const canonical = parseLegacyBackup(JSON.stringify(legacyWithoutParameters));
    expect(canonical.annualPayrollParameters).toEqual([
      {
        year: 2026,
        gelirVergisiDilimleri: [
          { limit: '190000', oran: '0.15' },
          { limit: '400000', oran: '0.20' },
          { limit: '1500000', oran: '0.27' },
          { limit: '5300000', oran: '0.35' },
          { limit: '1000000000000000', oran: '0.40' },
        ],
        sigortaGvYillikBrutAsgariUcretTavani: '396360',
      },
    ]);

    const legacyWithMissing2026Cap = makeLegacyV1Snapshot();
    delete (firstRecord(legacyWithMissing2026Cap, 'annualPayrollParameters') as TestRecord)
      .sigortaGvYillikBrutAsgariUcretTavani;
    expect(parseLegacyBackup(JSON.stringify(legacyWithMissing2026Cap)).annualPayrollParameters[0]
      .sigortaGvYillikBrutAsgariUcretTavani).toBe('396360');

    const currentWithoutParameters = parseTestSnapshot(makeV2Snapshot());
    delete currentWithoutParameters.annualPayrollParameters;
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(currentWithoutParameters))).toThrow(
      '$.annualPayrollParameters zorunlu alan eksik'
    );
  });

  test('treats explicit null top-level Option fields like native LegacyPayload', () => {
    const legacy = makeLegacyV1Snapshot();
    legacy.backupVersion = null;
    legacy.exportedAt = null;
    legacy.aktifDonemId = null;
    legacy.kurumDegerleriMap = null;
    legacy.puantajlar = null;
    legacy.taxOpenings = null;
    legacy.sickLeaveRecords = null;
    legacy.annualPayrollParameters = null;
    legacy.zamAylari = null;

    const parsed = parseLegacyBackup(JSON.stringify(legacy));
    expect(parsed.aktifDonemId).toBe('2026-01');
    expect(parsed.kurumDegerleriMap).toEqual({});
    expect(parsed.puantajlar).toEqual([]);
    expect(parsed.taxOpenings).toEqual([]);
    expect(parsed.sickLeaveRecords).toEqual([]);
    expect(parsed.annualPayrollParameters.length).toBe(1);
    expect(parsed.zamAylari).toEqual([]);
  });

  test('defaults legacy missing IsPrimiGrupItem.aktif to true and respects native map-key ownership', () => {
    const legacy = makeLegacyV1Snapshot();
    const settingsMap = legacy.kurumDegerleriMap as TestRecord;
    const settings = settingsMap['2026-01'] as TestRecord;
    (settings.isPrimiGruplari as TestRecord[])[0].aktif = undefined;
    delete (settings.isPrimiGruplari as TestRecord[])[0].aktif;
    settings.donemId = 'legacy-period-id';

    const parsed = parseLegacyBackup(JSON.stringify(legacy));
    expect(parsed.kurumDegerleriMap['2026-01'].donemId).toBe('2026-01');
    expect(parsed.kurumDegerleriMap['2026-01'].isPrimiGruplari?.[0].aktif).toBe(true);
  });

  test('rejects explicit invalid legacy types and never repairs duplicate identities', () => {
    const invalidType = makeLegacyV1Snapshot();
    (firstRecord(invalidType, 'bordrolar').puantajOzeti as TestRecord).T = '20';
    expect(() => parseLegacyBackup(JSON.stringify(invalidType))).toThrow(
      '$.bordrolar[0].puantajOzeti.T tam sayı olmalıdır'
    );

    const invalidOptionalDecimal = makeLegacyV1Snapshot();
    (firstRecord(invalidOptionalDecimal, 'bordrolar').gelirler as TestRecord).yemek = {};
    expect(() => parseLegacyBackup(JSON.stringify(invalidOptionalDecimal))).toThrow();

    const duplicate = makeLegacyV1Snapshot();
    (duplicate.puantajlar as TestRecord[]).push({
      ...firstRecord(duplicate, 'puantajlar'),
      id: 'attendance-b',
    });
    expect(() => parseLegacyBackup(JSON.stringify(duplicate))).toThrow(
      '$.puantajlar[1] duplicate (personelId, donemId): person-1 / 2026-01; ilk kayıt $.puantajlar[0]'
    );
  });
});
