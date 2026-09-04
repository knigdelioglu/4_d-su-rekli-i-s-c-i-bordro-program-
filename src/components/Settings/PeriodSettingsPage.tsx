import React, { useEffect, useState } from 'react';
import type {
  AnnualPayrollParameters,
  BordroDonemi,
  DönemselKurumDegerleri,
  Personel,
  SickLeaveRecord,
  TaxBracket,
  TediyeKalemi,
} from '../../types/payroll';
import type { ParametreSection } from '../../types/navigation';
import {
  AY_ISIMLERI,
  createBordroDonemi,
  DEFAULT_IS_PRIMI_GRUPLARI,
  DEFAULT_KURUM_DEGERLERI,
  DEFAULT_TEDIYE_LISTESI,
  DEFAULT_TIS_IKRAMIYE_LISTESI,
} from '../../utils/payrollPresentation';
import { AnnualTaxSection } from './AnnualTaxSection';
import { DeductionLegalRatesSection } from './DeductionLegalRatesSection';
import { IncomeParametersSection } from './IncomeParametersSection';
import { NewPeriodSection } from './NewPeriodSection';
import { PeriodListSection } from './PeriodListSection';
import { SickLeaveSection } from './SickLeaveSection';
import { TediyeTisSection } from './TediyeTisSection';

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
  sickLeaveRecords: SickLeaveRecord[];
  onSaveSickLeaveRecord: (record: SickLeaveRecord) => Promise<void> | void;
  onDeleteSickLeaveRecord: (id: string) => Promise<void> | void;
  zamAylari: number[];
  onSaveZamAylari: (months: number[]) => Promise<void> | void;
}

