/**
 * 4/D Sürekli İşçi Bordro Programı — Main App Component
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Navbar, TabType } from './components/Navbar';
import { PersonelList } from './components/PersonelList';
import { PuantajGrid } from './components/PuantajGrid';
import { BordroHesaplama } from './components/BordroHesaplama';
import { BankaListesi } from './components/Listeler/BankaListesi';
import { KesintiListesi } from './components/Listeler/KesintiListesi';
import { PeriodManagerModal } from './components/PeriodManagerModal';
import {
  AnnualPayrollParameters,
  BACKUP_FORMAT_VERSION,
  BackupPayload,
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  SickLeaveRecord,
  ZAM_AYLARI_SETTING_KEY,
} from './types/payroll';
import { tauriBridge } from './services/tauriBridge';
import { browserPayrollStore } from './services/storage/browserPayrollStore';
import {
  applyBrowserPayrollImpact,
  assertBrowserMutationImpactAllowed,
} from './services/storage/browserPayrollPolicies';
import { getPayrollEngine } from './services/payrollEngine';
import type { MutationImpact, PayrollMutation } from './services/payrollEngine';
import { parsePayrollStorage, serializePayrollStorage } from './services/payrollEngine/decimalBoundary';
import { PayrollNoticeCenter } from './components/PayrollNoticeCenter';
import { getInitialDataset } from './utils/sampleData';

const STORAGE_KEY = '4d_bordro_programi_mvp_v2';

type DatasetFields = Omit<BackupPayload, 'backupVersion' | 'exportedAt'>;

function normalizeZamAylari(value: unknown): number[] {
  if (!Array.isArray(value)) return [];
  return [...new Set(
    value.filter(
      (month): month is number =>
        typeof month === 'number' && Number.isInteger(month) && month >= 1 && month <= 12
    )
  )].sort((a, b) => a - b);
}

function parseZamAylariSetting(value: string | null): number[] {
  if (!value) return [];
  try {
    return normalizeZamAylari(JSON.parse(value));
  } catch {
    return [];
  }
}

function makeBackupPayload(data: DatasetFields): BackupPayload {
  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    ...data,
  };
}

function parseBackupPayload(json: string): BackupPayload {
  const parsed = parsePayrollStorage<Partial<BackupPayload> & Record<string, unknown>>(json);
  const version = typeof parsed.backupVersion === 'number' ? parsed.backupVersion : 1;
  if (version <= 0 || version > BACKUP_FORMAT_VERSION) {
    throw new Error(`Desteklenmeyen yedek sürümü: ${version}`);
  }
  if (!Array.isArray(parsed.donemler) || !Array.isArray(parsed.personeller)) {
    throw new Error('Yedek dosyasında dönem veya personel listesi bulunamadı.');
  }
  if (
    version >= 2 &&
    (!Array.isArray(parsed.puantajlar) ||
      !Array.isArray(parsed.bordrolar) ||
      !Array.isArray(parsed.taxOpenings) ||
      !Array.isArray(parsed.sickLeaveRecords) ||
      !Array.isArray(parsed.annualPayrollParameters))
  ) {
    throw new Error('V2 yedek dosyasında tüm domain kayıt listeleri bulunmalıdır.');
  }
  const periods = parsed.donemler as BordroDonemi[];
  const bordrolar = ((parsed.bordrolar as Array<Partial<BordroKaydi>> | undefined) || []).map(
    (bordro) => ({
      ...bordro,
      // V1 localStorage kayıtlarında durum alanı yoktu. Sınırda bir kez
      // normalize edilir; uygulama içindeki sözleşme Rust ile aynıdır.
      status: bordro.status || 'CALCULATED',
    })
  ) as BordroKaydi[];

  return makeBackupPayload({
    donemler: periods,
    aktifDonemId:
      typeof parsed.aktifDonemId === 'string'
        ? parsed.aktifDonemId
        : periods[0]?.id || '',
    personeller: parsed.personeller as Personel[],
    kurumDegerleriMap:
      (parsed.kurumDegerleriMap as Record<string, DönemselKurumDegerleri> | undefined) || {},
    puantajlar: (parsed.puantajlar as PersonelPuantaj[] | undefined) || [],
    bordrolar,
    taxOpenings: (parsed.taxOpenings as PersonelTaxOpening[] | undefined) || [],
    sickLeaveRecords: (parsed.sickLeaveRecords as SickLeaveRecord[] | undefined) || [],
    annualPayrollParameters:
      (parsed.annualPayrollParameters as AnnualPayrollParameters[] | undefined) || [],
    zamAylari: normalizeZamAylari(parsed.zamAylari),
  });
}

export default function App() {
  const [activeTab, setActiveTab] = useState<TabType>(() => {
    try {
      const saved = localStorage.getItem('4d_bordro_active_tab');
      if (
        saved &&
        ['personel', 'puantaj', 'bordro', 'banka', 'kesintiler'].includes(saved)
      ) {
        return saved as TabType;
      }
    } catch {
      // localStorage may be unavailable in a restricted browser context.
    }
    return 'personel';
  });

  const [donemler, setDonemler] = useState<BordroDonemi[]>([]);
  const [aktifDonemId, setAktifDonemId] = useState<string>('');
  const [personeller, setPersoneller] = useState<Personel[]>([]);
  const [kurumDegerleriMap, setKurumDegerleriMap] = useState<
    Record<string, DönemselKurumDegerleri>
  >({});
  const [puantajlar, setPuantajlar] = useState<PersonelPuantaj[]>([]);
  const [bordrolar, setBordrolar] = useState<BordroKaydi[]>([]);
  const [taxOpenings, setTaxOpenings] = useState<PersonelTaxOpening[]>([]);
  const [sickLeaveRecords, setSickLeaveRecords] = useState<SickLeaveRecord[]>([]);
  const [annualPayrollParameters, setAnnualPayrollParameters] = useState<
    AnnualPayrollParameters[]
  >([]);
  const [zamAylari, setZamAylari] = useState<number[]>([]);
  const [isDataLoaded, setIsDataLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [targetPersonelIdForBordro, setTargetPersonelIdForBordro] = useState<
    string | undefined
  >(undefined);
  const [isPeriodManagerOpen, setIsPeriodManagerOpen] = useState(false);

  useEffect(() => {
    try {
      localStorage.setItem('4d_bordro_active_tab', activeTab);
    } catch {
      // Ignore navigation preference write errors.
    }
  }, [activeTab]);

  const applyDataset = useCallback((data: DatasetFields) => {
    setDonemler(data.donemler);
    setAktifDonemId(data.aktifDonemId || data.donemler[0]?.id || '');
    setPersoneller(data.personeller);
    setKurumDegerleriMap(data.kurumDegerleriMap);
    setPuantajlar(data.puantajlar);
    setBordrolar(data.bordrolar);
    setTaxOpenings(data.taxOpenings);
    setSickLeaveRecords(data.sickLeaveRecords);
    setAnnualPayrollParameters(data.annualPayrollParameters);
    setZamAylari(normalizeZamAylari(data.zamAylari));
  }, []);

  const loadData = useCallback(async () => {
    setLoadError(null);
    const isNative = tauriBridge.isTauriAvailable();

    try {
      if (isNative) {
        const isMigrated = await tauriBridge.checkLegacyMigrated();
        if (!isMigrated) {
          const legacyStr = localStorage.getItem(STORAGE_KEY);
          if (legacyStr) {
            // Migration errors intentionally abort native loading. A failed
            // database write must never be presented as a browser-only success.
            await tauriBridge.migrateLegacyPayload(legacyStr);
          }
        }

        const [fetchedPeriods, fetchedPersonnel, fetchedAttendance, fetchedPayrolls, fetchedSettings, fetchedTaxOpenings, fetchedSickLeaves, fetchedAnnualParameters, savedActivePeriodId, savedZamAylari] =
          await Promise.all([
            tauriBridge.getPeriods(),
            tauriBridge.getPersonnelList(),
            tauriBridge.getAttendanceList(),
            tauriBridge.getPayrollList(),
            tauriBridge.getInstitutionSettings(),
            tauriBridge.getTaxOpenings(),
            tauriBridge.getSickLeaveRecords(),
            tauriBridge.getAnnualPayrollParameters(),
            tauriBridge.getAppSetting('active_period_id'),
            tauriBridge.getAppSetting(ZAM_AYLARI_SETTING_KEY),
          ]);

        applyDataset({
          donemler: fetchedPeriods,
          aktifDonemId: savedActivePeriodId || fetchedPeriods[0]?.id || '',
          personeller: fetchedPersonnel,
          kurumDegerleriMap: fetchedSettings,
          puantajlar: fetchedAttendance,
          bordrolar: fetchedPayrolls,
          taxOpenings: fetchedTaxOpenings,
          sickLeaveRecords: fetchedSickLeaves,
          annualPayrollParameters: fetchedAnnualParameters,
          zamAylari: parseZamAylariSetting(savedZamAylari),
        });
        setIsDataLoaded(true);
        return;
      }

      const saved = await browserPayrollStore.loadPayload();
      if (saved) {
        const payload = parseBackupPayload(saved);
        applyDataset(payload);
      } else {
        applyDataset({
          donemler: [],
          aktifDonemId: '',
          personeller: [],
          kurumDegerleriMap: {},
          puantajlar: [],
          bordrolar: [],
          taxOpenings: [],
          sickLeaveRecords: [],
          annualPayrollParameters: [],
          zamAylari: [],
        });
      }
      setIsDataLoaded(true);
    } catch (err) {
      const message = `Veri yüklenemedi: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
      // Native failures stop here. Browser failures also remain visible rather
      // than being replaced with an empty, apparently valid dataset.
      setIsDataLoaded(false);
    }
  }, [applyDataset]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  // Standalone browser mode has the same versioned backup contract as native
  // mode. The loaded guard prevents the initial empty state from overwriting a
  // real saved dataset before the first read completes.
  useEffect(() => {
    if (!isDataLoaded || tauriBridge.isTauriAvailable()) return;
    const payload = makeBackupPayload({
      donemler,
      aktifDonemId,
      personeller,
      kurumDegerleriMap,
      puantajlar,
      bordrolar,
      taxOpenings,
      sickLeaveRecords,
      annualPayrollParameters,
      zamAylari,
    });
    void browserPayrollStore.savePayload(serializePayrollStorage(payload)).catch((err) => {
      const message = `Tarayıcı verisi kaydedilemedi: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
    });
  }, [
    isDataLoaded,
    donemler,
    aktifDonemId,
    personeller,
    kurumDegerleriMap,
    puantajlar,
    bordrolar,
    taxOpenings,
    sickLeaveRecords,
    annualPayrollParameters,
    zamAylari,
  ]);

  const handleSelectDonem = async (id: string) => {
    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.setAppSetting('active_period_id', id);
      }
      setAktifDonemId(id);
    } catch (err) {
      const message = `Aktif dönem kaydedilemedi: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
    }
  };

  const aktifDonem = donemler.find((d) => d.id === aktifDonemId) || donemler[0];
  const payrollEngine = getPayrollEngine();
  const payrollDataset = useMemo(
    () => ({
      personnel: personeller,
      periods: donemler,
      institutionSettings: kurumDegerleriMap,
      attendances: puantajlar,
      payrolls: bordrolar,
      taxOpenings,
      sickLeaveRecords,
      annualPayrollParameters,
      zamAylari,
    }),
    [
      personeller,
      donemler,
      kurumDegerleriMap,
      puantajlar,
      bordrolar,
      taxOpenings,
      sickLeaveRecords,
      annualPayrollParameters,
      zamAylari,
    ]
  );

  /**
   * Browser mutations are authorized by the same Rust policy as native
   * mutations. This helper only merges the core response when a mutation has
   * both an old and a new source position (for example an edited period).
   */
  const evaluateBrowserMutations = async (
    mutation: PayrollMutation | PayrollMutation[]
  ): Promise<MutationImpact> => {
    const mutations = Array.isArray(mutation) ? mutation : [mutation];
    const impacts = await Promise.all(
      mutations.map((item) => payrollEngine.evaluateMutationPolicy(item, payrollDataset))
    );
    const affected = new Map<string, MutationImpact['affectedPayrolls'][number]>();
    const blocked = new Map<string, MutationImpact['blockedByFinalized'][number]>();
    for (const impact of impacts) {
      for (const key of impact.affectedPayrolls) {
        affected.set(`${key.personnelId}\u0000${key.periodId}`, key);
      }
      for (const key of impact.blockedByFinalized) {
        blocked.set(`${key.personnelId}\u0000${key.periodId}`, key);
      }
    }
    const merged = {
      affectedPayrolls: [...affected.values()],
      blockedByFinalized: [...blocked.values()],
    } satisfies MutationImpact;
    assertBrowserMutationImpactAllowed(merged);
    return merged;
  };

  const handleResetSampleData = async () => {
    if (
      !window.confirm(
        'Tüm mevcut veriler sıfırlanıp örnek 4/D bordro verileri yüklenecek. Emin misiniz?'
      )
    ) {
      return;
    }

    const initialData = getInitialDataset();
    const payload = makeBackupPayload({
      ...initialData,
      taxOpenings: initialData.taxOpenings || [],
      sickLeaveRecords: initialData.sickLeaveRecords || [],
      annualPayrollParameters: initialData.annualPayrollParameters || [],
      zamAylari: initialData.zamAylari || [],
    });

    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
        return;
      }

      await evaluateBrowserMutations({ kind: 'ALL' });
      applyDataset(payload);
      setIsDataLoaded(true);
    } catch (err) {
      const message = `Örnek veriler yüklenemedi: ${String(err)}`;
      console.error(message, err);
      setLoadError(message);
      alert(message);
    }
  };

  const handleExportBackup = () => {
    const payload = makeBackupPayload({
      donemler,
      aktifDonemId,
      personeller,
      kurumDegerleriMap,
      puantajlar,
      bordrolar,
      taxOpenings,
      sickLeaveRecords,
      annualPayrollParameters,
      zamAylari,
    });
    const jsonStr = serializePayrollStorage(payload, 2);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `4D_Bordro_Yedek_${aktifDonemId || 'bos'}_${new Date()
      .toISOString()
      .slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImportBackup = async (jsonStr: string) => {
    try {
      const payload = parseBackupPayload(jsonStr);
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.replaceBackupPayload(serializePayrollStorage(payload));
        await loadData();
      } else {
        await evaluateBrowserMutations({ kind: 'ALL' });
        applyDataset(payload);
        setIsDataLoaded(true);
      }
      alert('Yedek başarıyla yüklendi!');
    } catch (err) {
      console.error('Yedek yükleme başarısız:', err);
      alert(`Yedek yüklenemedi: ${String(err)}`);
    }
  };

  const handleSavePersonel = async (newPersonel: Personel) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.savePersonnel(newPersonel);
      setPersoneller(await tauriBridge.getPersonnelList());
      setTaxOpenings(await tauriBridge.getTaxOpenings());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON',
      personnelId: newPersonel.id,
    });
    setPersoneller((previous) => {
      const exists = previous.some((p) => p.id === newPersonel.id);
      return exists
        ? previous.map((p) => (p.id === newPersonel.id ? newPersonel : p))
        : [...previous, newPersonel];
    });
    setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact));
  };

  const handleDeletePersonel = async (personelId: string) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.deletePersonnel(personelId);
      setPersoneller(await tauriBridge.getPersonnelList());
      setBordrolar(await tauriBridge.getPayrollList());
      setPuantajlar(await tauriBridge.getAttendanceList());
      setTaxOpenings(await tauriBridge.getTaxOpenings());
      setSickLeaveRecords(await tauriBridge.getSickLeaveRecords());
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON',
      personnelId: personelId,
    });
    setPersoneller((previous) => previous.filter((p) => p.id !== personelId));
    setBordrolar((previous) =>
      applyBrowserPayrollImpact(previous, impact).filter((b) => b.personelId !== personelId)
    );
    setPuantajlar((previous) => previous.filter((p) => p.personelId !== personelId));
    setTaxOpenings((previous) => previous.filter((o) => o.personnelId !== personelId));
    setSickLeaveRecords((previous) => previous.filter((r) => r.personnelId !== personelId));
  };

  const handleCreateDonem = async (
    newDonem: BordroDonemi,
    kurumDegerleri: DönemselKurumDegerleri
  ) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.savePeriodWithSettings(newDonem, kurumDegerleri);
      setDonemler(await tauriBridge.getPeriods());
      setKurumDegerleriMap(await tauriBridge.getInstitutionSettings());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const existing = donemler.find((period) => period.id === newDonem.id);
    const positionMutations: PayrollMutation[] = [
      {
        kind: 'PERIOD_FROM_POSITION',
        startDate: newDonem.baslangicTarihi,
        taxYear: newDonem.taxYear,
        taxMonth: newDonem.taxMonth,
      },
    ];
    if (existing) {
      positionMutations.push({
        kind: 'PERIOD',
        periodId: newDonem.id,
      });
      const causalFieldsChanged =
        existing.yil !== newDonem.yil ||
        existing.ay !== newDonem.ay ||
        existing.baslangicTarihi !== newDonem.baslangicTarihi ||
        existing.bitisTarihi !== newDonem.bitisTarihi ||
        existing.taxYear !== newDonem.taxYear ||
        existing.taxMonth !== newDonem.taxMonth;
      if (causalFieldsChanged) {
        positionMutations.push({
          kind: 'PERIOD_FROM_POSITION',
          startDate: existing.baslangicTarihi,
          taxYear: existing.taxYear,
          taxMonth: existing.taxMonth,
        });
      }
    }
    const impact = await evaluateBrowserMutations(positionMutations);
    setDonemler((previous) =>
      previous.some((d) => d.id === newDonem.id)
        ? previous.map((d) => (d.id === newDonem.id ? newDonem : d))
        : [...previous, newDonem]
    );
    setKurumDegerleriMap((previous) => ({ ...previous, [newDonem.id]: kurumDegerleri }));
    setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact));
  };

  const handleSaveKurumDegerleri = async (settings: DönemselKurumDegerleri) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveInstitutionSettings(settings);
      setKurumDegerleriMap(await tauriBridge.getInstitutionSettings());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERIOD',
      periodId: settings.donemId,
    });
    setKurumDegerleriMap((previous) => ({ ...previous, [settings.donemId]: settings }));
    setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact));
  };

  const handleSavePuantaj = async (updatedPuantaj: PersonelPuantaj) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveAttendance(updatedPuantaj);
      setPuantajlar(await tauriBridge.getAttendanceList());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON_PERIOD',
      personnelId: updatedPuantaj.personelId,
      periodId: updatedPuantaj.donemId,
    });
    setPuantajlar((previous) => {
      const index = previous.findIndex((p) => p.id === updatedPuantaj.id);
      if (index < 0) return [...previous, updatedPuantaj];
      const next = [...previous];
      next[index] = updatedPuantaj;
      return next;
    });
    setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact));
  };

  const handleSaveTaxOpening = async (opening: PersonelTaxOpening) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveTaxOpening(opening);
      setTaxOpenings(await tauriBridge.getTaxOpenings());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON_TAX_YEAR',
      personnelId: opening.personnelId,
      taxYear: opening.year,
    });
    setTaxOpenings((previous) => {
      const index = previous.findIndex((item) => item.id === opening.id);
      if (index < 0) return [...previous, opening];
      const next = [...previous];
      next[index] = opening;
      return next;
    });
    setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact));
  };

  const handleSaveSickLeaveRecord = async (record: SickLeaveRecord) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveSickLeaveRecord(record);
      setSickLeaveRecords(await tauriBridge.getSickLeaveRecords());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const existing = sickLeaveRecords.find((item) => item.id === record.id);
    const mutations: PayrollMutation[] = [
      { kind: 'PERSON_FROM_DATE', personnelId: record.personnelId, effectiveFrom: record.startDate },
    ];
    if (existing) {
      mutations.push({
        kind: 'PERSON_FROM_DATE',
        personnelId: existing.personnelId,
        effectiveFrom: existing.startDate,
      });
    }
    const impact = await evaluateBrowserMutations(mutations);
    setSickLeaveRecords((previous) => {
      const index = previous.findIndex((item) => item.id === record.id);
      if (index < 0) return [...previous, record];
      const next = [...previous];
      next[index] = record;
      return next;
    });
    setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact));
  };

  const handleSaveAnnualPayrollParameters = async (parameters: AnnualPayrollParameters) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.saveAnnualPayrollParameters(parameters);
      setAnnualPayrollParameters(await tauriBridge.getAnnualPayrollParameters());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'TAX_YEAR',
      taxYear: parameters.year,
    });
    setAnnualPayrollParameters((previous) => {
      const index = previous.findIndex((item) => item.year === parameters.year);
      if (index < 0) return [...previous, parameters].sort((a, b) => a.year - b.year);
      const next = [...previous];
      next[index] = parameters;
      return next;
    });
    setBordrolar((current) => applyBrowserPayrollImpact(current, impact));
  };

  const handleSaveZamAylari = async (months: number[]) => {
    const normalized = normalizeZamAylari(months);
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.setAppSetting(ZAM_AYLARI_SETTING_KEY, JSON.stringify(normalized));
      setBordrolar(await tauriBridge.getPayrollList());
    }
    if (!tauriBridge.isTauriAvailable()) {
      const impact = await evaluateBrowserMutations({ kind: 'ALL' });
      setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact));
    }
    setZamAylari(normalized);
  };

  const handleDeleteSickLeaveRecord = async (id: string) => {
    if (tauriBridge.isTauriAvailable()) {
      await tauriBridge.deleteSickLeaveRecord(id);
      setSickLeaveRecords(await tauriBridge.getSickLeaveRecords());
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const record = sickLeaveRecords.find((item) => item.id === id);
    const impact = record
      ? await evaluateBrowserMutations({
          kind: 'PERSON_FROM_DATE',
          personnelId: record.personnelId,
          effectiveFrom: record.startDate,
        })
      : null;
    setSickLeaveRecords((previous) => previous.filter((record) => record.id !== id));
    if (record) {
      setBordrolar((previous) => applyBrowserPayrollImpact(previous, impact!));
    }
  };

  const handleSaveBordro = async (updatedBordro: BordroKaydi) => {
    if (tauriBridge.isTauriAvailable()) {
      // Re-fetch the whole ledger: recalculating an earlier payroll may have marked
      // one or more downstream CALCULATED payrolls as STALE in the same transaction.
      setBordrolar(await tauriBridge.getPayrollList());
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PAYROLL_CALCULATION',
      personnelId: updatedBordro.personelId,
      periodId: updatedBordro.donemId,
    });
    setBordrolar((previous) => {
      const invalidated = applyBrowserPayrollImpact(previous, impact);
      const index = invalidated.findIndex((b) => b.id === updatedBordro.id);
      if (index < 0) return [...invalidated, updatedBordro];
      const next = [...invalidated];
      next[index] = updatedBordro;
      return next;
    });
  };

  const handleSelectPersonelForBordro = (personelId: string) => {
    setTargetPersonelIdForBordro(personelId);
    setActiveTab('bordro');
  };

  return (
    <div className="min-h-screen bg-slate-100 text-slate-900 flex flex-col font-sans">
      <PayrollNoticeCenter
        enabled={isDataLoaded}
        periodId={aktifDonem?.id}
        engine={payrollEngine}
        dataset={payrollDataset}
      />
      <Navbar
        activeTab={activeTab}
        onTabChange={(tab) => {
          if (tab === 'parametrelar') setIsPeriodManagerOpen(true);
          else setActiveTab(tab);
        }}
        donemler={donemler}
        aktifDonemId={aktifDonemId}
        onSelectDonem={handleSelectDonem}
        onOpenPeriodManager={() => setIsPeriodManagerOpen(true)}
        onExportBackup={handleExportBackup}
        onImportBackup={handleImportBackup}
        onResetSampleData={handleResetSampleData}
      />

      <main className="max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-6 flex-1">
        {loadError && (
          <div className="mb-5 p-4 bg-rose-50 border border-rose-300 text-rose-900 rounded-xl text-xs font-semibold">
            {loadError}{' '}
            {tauriBridge.isTauriAvailable()
              ? 'Native veritabanı yüklenemedi; ekrandaki veriler değiştirilmedi.'
              : 'Tarayıcı verisi yüklenemedi; ekrandaki veriler değiştirilmedi.'}
          </div>
        )}

        {activeTab === 'personel' && (
          <PersonelList
            personeller={personeller}
            onSavePersonel={handleSavePersonel}
            onDeletePersonel={handleDeletePersonel}
            onSelectPersonelForBordro={handleSelectPersonelForBordro}
            isPrimiGruplari={aktifDonemId ? kurumDegerleriMap[aktifDonemId]?.isPrimiGruplari : undefined}
          />
        )}

        {!aktifDonem && activeTab !== 'personel' && (
          <div className="bg-white rounded-2xl p-8 border border-slate-200 shadow-sm text-center max-w-xl mx-auto my-12 space-y-4">
            <div className="w-12 h-12 bg-indigo-50 text-indigo-600 rounded-2xl flex items-center justify-center mx-auto text-xl font-bold">!</div>
            <h3 className="text-lg font-bold text-slate-800">Henüz Dönem Bulunmamaktadır</h3>
            <p className="text-xs text-slate-600 leading-relaxed">
              İşlemlere başlamak için yeni bir dönem tanımlayabilir veya örnek verileri yükleyebilirsiniz.
            </p>
            <div className="flex items-center justify-center gap-3 pt-2">
              <button type="button" onClick={() => setIsPeriodManagerOpen(true)} className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors">Yeni Dönem Aç</button>
              <button type="button" onClick={handleResetSampleData} className="px-4 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition-colors">Örnek Verileri Yükle</button>
            </div>
          </div>
        )}

        {aktifDonem && activeTab === 'puantaj' && (
          <PuantajGrid
            aktifDonem={aktifDonem}
            personeller={personeller}
            puantajlar={puantajlar}
            onSavePuantaj={handleSavePuantaj}
            onSelectPersonelForBordro={handleSelectPersonelForBordro}
          />
        )}

        {aktifDonem && activeTab === 'bordro' && (
          <BordroHesaplama
            aktifDonem={aktifDonem}
            donemler={donemler}
            personeller={personeller}
            kurumDegerleriMap={kurumDegerleriMap}
            puantajlar={puantajlar}
            bordrolar={bordrolar}
            taxOpenings={taxOpenings}
            sickLeaveRecords={sickLeaveRecords}
            annualPayrollParameters={annualPayrollParameters}
            zamAylari={zamAylari}
            onSaveBordro={handleSaveBordro}
            onSavePersonel={handleSavePersonel}
            onSaveTaxOpening={handleSaveTaxOpening}
            initialPersonelId={targetPersonelIdForBordro}
            onGoToPuantaj={(personelId) => {
              if (personelId) setTargetPersonelIdForBordro(personelId);
              setActiveTab('puantaj');
            }}
          />
        )}

        {aktifDonem && activeTab === 'banka' && <BankaListesi aktifDonem={aktifDonem} personeller={personeller} bordrolar={bordrolar} />}
        {aktifDonem && activeTab === 'kesintiler' && <KesintiListesi aktifDonem={aktifDonem} personeller={personeller} bordrolar={bordrolar} />}
      </main>

      <PeriodManagerModal
        isOpen={isPeriodManagerOpen}
        onClose={() => setIsPeriodManagerOpen(false)}
        donemler={donemler}
        aktifDonemId={aktifDonemId}
        onSelectDonem={handleSelectDonem}
        onCreateDonem={handleCreateDonem}
        kurumDegerleriMap={kurumDegerleriMap}
        onSaveKurumDegerleri={handleSaveKurumDegerleri}
        personeller={personeller}
        annualPayrollParameters={annualPayrollParameters}
        onSaveAnnualPayrollParameters={handleSaveAnnualPayrollParameters}
        zamAylari={zamAylari}
        onSaveZamAylari={handleSaveZamAylari}
        sickLeaveRecords={sickLeaveRecords}
        onSaveSickLeaveRecord={handleSaveSickLeaveRecord}
        onDeleteSickLeaveRecord={handleDeleteSickLeaveRecord}
      />
    </div>
  );
}
