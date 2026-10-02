import React, { useEffect, useState } from 'react';
import { AlertCircle, CheckCircle2 } from 'lucide-react';
import type {
  AnnualPayrollParameters,
  BordroDonemi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  SickLeaveRecord,
  TaxBracket,
  TediyeKalemi,
} from '../../types/payroll';
import type { ParametreSection } from '../../types/navigation';
import {
  AY_ISIMLERI,
  createBordroDonemi,
  DEFAULT_PRODUCTION_KURUM_DEGERLERI,
  withVisiblePeriodLegalDefaults,
} from '../../utils/payrollPresentation';
import {
  findSickLeaveConflicts,
  type SickLeaveConflict,
} from '../../utils/sickLeaveSync';
import {
  getDefaultAnnualPayrollParameters,
} from '../../services/storage/payrollDefaults';
import { AnnualTaxSection } from './AnnualTaxSection';
import { DeductionLegalRatesSection } from './DeductionLegalRatesSection';
import { IncomeParametersSection } from './IncomeParametersSection';
import { NewPeriodSection } from './NewPeriodSection';
import { PeriodListSection } from './PeriodListSection';
import { SickLeaveConflictModal } from './SickLeaveConflictModal';
import { SickLeaveSection } from './SickLeaveSection';
import { TediyeTisSection } from './TediyeTisSection';
import { formatPeriodSettingsSaveError, hasValidInitialWorkBonusGroups, isPositiveDailyBaseWage, savePeriodSettings } from './periodSettingsSave';
import { describeError } from '../../utils/errorMessage';

export interface PeriodSettingsPageProps {
  activeSection: ParametreSection;
  onSectionChange: (section: ParametreSection) => void;
  donemler: BordroDonemi[];
  aktifDonem?: BordroDonemi;
  aktifDonemId: string;
  onSelectDonem: (donemId: string) => Promise<void> | void;
  onCreateDonem: (
    donem: BordroDonemi,
    kurumDegerleri: DönemselKurumDegerleri
  ) => Promise<void> | void;
  kurumDegerleriMap: Record<string, DönemselKurumDegerleri>;
  onSaveKurumDegerleri: (kurumDegerleri: DönemselKurumDegerleri) => Promise<void> | void;
  personeller: Personel[];
  annualPayrollParameters: AnnualPayrollParameters[];
  onSaveAnnualPayrollParameters: (parameters: AnnualPayrollParameters) => Promise<void> | void;
  puantajlar?: PersonelPuantaj[];
  sickLeaveRecords: SickLeaveRecord[];
  onSaveSickLeaveRecord: (record: SickLeaveRecord) => Promise<void> | void;
  onDeleteSickLeaveRecord: (id: string) => Promise<void> | void;
  zamAylari: number[];
  onSaveZamAylari: (months: number[]) => Promise<void> | void;
}

const sanitizeTediyeList = (list?: TediyeKalemi[]) =>
  (list ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.tediyeListesi).map((item) => ({
    ...item,
    ad: item.ad.replace(/\s*\(\d+\s*gün\)/i, ''),
  }));

const createParamsForm = (
  settings: DönemselKurumDegerleri,
  donemId: string
): DönemselKurumDegerleri => ({
  ...withVisiblePeriodLegalDefaults(settings, donemId),
  isPrimiGruplari: settings.isPrimiGruplari ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.isPrimiGruplari,
  tediyeListesi: sanitizeTediyeList(settings.tediyeListesi),
  tisIkramiyeListesi:
    settings.tisIkramiyeListesi ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.tisIkramiyeListesi,
});

interface MissingPeriodSectionProps {
  testId: string;
  title: string;
  onOpenNewPeriod: () => void;
}