const sanitizeTediyeList = (list?: TediyeKalemi[]) =>
  (list || DEFAULT_TEDIYE_LISTESI).map((item) => ({
    ...item,
    ad: item.ad.replace(/\s*\(\d+\s*gün\)/i, ''),
  }));

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

  const resetTaxDefaults = (year: number, month: number) => {
    const taxMonth = month === 12 ? 1 : month + 1;
    const taxYear = month === 12 ? year + 1 : year;
    setNewTaxMonth(taxMonth);
    setNewTaxYear(taxYear);
  };

  const activeKurumDegerleri =
    kurumDegerleriMap[aktifDonemId] || {
      donemId: aktifDonemId,
      ...DEFAULT_KURUM_DEGERLERI,
    };
  const [paramsForm, setParamsForm] = useState<DönemselKurumDegerleri>({
    ...activeKurumDegerleri,
    isPrimiGruplari: activeKurumDegerleri.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI,
    tediyeListesi: sanitizeTediyeList(activeKurumDegerleri.tediyeListesi),
    tisIkramiyeListesi:
      activeKurumDegerleri.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
    gunlukYemekIstisnasiGV:
      activeKurumDegerleri.gunlukYemekIstisnasiGV ??
      activeKurumDegerleri.gunlukYemekIstisnasiSGK ??
      DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiGV,
    statutoryParameterSegments: activeKurumDegerleri.statutoryParameterSegments || [],
  });
  const [savedSuccess, setSavedSuccess] = useState(false);
  const [annualTaxYear, setAnnualTaxYear] = useState<number>(currentYear);
  const [annualTaxBrackets, setAnnualTaxBrackets] = useState<TaxBracket[]>([]);
  const [annualInsuranceGvCap, setAnnualInsuranceGvCap] = useState<number>(396360);
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
      ...DEFAULT_KURUM_DEGERLERI,
    };
    setParamsForm({
      ...active,
      isPrimiGruplari: active.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI,
      tediyeListesi: sanitizeTediyeList(active.tediyeListesi),
      tisIkramiyeListesi: active.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
      gunlukYemekIstisnasiGV:
        active.gunlukYemekIstisnasiGV ??
        active.gunlukYemekIstisnasiSGK ??
        DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiGV,
      statutoryParameterSegments: active.statutoryParameterSegments || [],
    });
  }, [aktifDonemId, kurumDegerleriMap]);

  useEffect(() => {
    const activePeriod = donemler.find((period) => period.id === aktifDonemId);
    const year = activePeriod?.taxYear || newTaxYear;
    const savedParameters = annualPayrollParameters.find(
      (parameters) => parameters.year === year
    );
    setAnnualTaxYear(year);
    setAnnualTaxBrackets(
      savedParameters?.gelirVergisiDilimleri.map((bracket) => ({ ...bracket })) || []
    );
    setAnnualInsuranceGvCap(
      savedParameters?.sigortaGvYillikBrutAsgariUcretTavani ?? (year === 2026 ? 396360 : 0)
    );
  }, [aktifDonemId, annualPayrollParameters, donemler, newTaxYear]);

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

  const handleAddSickLeave = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedPersonForSick || !sickStartDate || !sickEndDate) return;

    const newRecord: SickLeaveRecord = {
      id: `sick_${selectedPersonForSick}_${Date.now()}`,
      personnelId: selectedPersonForSick,
      startDate: sickStartDate,
      endDate: sickEndDate,
    };

    try {
      await onSaveSickLeaveRecord(newRecord);
      setSickSuccessMsg('Rapor olayı başarıyla kaydedildi.');
      setTimeout(() => setSickSuccessMsg(null), 2500);
      setSickStartDate('');
      setSickEndDate('');
    } catch (error) {
      alert(`Rapor olayı kaydedilemedi: ${String(error)}`);
    }
  };

  const handleDeleteSickLeave = async (id: string) => {
    try {
      await onDeleteSickLeaveRecord(id);
    } catch (error) {
      alert(`Rapor olayı silinemedi: ${String(error)}`);
    }
  };

  const handleCreateNewPeriod = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const newDonem = createBordroDonemi(newYear, newMonth, newTaxYear, newTaxMonth);
    const initialKurum: DönemselKurumDegerleri = {
      ...DEFAULT_KURUM_DEGERLERI,
      ...paramsForm,
      donemId: newDonem.id,
      gunlukTabanUcret:
        paramsForm.gunlukTabanUcret ?? DEFAULT_KURUM_DEGERLERI.gunlukTabanUcret,
      gunlukYemek: paramsForm.gunlukYemek ?? DEFAULT_KURUM_DEGERLERI.gunlukYemek,
      birlestirilmisSosyalYardim:
        paramsForm.birlestirilmisSosyalYardim ??
        DEFAULT_KURUM_DEGERLERI.birlestirilmisSosyalYardim,
      gunlukVasitaYol:
        paramsForm.gunlukVasitaYol ?? DEFAULT_KURUM_DEGERLERI.gunlukVasitaYol,
      giyimYardimi: paramsForm.giyimYardimi ?? DEFAULT_KURUM_DEGERLERI.giyimYardimi,
      hizmetZammiBirimi:
        paramsForm.hizmetZammiBirimi ?? DEFAULT_KURUM_DEGERLERI.hizmetZammiBirimi,
      isPrimiYuzde: paramsForm.isPrimiYuzde || 0,
      isPrimiGruplari: paramsForm.isPrimiGruplari || DEFAULT_IS_PRIMI_GRUPLARI,
      ekOdeme: paramsForm.ekOdeme || 0,
      tediyeListesi: paramsForm.tediyeListesi || DEFAULT_TEDIYE_LISTESI,
      tisIkramiyeListesi: paramsForm.tisIkramiyeListesi || DEFAULT_TIS_IKRAMIYE_LISTESI,
      tediyeTisNotu: paramsForm.tediyeTisNotu || DEFAULT_KURUM_DEGERLERI.tediyeTisNotu,
      sgkIsciOraniYuzde:
        paramsForm.sgkIsciOraniYuzde ?? DEFAULT_KURUM_DEGERLERI.sgkIsciOraniYuzde,
      issizlikIsciOraniYuzde:
        paramsForm.issizlikIsciOraniYuzde ?? DEFAULT_KURUM_DEGERLERI.issizlikIsciOraniYuzde,
      sgkIsverenOraniYuzde:
        paramsForm.sgkIsverenOraniYuzde ?? DEFAULT_KURUM_DEGERLERI.sgkIsverenOraniYuzde,
      issizlikIsverenOraniYuzde:
        paramsForm.issizlikIsverenOraniYuzde ??
        DEFAULT_KURUM_DEGERLERI.issizlikIsverenOraniYuzde,
      gunlukYemekIstisnasiSGK:
        paramsForm.gunlukYemekIstisnasiSGK ??
        DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiSGK,
      gunlukYemekIstisnasiGV:
        paramsForm.gunlukYemekIstisnasiGV ?? DEFAULT_KURUM_DEGERLERI.gunlukYemekIstisnasiGV,
      statutoryParameterSegments: paramsForm.statutoryParameterSegments || [],
      pekTavanKatsayisi:
        paramsForm.pekTavanKatsayisi ?? DEFAULT_KURUM_DEGERLERI.pekTavanKatsayisi,
      gunlukAsgariUcret:
        paramsForm.gunlukAsgariUcret ?? DEFAULT_KURUM_DEGERLERI.gunlukAsgariUcret,
    };

    try {
      await onCreateDonem(newDonem, initialKurum);
      await onSelectDonem(newDonem.id);
      onSectionChange('gelir');
    } catch (error) {
      alert(`Dönem kaydedilemedi: ${String(error)}`);
    }
  };

  const handleSaveParams = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await onSaveKurumDegerleri({ ...paramsForm, donemId: aktifDonemId });
      await onSaveZamAylari(zamAylariForm);
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 2500);
    } catch (error) {
      alert(`Kurum ayarları kaydedilemedi: ${String(error)}`);
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
      alert(`Yıllık vergi parametreleri kaydedilemedi: ${String(error)}`);
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
          onSubmit={handleSaveParams}
        />
      ) : (
        <MissingPeriodSection
          testId="period-settings-gelir"
          title="Gelir Parametreleri"
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
          title="Kesinti & Yasal Oranlar"
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
          title="Tediye & TİS"
          onOpenNewPeriod={openNewPeriod}
        />
      );
    }

    if (activeSection === 'sickLeave') {
      return (
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
        />
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
        setNewYear={setNewYear}
        newMonth={newMonth}
        setNewMonth={setNewMonth}
        newTaxYear={newTaxYear}
        setNewTaxYear={setNewTaxYear}
        newTaxMonth={newTaxMonth}
        setNewTaxMonth={setNewTaxMonth}
        yearOptions={yearOptions}
        resetTaxDefaults={resetTaxDefaults}
        previewDonem={previewDonem}
        previewExists={previewExists}
        previewTaxChanged={previewTaxChanged}
        onSubmit={handleCreateNewPeriod}
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
      {renderActiveSection()}
    </section>
  );
};
