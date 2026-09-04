import { describe, expect, test } from 'bun:test';
import {
  browserPayrollStore,
  canonicalizeLegacyBackupPayload,
  isMigratableBackupPayload,
  SerializedWriteQueue,
} from './browserPayrollStore';
import {
  isSupportedLegacyBackupPayload,
  parseCurrentBrowserSnapshot,
  parseImportedBackup,
  parseLegacyBackup,
} from './payrollPayload';
import type { PayrollStorageDto } from '../payrollEngine/decimalBoundary';

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
      cariGvMatrahi: '3000.00',
      yeniKumulatifGvMatrahi: '3000.00',
      brutGelirVergisi: '450.00',
      asgariUcretGvMatrahi: '0.00',
      asgariUcretReferansKumulatifMatrahi: '0.00',
      asgariUcretGvIstisnasi: '0.00',
      uygulananGvIstisnasi: '0.00',
      kesilenGelirVergisi: '0.00',
      dogumAskerlikGvIndirimi: '0.00',
      sigortaGvIndirimAdayi: '0.00',
      sigortaGvAylikLimiti: '0.00',
      sigortaGvYillikKalanLimiti: '0.00',
      uygulanabilirSigortaGvIndirimi: '0.00',
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
    backupVersion: 2,
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
  };
}

function makeV2Snapshot(netOdeme: unknown = '64179.78'): string {
  const snapshot = makeRealisticSnapshot() as unknown as TestRecord;
  firstRecord(snapshot, 'bordrolar').netOdeme = netOdeme;
  return JSON.stringify(snapshot);
}

function parseTestSnapshot(json: string): TestRecord {
  return JSON.parse(json) as TestRecord;
}

function firstRecord(snapshot: TestRecord, collection: string): TestRecord {
  return (snapshot[collection] as TestRecord[])[0];
}

function makeLegacyV1Snapshot(netOdeme: unknown = '64179.78'): TestRecord {
  const legacy = parseTestSnapshot(makeV2Snapshot(netOdeme));
  legacy.backupVersion = 1;
  return legacy;
}

describe('BrowserPayrollStore', () => {
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
    expect(isMigratableBackupPayload(JSON.stringify({ backupVersion: 3 }))).toBe(false);
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
    expect(canonicalPayload.backupVersion).toBe(2);
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

  test('accepts an exact, realistic current V2 snapshot', () => {
    const parsed = parseCurrentBrowserSnapshot(makeV2Snapshot());

    expect(parsed.backupVersion).toBe(2);
    expect(parsed.donemler.length).toBe(1);
    expect(parsed.personeller.length).toBe(1);
    expect(parsed.puantajlar.length).toBe(1);
    expect(parsed.bordrolar.length).toBe(1);
    expect(parsed.taxOpenings.length).toBe(1);
    expect(parsed.annualPayrollParameters.length).toBe(1);
    expect(parsed.bordrolar[0].netOdeme).toBe('64179.78');
  });

  test('requires every current V2 top-level field and preserves unknown fields', () => {
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
      '$.bordrolar[0].netOdeme Decimal değeri string olmalıdır'
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
      '$.aktifDonemId zorunlu alan eksik'
    );
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
});

describe('SQLite persistence invariant parity', () => {
  test('accepts the unique realistic snapshot', () => {
    expect(() => parseCurrentBrowserSnapshot(makeV2Snapshot())).not.toThrow();
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
    });
    expect(() => parseCurrentBrowserSnapshot(JSON.stringify(duplicate))).toThrow(
      '$.bordrolar[1] duplicate (personelId, donemId): person-1 / 2026-01; ilk kayıt $.bordrolar[0]'
    );
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