const MissingPeriodSection: React.FC<MissingPeriodSectionProps> = ({
  testId,
  title,
  onOpenNewPeriod,
}) => (
  <section data-testid={testId} className="space-y-5">
    <header>
      <h2 className="text-xl font-bold text-slate-900">{title}</h2>
      <p className="mt-1 text-xs text-slate-500">
        Bu bölüm aktif bir bordro dönemine bağlı çalışır.
      </p>
    </header>
    <div className="rounded-2xl border border-dashed border-slate-300 bg-slate-50 p-8 text-center">
      <h3 className="text-sm font-bold text-slate-800">Aktif dönem bulunmuyor</h3>
      <p className="mx-auto mt-2 max-w-md text-xs leading-relaxed text-slate-600">
        Bu bölümü kullanabilmek için önce bir bordro dönemi oluşturun. Mevcut formlarınız yeni dönem açılana kadar değiştirilmez.
      </p>
      <button
        type="button"
        onClick={onOpenNewPeriod}
        className="mt-4 rounded-xl bg-indigo-600 px-4 py-2 text-xs font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700"
      >
        Yeni Dönem Aç
      </button>
    </div>
  </section>
);

export const PeriodSettingsPage: React.FC<PeriodSettingsPageProps> = ({
  activeSection,
  onSectionChange,
  donemler,
  aktifDonem,
  aktifDonemId,
  onSelectDonem,
  onCreateDonem,
  kurumDegerleriMap,
  onSaveKurumDegerleri,
  personeller,
  annualPayrollParameters,
  onSaveAnnualPayrollParameters,
  puantajlar,
  sickLeaveRecords,
  onSaveSickLeaveRecord,
  onDeleteSickLeaveRecord,
  zamAylari,
  onSaveZamAylari,
}) => {
  const currentYear = new Date().getFullYear();
  const configuredYears = [
    ...donemler.flatMap((period) => [period.yil, period.taxYear]),
    ...annualPayrollParameters.map((parameters) => parameters.year),
  ].filter((year): year is number => Number.isInteger(year));
  const firstSelectableYear = Math.min(currentYear - 5, ...configuredYears, currentYear);
  const lastSelectableYear = Math.max(currentYear + 40, ...configuredYears, currentYear);
  const yearOptions = Array.from(
    { length: lastSelectableYear - firstSelectableYear + 1 },
    (_, index) => firstSelectableYear + index
  );

  const [newYear, setNewYear] = useState<number>(currentYear);
  const [newMonth, setNewMonth] = useState<number>(1);
  const [newTaxYear, setNewTaxYear] = useState<number>(currentYear);
  const [newTaxMonth, setNewTaxMonth] = useState<number>(2);
  const [periodGlobalSuccess, setPeriodGlobalSuccess] = useState<string | null>(null);
  const [periodGlobalError, setPeriodGlobalError] = useState<string | null>(null);
  const [isSubmittingPeriod, setIsSubmittingPeriod] = useState(false);
  const [initialDailyBaseWage, setInitialDailyBaseWage] = useState(() => {
    const latestPeriod = [...donemler].sort((left, right) => right.baslangicTarihi.localeCompare(left.baslangicTarihi))[0];
    const inherited = kurumDegerleriMap[aktifDonemId]?.gunlukTabanUcret ??
      (latestPeriod ? kurumDegerleriMap[latestPeriod.id]?.gunlukTabanUcret : undefined);
    return inherited && inherited > 0 ? String(inherited) : '';
  });

  const resetTaxDefaults = (year: number, month: number) => {
    setPeriodGlobalError(null);
    const taxMonth = month === 12 ? 1 : month + 1;
    const taxYear = month === 12 ? year + 1 : year;
    setNewTaxMonth(taxMonth);
    setNewTaxYear(taxYear);
  };

  const handleYearChange = (year: number) => {
    setPeriodGlobalError(null);
    setNewYear(year);
    resetTaxDefaults(year, newMonth);
  };

  const handleMonthChange = (month: number) => {
    setPeriodGlobalError(null);
    setNewMonth(month);
    resetTaxDefaults(newYear, month);
  };

  const handleTaxYearChange = (year: number) => {
    setPeriodGlobalError(null);
    setNewTaxYear(year);
  };

  const handleTaxMonthChange = (month: number) => {
    setPeriodGlobalError(null);
    setNewTaxMonth(month);
  };

  const activeKurumDegerleri =
    kurumDegerleriMap[aktifDonemId] || {
      donemId: aktifDonemId,
      ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
    };
  const [paramsForm, setParamsForm] = useState<DönemselKurumDegerleri>(
    createParamsForm(activeKurumDegerleri, aktifDonemId)
  );
  const [savedSuccess, setSavedSuccess] = useState(false);
  const [paramsSaveError, setParamsSaveError] = useState<string | null>(null);
  const [annualTaxYear, setAnnualTaxYear] = useState<number>(currentYear);
  const initialAnnualDefaults = getDefaultAnnualPayrollParameters(currentYear);
  const [annualTaxBrackets, setAnnualTaxBrackets] = useState<TaxBracket[]>(
    initialAnnualDefaults?.gelirVergisiDilimleri.map((bracket) => ({ ...bracket })) || []
  );
  const [annualInsuranceGvCap, setAnnualInsuranceGvCap] = useState<number>(
    initialAnnualDefaults?.sigortaGvYillikBrutAsgariUcretTavani ?? 0
  );
  const [annualTaxSuccess, setAnnualTaxSuccess] = useState(false);
  const [zamAylariForm, setZamAylariForm] = useState<number[]>(
    [...zamAylari].sort((a, b) => a - b)
  );
  const [selectedPersonForSick, setSelectedPersonForSick] = useState(
    personeller[0]?.id || ''
  );
  const [sickStartDate, setSickStartDate] = useState('');
  const [sickEndDate, setSickEndDate] = useState('');
  const [sickSuccessMsg, setSickSuccessMsg] = useState<string | null>(null);

  useEffect(() => {
    if (personeller.length > 0 && !selectedPersonForSick) {
      setSelectedPersonForSick(personeller[0].id);
    }
  }, [personeller, selectedPersonForSick]);

  useEffect(() => {
    const active = kurumDegerleriMap[aktifDonemId] || {
      donemId: aktifDonemId,
      ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
    };
    setParamsForm(createParamsForm(active, aktifDonemId));
  }, [aktifDonemId, kurumDegerleriMap]);

  useEffect(() => {
    if (initialDailyBaseWage !== '') return;
    const latestPeriod = [...donemler].sort((left, right) => right.baslangicTarihi.localeCompare(left.baslangicTarihi))[0];
    const inherited = kurumDegerleriMap[aktifDonemId]?.gunlukTabanUcret ??
      (latestPeriod ? kurumDegerleriMap[latestPeriod.id]?.gunlukTabanUcret : undefined);
    if (inherited && inherited > 0) setInitialDailyBaseWage(String(inherited));
  }, [aktifDonemId, donemler, kurumDegerleriMap, initialDailyBaseWage]);

  const activePeriodForTaxYear = donemler.find((period) => period.id === aktifDonemId);
  const activeTaxYear = activePeriodForTaxYear?.taxYear || newTaxYear;

  useEffect(() => {
    setAnnualTaxYear(activeTaxYear);
  }, [aktifDonemId, activeTaxYear]);

  useEffect(() => {
    // Keep a manually selected tariff year and its currently edited brackets
    // when a save refreshes the annual parameter list. Only hydrate the form
    // from persisted data while it is showing the active period's year.
    if (annualTaxYear !== activeTaxYear) return;

    const savedParameters = annualPayrollParameters.find(
      (parameters) => parameters.year === activeTaxYear
    );
    setAnnualTaxBrackets(
      savedParameters?.gelirVergisiDilimleri.map((bracket) => ({ ...bracket })) ||
        getDefaultAnnualPayrollParameters(activeTaxYear)?.gelirVergisiDilimleri.map((bracket) => ({ ...bracket })) ||
        []
    );
    setAnnualInsuranceGvCap(
      savedParameters?.sigortaGvYillikBrutAsgariUcretTavani ??
        getDefaultAnnualPayrollParameters(activeTaxYear)?.sigortaGvYillikBrutAsgariUcretTavani ??
        0
    );
  }, [annualPayrollParameters, annualTaxYear, activeTaxYear]);

  useEffect(() => {
    setZamAylariForm([...zamAylari].sort((a, b) => a - b));
  }, [zamAylari]);

  const activePeriodForParams =
    donemler.find((period) => period.id === aktifDonemId) || aktifDonem;
  const previewDonem = createBordroDonemi(newYear, newMonth, newTaxYear, newTaxMonth);
  const existingPreview = donemler.find((period) => period.id === previewDonem.id);
  const previewExists = existingPreview !== undefined;
  const previewTaxChanged =
    previewExists &&
    (existingPreview.taxYear !== previewDonem.taxYear ||
      existingPreview.taxMonth !== previewDonem.taxMonth);

  const [editingSickRecord, setEditingSickRecord] = useState<SickLeaveRecord | null>(null);
  const [pendingSickLeave, setPendingSickLeave] = useState<SickLeaveRecord | null>(null);
  const [pendingConflicts, setPendingConflicts] = useState<SickLeaveConflict[]>([]);
  const [isConflictModalOpen, setIsConflictModalOpen] = useState(false);
  const [isSubmittingSickLeave, setIsSubmittingSickLeave] = useState(false);
  const [sickLeaveError, setSickLeaveError] = useState<string | null>(null);

  const handleStartEditSickLeave = (record: SickLeaveRecord) => {
    setEditingSickRecord(record);
    setSelectedPersonForSick(record.personnelId);
    setSickStartDate(record.startDate);
    setSickEndDate(record.endDate);
  };

  const handleCancelEditSickLeave = () => {
    setEditingSickRecord(null);
    setSickStartDate('');
    setSickEndDate('');
  };

  const executeSaveSickLeave = async (targetRecord: SickLeaveRecord) => {
    setIsSubmittingSickLeave(true);
    setSickLeaveError(null);
    try {
      await onSaveSickLeaveRecord(targetRecord);
      setSickSuccessMsg(
        editingSickRecord
          ? 'Rapor olayı başarıyla güncellendi.'
          : 'Rapor olayı başarıyla kaydedildi.'
      );
      setTimeout(() => setSickSuccessMsg(null), 2500);
      setSickStartDate('');
      setSickEndDate('');
      setEditingSickRecord(null);
      setIsConflictModalOpen(false);
      setPendingSickLeave(null);
      setPendingConflicts([]);
    } catch (error) {
      const reason =
        error instanceof Error
          ? error.message
          : describeError(error);
      setSickLeaveError(`Rapor kaydedilemedi: ${reason || 'Beklenmeyen bir hata oluştu.'}`);
    } finally {
      setIsSubmittingSickLeave(false);
    }
  };

  const handleAddSickLeave = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedPersonForSick || !sickStartDate || !sickEndDate) return;

    const targetRecord: SickLeaveRecord = {
      id: editingSickRecord ? editingSickRecord.id : `sick_${selectedPersonForSick}_${Date.now()}`,
      personnelId: selectedPersonForSick,
      startDate: sickStartDate,
      endDate: sickEndDate,
    };

    const conflicts = findSickLeaveConflicts({
      personnelId: selectedPersonForSick,
      startDate: sickStartDate,
      endDate: sickEndDate,
      donemler,
      puantajlar: puantajlar ?? [],
    });

    if (conflicts.length > 0) {
      setPendingSickLeave(targetRecord);
      setPendingConflicts(conflicts);
      setIsConflictModalOpen(true);
      return;
    }

    await executeSaveSickLeave(targetRecord);
  };

  const handleApplyConflict = async () => {
    if (!pendingSickLeave) return;
    await executeSaveSickLeave(pendingSickLeave);
  };

  const handleCancelConflict = () => {
    setIsConflictModalOpen(false);
    setPendingSickLeave(null);
    setPendingConflicts([]);
    setSickLeaveError(null);
  };

  const handleDeleteSickLeave = async (id: string) => {
    if (editingSickRecord?.id === id) {
      handleCancelEditSickLeave();
    }
    try {
      await onDeleteSickLeaveRecord(id);
    } catch (error) {
      alert(`Rapor olayı silinemedi: ${describeError(error)}`);
    }
  };

  const handleCreateNewPeriod = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (isSubmittingPeriod) return;

    setPeriodGlobalError(null);
    setPeriodGlobalSuccess(null);
    setIsSubmittingPeriod(true);

    const newDonem = createBordroDonemi(newYear, newMonth, newTaxYear, newTaxMonth);

    try {
      if (previewExists) {
        // Mevcut dönemi koru: tahakkuk veya parametreleri asla ezme, sadece seç ve geç
        await onSelectDonem(newDonem.id);
        setPeriodGlobalSuccess(`${newDonem.id} dönemi seçildi.`);
        setTimeout(() => setPeriodGlobalSuccess(null), 4000);
        onSectionChange('gelir');
        return;
      }

      if (!isPositiveDailyBaseWage(initialDailyBaseWage)) {
        throw new Error('Dönemi oluşturmak için sıfırdan büyük Günlük Taban Ücret girin.');
      }
      const workBonusGroups = paramsForm.isPrimiGruplari ?? [];
      if (!hasValidInitialWorkBonusGroups(workBonusGroups)) {
        throw new Error('Dönemi oluşturmak için kurumunuzun İş Primi grup adlarını ve 0–100 arasındaki oranlarını girin.');
      }

      const initialKurum: DönemselKurumDegerleri = withVisiblePeriodLegalDefaults(
        {
          ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
          ...paramsForm,
          donemId: newDonem.id,
          gunlukTabanUcret:
            Number(initialDailyBaseWage),
          gunlukYemek: paramsForm.gunlukYemek ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.gunlukYemek,
          birlestirilmisSosyalYardim:
            paramsForm.birlestirilmisSosyalYardim ??
            DEFAULT_PRODUCTION_KURUM_DEGERLERI.birlestirilmisSosyalYardim,
          gunlukVasitaYol:
            paramsForm.gunlukVasitaYol ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.gunlukVasitaYol,
          giyimYardimi: paramsForm.giyimYardimi ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.giyimYardimi,
          hizmetZammiBirimi:
            paramsForm.hizmetZammiBirimi ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.hizmetZammiBirimi,
          isPrimiYuzde: paramsForm.isPrimiYuzde || 0,
          isPrimiGruplari: workBonusGroups,
          ekOdeme: paramsForm.ekOdeme || 0,
          tediyeListesi: sanitizeTediyeList(paramsForm.tediyeListesi),
          tisIkramiyeListesi:
            paramsForm.tisIkramiyeListesi ?? DEFAULT_PRODUCTION_KURUM_DEGERLERI.tisIkramiyeListesi,
          tediyeTisNotu: paramsForm.tediyeTisNotu || DEFAULT_PRODUCTION_KURUM_DEGERLERI.tediyeTisNotu,
          statutoryParameterSegments: paramsForm.statutoryParameterSegments ?? [],
          statutoryParameterSnapshot: undefined,
        },
        newDonem.id
      );

      await onCreateDonem(newDonem, initialKurum);
      await onSelectDonem(newDonem.id);
      setPeriodGlobalSuccess(`${newDonem.id} dönemi başarıyla oluşturuldu ve seçildi.`);
      setTimeout(() => setPeriodGlobalSuccess(null), 4000);
      onSectionChange('gelir');
    } catch (error) {
      setPeriodGlobalError(formatPeriodSettingsSaveError(error, 'Dönem işlemi'));
    } finally {
      setIsSubmittingPeriod(false);
    }
  };

  const handleSaveParams = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setParamsSaveError(null);
    setSavedSuccess(false);
    const outcome = await savePeriodSettings(
      { ...paramsForm, donemId: aktifDonemId },
      zamAylariForm,
      createParamsForm(activeKurumDegerleri, aktifDonemId),
      zamAylari,
      onSaveKurumDegerleri,
      onSaveZamAylari
    );
    if (outcome.kind === 'success') {
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 2500);
    } else {
      setParamsForm(outcome.paramsForm);
      setZamAylariForm(outcome.zamAylariForm);
      setParamsSaveError(outcome.message);
    }
  };

  const handleSaveAnnualTaxParameters = async (
    event: React.FormEvent<HTMLFormElement>
  ) => {
    event.preventDefault();
    if (!Number.isInteger(annualTaxYear) || annualTaxYear <= 0 || annualTaxBrackets.length === 0) {
      alert('Vergi yılı ve en az bir gelir vergisi dilimi girilmelidir.');
      return;
    }

    if (!Number.isFinite(annualInsuranceGvCap) || annualInsuranceGvCap <= 0) {
      alert('Sigorta GV yıllık brüt asgari ücret tavanı sıfırdan büyük olmalıdır.');
      return;
    }

    let previousLimit = 0;
    for (const bracket of annualTaxBrackets) {
      if (bracket.limit <= previousLimit || bracket.oran < 0 || bracket.oran > 1) {
        alert('Vergi dilimleri artan limitlere ve %0-%100 arasında oranlara sahip olmalıdır.');
        return;
      }
      previousLimit = bracket.limit;
    }

    try {
      await onSaveAnnualPayrollParameters({
        year: annualTaxYear,
        gelirVergisiDilimleri: annualTaxBrackets,
        sigortaGvYillikBrutAsgariUcretTavani: annualInsuranceGvCap,
      });
      setAnnualTaxSuccess(true);
      setTimeout(() => setAnnualTaxSuccess(false), 2500);
    } catch (error) {
      alert(`Yıllık vergi parametreleri kaydedilemedi: ${describeError(error)}`);
    }
  };

  const openNewPeriod = () => onSectionChange('newPeriod');

  const renderActiveSection = () => {
    if (activeSection === 'gelir') {
      return aktifDonem ? (
        <IncomeParametersSection
          aktifDonemId={aktifDonem.id}
          paramsForm={paramsForm}
          setParamsForm={setParamsForm}
          zamAylariForm={zamAylariForm}
          setZamAylariForm={setZamAylariForm}
          savedSuccess={savedSuccess}
          errorMessage={paramsSaveError}
          onSubmit={handleSaveParams}
        />
      ) : (
        <MissingPeriodSection
          testId="period-settings-gelir"
          title="Ücretler"
          onOpenNewPeriod={openNewPeriod}
        />
      );
    }

    if (activeSection === 'kesinti') {
      return aktifDonem ? (
        <DeductionLegalRatesSection
          aktifDonemId={aktifDonem.id}
          activePeriodForParams={activePeriodForParams}
          paramsForm={paramsForm}
          setParamsForm={setParamsForm}
          savedSuccess={savedSuccess}
          onSubmit={handleSaveParams}
        />
      ) : (
        <MissingPeriodSection
          testId="period-settings-kesinti"
          title="Vergi & Yasal Oranlar"
          onOpenNewPeriod={openNewPeriod}
        />
      );
    }

    if (activeSection === 'annualTax') {
      return (
        <AnnualTaxSection
          annualTaxYear={annualTaxYear}
          setAnnualTaxYear={setAnnualTaxYear}
          annualTaxBrackets={annualTaxBrackets}
          setAnnualTaxBrackets={setAnnualTaxBrackets}
          annualInsuranceGvCap={annualInsuranceGvCap}
          setAnnualInsuranceGvCap={setAnnualInsuranceGvCap}
          annualTaxSuccess={annualTaxSuccess}
          onSubmit={handleSaveAnnualTaxParameters}
        />
      );
    }

    if (activeSection === 'tediyeTis') {
      return aktifDonem ? (
        <TediyeTisSection
          paramsForm={paramsForm}
          setParamsForm={setParamsForm}
          savedSuccess={savedSuccess}
          onSubmit={handleSaveParams}
        />
      ) : (
        <MissingPeriodSection
          testId="period-settings-tediye-tis"
          title="TİS / Tediye Takvimi"
          onOpenNewPeriod={openNewPeriod}
        />
      );
    }

    if (activeSection === 'sickLeave') {
      return (
        <>
          {sickLeaveError && (
            <div
              role="alert"
              data-testid="sick-leave-error-banner"
              className="mb-4 p-3.5 bg-rose-50 text-rose-900 border border-rose-300 rounded-xl text-xs font-semibold flex items-center gap-2.5"
            >
              <AlertCircle className="w-5 h-5 text-rose-600 shrink-0" />
              <span>{sickLeaveError}</span>
            </div>
          )}
          <SickLeaveSection
            personeller={personeller}
            sickLeaveRecords={sickLeaveRecords}
            selectedPersonForSick={selectedPersonForSick}
            setSelectedPersonForSick={setSelectedPersonForSick}
            sickStartDate={sickStartDate}
            setSickStartDate={setSickStartDate}
            sickEndDate={sickEndDate}
            setSickEndDate={setSickEndDate}
            sickSuccessMsg={sickSuccessMsg}
            onAddSickLeave={handleAddSickLeave}
            onDeleteSickLeave={handleDeleteSickLeave}
            editingSickRecord={editingSickRecord}
            onStartEditSickLeave={handleStartEditSickLeave}
            onCancelEditSickLeave={handleCancelEditSickLeave}
          />
          <SickLeaveConflictModal
            isOpen={isConflictModalOpen}
            conflicts={pendingConflicts}
            onApply={handleApplyConflict}
            onCancel={handleCancelConflict}
            isSubmitting={isSubmittingSickLeave}
          />
        </>
      );
    }

    if (activeSection === 'donemler') {
      return (
        <PeriodListSection
          donemler={donemler}
          aktifDonemId={aktifDonemId}
          onSelectDonem={onSelectDonem}
          onOpenNewPeriod={openNewPeriod}
        />
      );
    }

    return (
      <NewPeriodSection
        newYear={newYear}
        setNewYear={handleYearChange}
        newMonth={newMonth}
        setNewMonth={handleMonthChange}
        newTaxYear={newTaxYear}
        setNewTaxYear={handleTaxYearChange}
        newTaxMonth={newTaxMonth}
        setNewTaxMonth={handleTaxMonthChange}
        yearOptions={yearOptions}
        resetTaxDefaults={resetTaxDefaults}
        previewDonem={previewDonem}
        previewExists={previewExists}
        previewTaxChanged={previewTaxChanged}
        onSubmit={handleCreateNewPeriod}
        isSubmitting={isSubmittingPeriod}
        dailyBaseWage={initialDailyBaseWage}
        onDailyBaseWageChange={(value) => {
          setInitialDailyBaseWage(value);
          setPeriodGlobalError(null);
        }}
        workBonusGroups={paramsForm.isPrimiGruplari ?? []}
        onWorkBonusGroupsChange={(groups) => {
          setParamsForm((current) => ({ ...current, isPrimiGruplari: groups }));
          setPeriodGlobalError(null);
        }}
        errorMessage={periodGlobalError}
        successMessage={periodGlobalSuccess}
      />
    );
  };

  return (
    <section data-testid="period-settings-page" className="space-y-6">
      <header className="border-b border-slate-200 pb-5">
        <h1 className="text-2xl font-extrabold tracking-tight text-slate-900">Dönem Parametreleri</h1>
        <p className="mt-1 max-w-3xl text-sm leading-relaxed text-slate-600">
          Bordro dönemlerini, dönemsel kurum değerlerini ve yasal hesaplama parametrelerini yönetin.
          Bölümler arasında geçiş yapmak için sol menüyü kullanın.
        </p>
      </header>
      {periodGlobalSuccess && (
        <div
          role="status"
          data-testid="period-settings-success-banner"
          className="p-3.5 bg-emerald-50 text-emerald-900 border border-emerald-300 rounded-xl text-xs font-semibold flex items-center gap-2.5 animate-in fade-in"
        >
          <CheckCircle2 className="w-5 h-5 text-emerald-600 shrink-0" />
          <span>{periodGlobalSuccess}</span>
        </div>
      )}
      {periodGlobalError && activeSection !== 'newPeriod' && (
        <div
          role="alert"
          data-testid="period-settings-error-banner"
          className="p-3.5 bg-rose-50 text-rose-900 border border-rose-300 rounded-xl text-xs font-semibold flex items-center gap-2.5 animate-in fade-in"
        >
          <AlertCircle className="w-5 h-5 text-rose-600 shrink-0" />
          <span>{periodGlobalError}</span>
        </div>
      )}
      {renderActiveSection()}
    </section>
  );
};
