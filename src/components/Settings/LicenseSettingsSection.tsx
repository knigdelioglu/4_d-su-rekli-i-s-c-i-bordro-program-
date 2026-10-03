import React, { useEffect, useState } from 'react';
import { AlertCircle, CheckCircle2, KeyRound, RefreshCw, ShieldCheck } from 'lucide-react';
import { tauriBridge, type LicenseStatus } from '../../services/tauriBridge';
import { describeError } from '../../utils/errorMessage';

function stateLabel(state: LicenseStatus['state']): string {
  switch (state) {
    case 'active': return 'Etkin';
    case 'pending': return 'Onay bekliyor';
    case 'denied': return 'Reddedildi';
    case 'expired': return 'Süre doldu';
    case 'invalid': return 'Doğrulanamadı';
    case 'configurationError': return 'Yapılandırma gerekli';
    default: return 'Etkinleştirilmedi';
  }
}

function dateLabel(value: string | null): string {
  if (!value) return '—';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '—' : new Intl.DateTimeFormat('tr-TR', { dateStyle: 'medium' }).format(date);
}

export const LicenseSettingsSection: React.FC = () => {
  const [status, setStatus] = useState<LicenseStatus | null>(null);
  const [licenseKey, setLicenseKey] = useState('');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const isDesktop = tauriBridge.isTauriAvailable();

  useEffect(() => {
    let active = true;
    if (!isDesktop) {
      setIsLoading(false);
      return () => { active = false; };
    }
    // This command reads only local Rust-owned state and never contacts the license server.
    void tauriBridge.getLicenseStatus().then((next) => {
      if (active) setStatus(next);
    }).catch((error) => {
      if (active) setErrorMessage(describeError(error));
    }).finally(() => {
      if (active) setIsLoading(false);
    });
    return () => { active = false; };
  }, [isDesktop]);

  const runExplicitCheck = async () => {
    setErrorMessage(null);
    setSuccessMessage(null);
    setIsSubmitting(true);
    try {
      const next = await tauriBridge.refreshLicenseStatus();
      setStatus(next);
      setSuccessMessage(next.state === 'pending'
        ? 'Talep henüz sonuçlanmadı. Daha sonra yeniden kontrol edebilirsiniz.'
        : 'Lisans durumu sunucudan yenilendi.');
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setIsSubmitting(false);
    }
  };

  const submitActivation = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setErrorMessage(null);
    setSuccessMessage(null);
    setIsSubmitting(true);
    try {
      const next = await tauriBridge.activateLicense(licenseKey);
      setStatus(next);
      setLicenseKey('');
      setSuccessMessage('Aktivasyon talebi oluşturuldu. Yönetici onayından sonra durumu burada yenileyin.');
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <section data-testid="license-settings-section" className="space-y-5">
      <header>
        <h2 className="text-xl font-bold text-slate-900">Lisans</h2>
        <p className="mt-1 max-w-3xl text-xs leading-relaxed text-slate-600">
          Lisans durumu bu cihazda saklanır. Kayıt, personel veya bordro verileri lisans sunucusuna gönderilmez.
        </p>
      </header>

      {!isDesktop ? (
        <div className="rounded-xl border border-slate-200 bg-slate-50 p-4 text-sm text-slate-700">
          Lisans etkinleştirme yalnızca masaüstü uygulamasında kullanılabilir.
        </div>
      ) : isLoading ? (
        <div role="status" className="rounded-xl border border-slate-200 bg-white p-4 text-sm text-slate-600">Yerel lisans durumu okunuyor…</div>
      ) : (
        <>
          {status && (
            <div className="rounded-xl border border-slate-200 bg-white p-4">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="flex items-start gap-3">
                  <ShieldCheck aria-hidden="true" className="mt-0.5 h-5 w-5 text-indigo-600" />
                  <div>
                    <h3 className="text-sm font-bold text-slate-900">{stateLabel(status.state)}</h3>
                    <p className="mt-1 text-xs text-slate-600">
                      Uygulama modu: {status.mode === 'required' ? 'Lisans zorunlu' : 'Lisans isteğe bağlı'} · Bu cihaz: {status.deviceId}
                    </p>
                  </div>
                </div>
                {(status.state === 'pending' || status.licenseId) && (
                  <button
                    type="button"
                    onClick={runExplicitCheck}
                    disabled={isSubmitting}
                    className="inline-flex min-h-9 items-center gap-2 rounded-lg border border-slate-300 px-3 text-xs font-semibold text-slate-700 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    <RefreshCw aria-hidden="true" className="h-3.5 w-3.5" />
                    {isSubmitting ? 'Kontrol ediliyor…' : 'Durumu yenile'}
                  </button>
                )}
              </div>
              <dl className="mt-4 grid gap-x-6 gap-y-2 border-t border-slate-100 pt-3 text-xs sm:grid-cols-2">
                <div><dt className="text-slate-500">Geçerlilik sonu</dt><dd className="mt-0.5 font-semibold text-slate-800">{dateLabel(status.expiresAt)}</dd></div>
                <div><dt className="text-slate-500">Son çevrimiçi doğrulama</dt><dd className="mt-0.5 font-semibold text-slate-800">{dateLabel(status.lastOnlineAt)}</dd></div>
                {status.mode === 'required' && status.state !== 'active' && (
                  <div className="sm:col-span-2"><dt className="sr-only">İşlem sınırı</dt><dd className="mt-1 text-amber-800">Yeni bordro hesaplama ve kesinleştirme durdurulmuştur. Mevcut kayıtları görüntüleme ve yedek alma kullanılabilir.</dd></div>
                )}
              </dl>
              {status.reason && (
                <p role="status" className="mt-3 rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-900">{status.reason}</p>
              )}
            </div>
          )}

          {status?.mode === 'optional' && (
            <div className="rounded-xl border border-emerald-200 bg-emerald-50 p-3.5 text-xs leading-relaxed text-emerald-900">
              Lisans zorunluluğu şu an kapalı. Aktivasyon yapmadan mevcut bordro akışınızı kullanabilirsiniz.
            </div>
          )}

          {status?.state !== 'active' && (
            <form onSubmit={submitActivation} className="rounded-xl border border-slate-200 bg-white p-4">
              <label htmlFor="license-key" className="block text-xs font-bold text-slate-800">Lisans anahtarı</label>
              <p className="mt-1 text-xs text-slate-500">Anahtar ilk aktivasyonda çevrimiçi doğrulanır. Cihaz onayı manuel yapılır.</p>
              <div className="mt-3 flex flex-col gap-2 sm:flex-row">
                <input
                  id="license-key"
                  name="licenseKey"
                  type="password"
                  autoComplete="off"
                  value={licenseKey}
                  onChange={(event) => setLicenseKey(event.target.value)}
                  maxLength={160}
                  required
                  className="min-h-10 flex-1 rounded-lg border border-slate-300 px-3 text-sm outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-100"
                />
                <button
                  type="submit"
                  disabled={isSubmitting || !licenseKey.trim()}
                  className="inline-flex min-h-10 items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 text-xs font-bold text-white hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <KeyRound aria-hidden="true" className="h-4 w-4" />
                  {isSubmitting ? 'Gönderiliyor…' : 'Aktivasyon talebi gönder'}
                </button>
              </div>
            </form>
          )}

          {errorMessage && (
            <div role="alert" className="flex items-start gap-2 rounded-xl border border-rose-200 bg-rose-50 p-3.5 text-xs text-rose-900">
              <AlertCircle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" /><span>{errorMessage}</span>
            </div>
          )}
          {successMessage && (
            <div role="status" className="flex items-start gap-2 rounded-xl border border-emerald-200 bg-emerald-50 p-3.5 text-xs text-emerald-900">
              <CheckCircle2 aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" /><span>{successMessage}</span>
            </div>
          )}
        </>
      )}
    </section>
  );
};
