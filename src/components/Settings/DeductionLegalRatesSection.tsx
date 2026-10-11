import React, { useState } from 'react';
import { Award, Percent, Plus, Save, ShieldAlert, Trash2, Check } from 'lucide-react';
import type {
  AnnualPayrollParameters,
  BordroDonemi,
  PayrollPeriodInstitutionSettings,
  StatutoryParameterSnapshot,
} from '../../types/payroll';
import { formatDateTR, formatTL } from '../../utils/payrollPresentation';

type StatutoryField =
  | 'effectiveFrom'
  | 'gunlukAsgariUcret'
  | 'pekTavanKatsayisi'
  | 'gunlukYemekIstisnasiSGK'
  | 'gunlukYemekIstisnasiGV';

export type SendikaAidatiYontemi = 'oran' | 'sabit';

export const getSendikaAidatiYontemi = (
  sabitSendikaAidati: number | null | undefined
): SendikaAidatiYontemi => ((sabitSendikaAidati ?? 0) > 0 ? 'sabit' : 'oran');

export const applySendikaAidatiYontemi = (
  params: PayrollPeriodInstitutionSettings,
  yontem: SendikaAidatiYontemi
): PayrollPeriodInstitutionSettings =>
  yontem === 'oran' ? { ...params, sabitSendikaAidati: 0 } : params;

interface DeductionLegalRatesSectionProps {
  aktifDonemId: string;
  activePeriodForParams?: BordroDonemi;
  annualPayrollParameters?: AnnualPayrollParameters[];
  paramsForm: PayrollPeriodInstitutionSettings;
  setParamsForm: React.Dispatch<React.SetStateAction<PayrollPeriodInstitutionSettings>>;
  savedSuccess: boolean;
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void> | void;
}

export const DeductionLegalRatesSection: React.FC<DeductionLegalRatesSectionProps> = ({
  aktifDonemId,
  activePeriodForParams,
  annualPayrollParameters = [],
  paramsForm,
  setParamsForm,
  savedSuccess,
  onSubmit,
}) => {
  const updateNumber = (
    field: keyof PayrollPeriodInstitutionSettings,
    value: number
  ) => {
    setParamsForm((current) => ({
      ...current,
      [field]: value,
      ...(field === 'sendikaAidatiYuzde' ? { sabitSendikaAidati: 0 } : {}),
    }));
  };

  const sendikaOrani = paramsForm.sendikaAidatiYuzde ?? 65;
  const sabitSendikaAidati = paramsForm.sabitSendikaAidati ?? 0;
  const [sabitYontemiTaslakDonemId, setSabitYontemiTaslakDonemId] = useState<string | null>(null);
  const [sabitTutarHatasi, setSabitTutarHatasi] = useState(false);
  const sendikaAidatiYontemi =
    getSendikaAidatiYontemi(sabitSendikaAidati) === 'sabit' ||
    sabitYontemiTaslakDonemId === aktifDonemId
      ? 'sabit'
      : 'oran';

  const selectSendikaAidatiYontemi = (yontem: SendikaAidatiYontemi) => {
    setSabitYontemiTaslakDonemId(yontem === 'sabit' ? aktifDonemId : null);
    setSabitTutarHatasi(false);
    setParamsForm((current) => applySendikaAidatiYontemi(current, yontem));
  };

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    if (sendikaAidatiYontemi === 'sabit' && sabitSendikaAidati <= 0) {
      event.preventDefault();
      setSabitTutarHatasi(true);
      return;
    }

    setSabitTutarHatasi(false);
    onSubmit(event);
  };

  const addStatutorySegment = () => {
    setParamsForm((current) => ({
      ...current,
      statutoryParameterSegments: [
        ...(current.statutoryParameterSegments || []),
        { effectiveFrom: activePeriodForParams?.baslangicTarihi || '' },
      ],
    }));
  };

  const updateStatutorySegment = (
    index: number,
    field: StatutoryField,
    value: string | number | undefined
  ) => {
    setParamsForm((current) => ({
      ...current,
      statutoryParameterSegments: (current.statutoryParameterSegments || []).map(
        (segment, segmentIndex) =>
          segmentIndex === index ? { ...segment, [field]: value } : segment
      ),
    }));
  };

  const removeStatutorySegment = (index: number) => {
    setParamsForm((current) => ({
      ...current,
      statutoryParameterSegments: (current.statutoryParameterSegments || []).filter(
        (_, segmentIndex) => segmentIndex !== index
      ),
    }));
  };

  const segments = paramsForm.statutoryParameterSegments || [];
  const statutorySnapshot: StatutoryParameterSnapshot | undefined =
    paramsForm.statutoryParameterSnapshot;
  const taxYear = activePeriodForParams?.taxYear;
  const taxMonth = activePeriodForParams?.taxMonth;
  const savedAnnualParameters = taxYear === undefined
    ? undefined
    : annualPayrollParameters.find((parameters) => parameters.year === taxYear);
  const serviceYear = activePeriodForParams?.yil ?? Number(aktifDonemId.slice(0, 4));
  const formatAmount = (value: number | undefined) =>
    value === undefined ? 'kayıtlı değil' : formatTL(value);
  const sourceSegments = statutorySnapshot?.statutoryParameterSegments ?? segments;

  return (
    <section data-testid="period-settings-kesinti" className="space-y-5">
      <header>
        <h2 className="text-xl font-bold text-slate-900">Vergi &amp; Yasal Oranlar</h2>
        <p className="mt-1 text-xs text-slate-500">
          Aktif dönemin yasal kesinti oranlarını ve dönem içi mevzuat değişikliklerini yönetin.
        </p>
      </header>

      <form onSubmit={handleSubmit} className="space-y-5">
        <div className="bg-rose-50 border border-rose-100 rounded-xl p-3.5 flex items-start gap-3">
          <ShieldAlert className="w-5 h-5 text-rose-600 shrink-0 mt-0.5" />
          <div className="text-xs text-rose-950 leading-relaxed">
            <strong>{aktifDonemId}</strong> dönemi için kesinti parametreleri ve yasal vergi/sigorta oranları.
            SGK, İşsizlik, Gelir Vergisi, Damga Vergisi, Sendika ve BES kesintileri bu oranlardan hesaplanıp otomatik bordroya aktarılır.
          </div>
        </div>

        {savedSuccess && (
          <div className="p-3 bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-xl text-xs font-semibold flex items-center gap-2 animate-in fade-in">
            <Check className="w-4 h-4 text-emerald-600" />
            <span>Kesinti ve yasal oran parametreleri başarıyla kaydedildi!</span>
          </div>
        )}

        <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl space-y-3">
          <h3 className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
            <Percent className="w-4 h-4 text-indigo-600" />
            <span>Yasal Vergi ve Sigorta Oranları</span>
          </h3>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                SGK İşçi Primi Oranı (%)
              </label>
              <input
                type="number"
                step="0.1"
                value={paramsForm.sgkIsciOraniYuzde ?? 14}
                onChange={(e) => updateNumber('sgkIsciOraniYuzde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Dönem oranı girdisi: %{paramsForm.sgkIsciOraniYuzde ?? 'kayıtlı değil'}. Kaynak doğrulaması bu kayda bağlı değildir.
              </span>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                İşçi İşsizlik Sigortası Oranı (%)
              </label>
              <input
                type="number"
                step="0.1"
                value={paramsForm.issizlikIsciOraniYuzde ?? 1}
                onChange={(e) => updateNumber('issizlikIsciOraniYuzde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Dönem oranı girdisi: %{paramsForm.issizlikIsciOraniYuzde ?? 'kayıtlı değil'}. Kaynak doğrulaması bu kayda bağlı değildir.
              </span>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                Gelir Vergisi Tarifesi
              </label>
              <div className="px-3 py-2 bg-slate-100 border border-slate-300 rounded-lg text-xs font-semibold text-slate-800">
                Yıllık GV parametre tablosu kullanılır
              </div>
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Tarifeyi “Yıllık GV Tarifesi” bölümünden yönetin.
              </span>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                Damga Vergisi Oranı (‰ Binde)
              </label>
              <input
                type="number"
                step="0.001"
                value={paramsForm.damgaVergisiOraniBinde ?? 7.59}
                onChange={(e) => updateNumber('damgaVergisiOraniBinde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Dönem oranı girdisi: {paramsForm.damgaVergisiOraniBinde ?? 'kayıtlı değil'} ‰. Kaynak doğrulaması bu kayda bağlı değildir.
              </span>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                SGK İşveren Prim Oranı (%)
              </label>
              <input
                type="number"
                step="0.01"
                value={paramsForm.sgkIsverenOraniYuzde ?? 21.75}
                onChange={(e) => updateNumber('sgkIsverenOraniYuzde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Dönem oranı girdisi: %{paramsForm.sgkIsverenOraniYuzde ?? 'kayıtlı değil'}. Kaynak doğrulaması bu kayda bağlı değildir.
              </span>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                İşveren İşsizlik Sigortası Oranı (%)
              </label>
              <input
                type="number"
                step="0.01"
                value={paramsForm.issizlikIsverenOraniYuzde ?? 2.0}
                onChange={(e) => updateNumber('issizlikIsverenOraniYuzde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono font-bold focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                Dönem oranı girdisi: %{paramsForm.issizlikIsverenOraniYuzde ?? 'kayıtlı değil'}. Kaynak doğrulaması bu kayda bağlı değildir.
              </span>
            </div>
          </div>
        </div>

        <div className="bg-slate-50 border border-slate-200 p-4 rounded-xl space-y-3">
          <h3 className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
            <Award className="w-4 h-4 text-amber-600" />
            <span>Sendika Aidatı ve Otomatik Katılım (OKS) Parametreleri</span>
          </h3>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <fieldset className="sm:col-span-2 space-y-2">
              <legend className="block text-xs font-semibold text-slate-700">
                Sendika aidatı hesaplama yöntemi
              </legend>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <label
                  className={`flex items-start gap-2 rounded-lg border px-3 py-2.5 cursor-pointer focus-within:ring-2 focus-within:ring-indigo-500 ${
                    sendikaAidatiYontemi === 'oran'
                      ? 'border-indigo-400 bg-indigo-50'
                      : 'border-slate-300 bg-white'
                  }`}
                >
                  <input
                    type="radio"
                    name="sendikaAidatiYontemi"
                    value="oran"
                    checked={sendikaAidatiYontemi === 'oran'}
                    onChange={() => selectSendikaAidatiYontemi('oran')}
                    className="mt-0.5 accent-indigo-600"
                  />
                  <span>
                    <span className="block text-xs font-semibold text-slate-800">Oran üzerinden</span>
                    <span className="mt-0.5 block text-[11px] text-slate-600">
                      Günlük çıplak ücret × oran; bu seçime geçerken sabit tutar temizlenir.
                    </span>
                  </span>
                </label>
                <label
                  className={`flex items-start gap-2 rounded-lg border px-3 py-2.5 cursor-pointer focus-within:ring-2 focus-within:ring-indigo-500 ${
                    sendikaAidatiYontemi === 'sabit'
                      ? 'border-indigo-400 bg-indigo-50'
                      : 'border-slate-300 bg-white'
                  }`}
                >
                  <input
                    type="radio"
                    name="sendikaAidatiYontemi"
                    value="sabit"
                    checked={sendikaAidatiYontemi === 'sabit'}
                    onChange={() => selectSendikaAidatiYontemi('sabit')}
                    className="mt-0.5 accent-indigo-600"
                  />
                  <span>
                    <span className="block text-xs font-semibold text-slate-800">Sabit tutar</span>
                    <span className="mt-0.5 block text-[11px] text-slate-600">
                      Belirlenen TL / gün tutarı
                    </span>
                  </span>
                </label>
              </div>
            </fieldset>

            <div className="sm:col-span-2" role="status" aria-live="polite">
              <p className="text-xs font-semibold text-slate-800">
                {sendikaAidatiYontemi === 'sabit'
                  ? sabitSendikaAidati > 0
                    ? `Kurum genelinde uygulanan yöntem: Sabit tutar — ${formatTL(sabitSendikaAidati)} / gün.`
                    : 'Kurum genelinde sabit yöntem seçildi; günlük tutarı girin.'
                  : `Kurum genelinde uygulanan yöntem: Oran — günlük çıplak ücret × %${sendikaOrani}.`}
              </p>
              <p className="mt-0.5 text-[11px] text-slate-500">
                Personel kartında kişiye özel sabit aidat varsa, o tutar bu kurum ayarından önce uygulanır.
              </p>
            </div>

            {sendikaAidatiYontemi === 'oran' ? (
              <div className="sm:col-span-2 sm:max-w-xl">
                <label htmlFor="sendika-aidati-orani" className="block text-xs font-semibold text-slate-700 mb-1">
                  Günlük Çıplak Ücret Oranı (%)
                </label>
                <input
                  id="sendika-aidati-orani"
                  type="number"
                  step="1"
                  value={paramsForm.sendikaAidatiYuzde ?? 65}
                  onChange={(e) => updateNumber('sendikaAidatiYuzde', parseFloat(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  Günlük çıplak ücret, etkin ücret politikası sürümünden alınır.
                </span>
              </div>
            ) : (
              <div className="sm:col-span-2 sm:max-w-xl">
                <label htmlFor="sendika-aidati-sabit-tutar" className="block text-xs font-semibold text-slate-700 mb-1">
                  Günlük Sabit Sendika Aidatı (TL / gün)
                </label>
                <input
                  id="sendika-aidati-sabit-tutar"
                  type="number"
                  step="0.01"
                  value={sabitSendikaAidati}
                  onChange={(e) => {
                    const tutar = parseFloat(e.target.value) || 0;
                    setSabitYontemiTaslakDonemId(aktifDonemId);
                    updateNumber('sabitSendikaAidati', tutar);
                    if (tutar > 0) setSabitTutarHatasi(false);
                  }}
                  aria-invalid={sabitTutarHatasi}
                  aria-describedby={
                    sabitTutarHatasi
                      ? 'sendika-aidati-sabit-yardim sendika-aidati-sabit-hata'
                      : 'sendika-aidati-sabit-yardim'
                  }
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
                />
                <span id="sendika-aidati-sabit-yardim" className="text-[11px] text-slate-500 mt-0.5 block">
                  Bu tutar, kişi kartında özel sabit aidat tanımlanmayan sendika üyeleri için geçerlidir.
                </span>
                {sabitTutarHatasi && (
                  <span id="sendika-aidati-sabit-hata" role="alert" className="text-[11px] text-rose-700 mt-1 block">
                    Sabit yöntem için 0’dan büyük günlük bir tutar girin.
                  </span>
                )}
              </div>
            )}

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">
                Otomatik Katılım (OKS) Oranı (%)
              </label>
              <input
                type="number"
                step="0.1"
                value={paramsForm.besOraniYuzde ?? 3}
                onChange={(e) => updateNumber('besOraniYuzde', parseFloat(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-rose-500"
              />
              <span className="text-[11px] text-slate-500 mt-0.5 block">
                PEK × %3 (Hesaplanan tutarın kuruş kısmı atılır)
              </span>
            </div>
          </div>

          <div className="pt-4 border-t border-slate-200">
            <h3 className="text-xs font-bold text-slate-900 uppercase tracking-wider mb-3">
              SGK Prime Esas Kazanç (PEK) Parametreleri — Hizmet dönemi {serviceYear}
            </h3>
            <div
              data-testid="statutory-parameter-context"
              className="mb-4 rounded-lg border border-slate-200 bg-white p-3 text-[11px] leading-relaxed text-slate-700"
            >
              <p>
                <strong>Hizmet dönemi:</strong>{' '}
                {activePeriodForParams
                  ? `${activePeriodForParams.id} (${activePeriodForParams.baslangicTarihi}–${activePeriodForParams.bitisTarihi})`
                  : aktifDonemId}
                {' · '}
                <strong>Ödeme/vergi dönemi:</strong>{' '}
                {taxYear === undefined
                  ? 'dönem kaydı yok'
                  : `${taxYear}-${String(taxMonth ?? 0).padStart(2, '0')} (${taxYear} vergi yılı)`}
              </p>
              {statutorySnapshot ? (
                <>
                  <p className="mt-1 font-semibold text-indigo-800">
                    Bordro motoru kayıtlı dönem yasal parametre snapshot’ını kullanıyor. Alttaki girişler dönem ayarlarıdır; snapshot değerleri değişmez.
                  </p>
                  <div data-testid="statutory-engine-snapshot" className="mt-2 grid grid-cols-1 gap-1 sm:grid-cols-2">
                    <span>Snapshot günlük asgari ücret: {formatAmount(statutorySnapshot.gunlukAsgariUcret)}</span>
                    <span>Snapshot PEK tavan katsayısı: {statutorySnapshot.pekTavanKatsayisi}</span>
                    <span>Snapshot SGK yemek istisnası: {formatAmount(statutorySnapshot.gunlukYemekIstisnasiSGK)} / gün</span>
                    <span>Snapshot GV yemek istisnası: {formatAmount(statutorySnapshot.gunlukYemekIstisnasiGV)} / gün</span>
                    <span>Snapshot SGK işçi primi: %{statutorySnapshot.sgkIsciOraniYuzde ?? 'kayıtlı değil'}</span>
                    <span>Snapshot işsizlik işçi primi: %{statutorySnapshot.issizlikIsciOraniYuzde ?? 'kayıtlı değil'}</span>
                  </div>
                  {sourceSegments.length > 0 && (
                    <ul className="mt-2 list-disc pl-4">
                      {sourceSegments.map((segment, index) => (
                        <li key={`${segment.effectiveFrom}-${index}`}>
                          {segment.effectiveFrom} yürürlük: günlük asgari ücret {formatAmount(segment.gunlukAsgariUcret)}, PEK katsayısı {segment.pekTavanKatsayisi ?? 'değişmiyor'}, SGK yemek {formatAmount(segment.gunlukYemekIstisnasiSGK)}, GV yemek {formatAmount(segment.gunlukYemekIstisnasiGV)}.
                        </li>
                      ))}
                    </ul>
                  )}
                  <p className="mt-1 text-amber-800">
                    Bu snapshot’ta mevzuat yayını, kaynak bağlantısı veya sürüm bilgisi kayıtlı değil.
                  </p>
                </>
              ) : (
                <p className="mt-1 font-semibold text-amber-800">
                  Kayıtlı yasal snapshot yok; bordro motoru dönem ayarları ve dönem içi segmentleri kullanır. Bu girdilerin mevzuat kaynağı ve sürümü kayıtlı olmadığından doğrulanmış yasal değer olarak gösterilmez.
                </p>
              )}
              <p className="mt-2">
                {taxYear === undefined ? (
                  'Ödeme/vergi yılı belirlenemediği için yıllık gelir vergisi parametre durumu gösterilemiyor.'
                ) : savedAnnualParameters ? (
                  <>
                    {taxYear} vergi yılı gelir vergisi parametre kaydı mevcut
                    {savedAnnualParameters.updatedAt
                      ? `; son güncelleme ${savedAnnualParameters.updatedAt}`
                      : '; kayıtlı güncelleme tarihi yok'}.
                    {' '}Mevzuat yayın kaynağı bu kayıtta tutulmuyor.
                  </>
                ) : (
                  <strong>{taxYear} vergi yılı gelir vergisi parametre kaydı bulunmuyor; bordro motoru bu yıl için tarife bulamaz.</strong>
                )}
              </p>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">
                  Günlük SGK Yemek İstisnası (TL)
                </label>
                <input
                  type="number"
                  step="0.01"
                  value={paramsForm.gunlukYemekIstisnasiSGK ?? 300.00}
                  onChange={(e) => updateNumber('gunlukYemekIstisnasiSGK', parseFloat(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  Dönem ayarı girdisi: {formatAmount(paramsForm.gunlukYemekIstisnasiSGK)} / gün. Kaynak doğrulaması bu kayda bağlı değildir.
                </span>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">
                  Günlük GV Yemek İstisnası (TL)
                </label>
                <input
                  type="number"
                  step="0.01"
                  value={paramsForm.gunlukYemekIstisnasiGV ?? ''}
                  onChange={(e) => updateNumber('gunlukYemekIstisnasiGV', parseFloat(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  Gelir vergisi istisnası SGK limitinden bağımsızdır.
                </span>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">
                  PEK Tavan Katsayısı
                </label>
                <input
                  type="number"
                  step="0.5"
                  value={paramsForm.pekTavanKatsayisi ?? 9}
                  onChange={(e) => updateNumber('pekTavanKatsayisi', parseFloat(e.target.value) || 9)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  Dönem ayarı katsayısı: {paramsForm.pekTavanKatsayisi ?? 'kayıtlı değil'}. Mevzuat kaynağı bu kayda bağlı değildir.
                </span>
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">
                  Günlük Brüt Asgari Ücret (TL)
                </label>
                <input
                  type="number"
                  step="0.01"
                  value={paramsForm.gunlukAsgariUcret ?? 1101.00}
                  onChange={(e) => updateNumber('gunlukAsgariUcret', parseFloat(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-white border border-slate-300 rounded-lg text-sm text-slate-900 font-mono focus:ring-2 focus:ring-indigo-500"
                />
                <span className="text-[11px] text-slate-500 mt-0.5 block">
                  Dönem ayarı günlük tutarı: {formatAmount(paramsForm.gunlukAsgariUcret)}. Mevzuat kaynağı bu kayda bağlı değildir.
                </span>
              </div>
            </div>

            <div className="mt-4 pt-4 border-t border-slate-200 space-y-3">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <h4 className="text-xs font-bold text-slate-900">Dönem İçi Yasal Parametre Değişimleri</h4>
                  <p className="text-[11px] text-slate-600 mt-1">
                    Yalnız aktif/gelecek {activePeriodForParams?.baslangicTarihi}–{activePeriodForParams?.bitisTarihi}
                    dönemi içindeki yürürlük değişimlerini girin. Bu alan geçmiş mevzuat arşivi oluşturmaz.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={addStatutorySegment}
                  className="px-3 py-1.5 rounded-lg bg-indigo-100 hover:bg-indigo-200 text-indigo-800 text-xs font-semibold flex items-center gap-1 shrink-0"
                >
                  <Plus className="w-3.5 h-3.5" /> Dönem İçi Yasal Değişiklik Ekle
                </button>
              </div>

              {segments.map((segment, index) => (
                <div
                  key={`${segment.effectiveFrom}-${index}`}
                  className="border border-indigo-200 bg-indigo-50/50 rounded-xl p-3 space-y-3"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-bold text-indigo-900">
                      {segment.effectiveFrom
                        ? `${formatDateTR(segment.effectiveFrom)} tarihinden itibaren geçerli değişiklik`
                        : 'Dönem içi yasal değişiklik'}
                    </span>
                    <button
                      type="button"
                      onClick={() => removeStatutorySegment(index)}
                      className="p-1.5 rounded-lg text-rose-600 hover:bg-rose-100"
                      title="Değişikliği kaldır"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
                    <div>
                      <label className="block text-[11px] font-semibold text-slate-700 mb-1">
                        Yürürlük Tarihi
                      </label>
                      <input
                        type="date"
                        min={activePeriodForParams?.baslangicTarihi}
                        max={activePeriodForParams?.bitisTarihi}
                        value={segment.effectiveFrom}
                        onChange={(e) =>
                          updateStatutorySegment(index, 'effectiveFrom', e.target.value)
                        }
                        className="w-full px-2 py-2 bg-white border border-slate-300 rounded-lg text-xs"
                      />
                    </div>
                    {[
                      ['gunlukAsgariUcret', 'Günlük Asgari'],
                      ['pekTavanKatsayisi', 'PEK Katsayı'],
                      ['gunlukYemekIstisnasiSGK', 'SGK Yemek'],
                      ['gunlukYemekIstisnasiGV', 'GV Yemek'],
                    ].map(([field, label]) => (
                      <div key={field}>
                        <label className="block text-[11px] font-semibold text-slate-700 mb-1">
                          {label}
                        </label>
                        <input
                          type="number"
                          step="0.01"
                          value={(segment as Record<string, number | string | undefined>)[field] ?? ''}
                          placeholder="Değişmiyor"
                          onChange={(e) =>
                            updateStatutorySegment(
                              index,
                              field as StatutoryField,
                              e.target.value === '' ? undefined : Number(e.target.value)
                            )
                          }
                          className="w-full px-2 py-2 bg-white border border-slate-300 rounded-lg text-xs font-mono"
                        />
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="pt-3 border-t border-slate-200 flex justify-end">
          <button
            type="submit"
            className="px-5 py-2.5 bg-rose-600 hover:bg-rose-700 text-white rounded-xl text-xs font-semibold shadow-xs transition-colors flex items-center gap-2"
          >
            <Save className="w-4 h-4" />
            <span>Kesinti ve Yasal Oranları Kaydet</span>
          </button>
        </div>
      </form>
    </section>
  );
};
