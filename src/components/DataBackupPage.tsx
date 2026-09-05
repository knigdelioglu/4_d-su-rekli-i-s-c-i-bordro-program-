import React, { useRef } from 'react';
import { CheckCircle2, Download, HardDrive, RotateCcw, Upload } from 'lucide-react';

interface DataBackupPageProps {
  lastSavedAt?: string;
  hasData: boolean;
  storageLabel?: string;
  storageDetail?: string;
  onExportBackup: () => void;
  onImportBackup: (jsonStr: string) => void;
  onResetSampleData: () => void;
}

function formatSavedAt(value?: string): string {
  if (!value) return 'Henüz kayıt zamanı bulunmuyor';
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleString('tr-TR', { dateStyle: 'medium', timeStyle: 'short' });
}

export const DataBackupPage: React.FC<DataBackupPageProps> = ({
  lastSavedAt,
  hasData,
  storageLabel = 'Bu tarayıcıda yerel kayıt',
  storageDetail = 'Veriler bu tarayıcıda yerel olarak tutulur; düzenli JSON yedeği almanız önerilir.',
  onExportBackup,
  onImportBackup,
  onResetSampleData,
}) => {
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const confirmed = window.confirm(
      'Yedekten geri yükleme mevcut verileri yedekteki verilerle değiştirecek. Bu işlem geri alınamaz. Devam etmek istiyor musunuz?'
    );
    if (!confirmed) {
      event.target.value = '';
      return;
    }
    const reader = new FileReader();
    reader.onload = (loadEvent) => {
      const content = loadEvent.target?.result;
      if (typeof content === 'string' && content) onImportBackup(content);
    };
    reader.readAsText(file);
    event.target.value = '';
  };

  return (
    <section data-testid="data-backup-page" className="mx-auto max-w-3xl space-y-5">
      <header>
        <p className="text-[10px] font-bold uppercase tracking-[0.16em] text-indigo-600">Uygulama verisi</p>
        <h2 className="mt-1 text-2xl font-bold tracking-tight text-slate-900">Veri ve Yedekleme</h2>
        <p className="mt-1 text-sm text-slate-600">Kayıtlarınızın nerede tutulduğunu görün ve geri yükleme işlemlerini buradan yönetin.</p>
      </header>

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
        <div className="flex items-start gap-3 border-b border-slate-200 pb-4">
          <div className="rounded-xl bg-emerald-50 p-2.5 text-emerald-700"><HardDrive className="h-5 w-5" aria-hidden="true" /></div>
          <div>
            <h3 className="text-sm font-bold text-slate-900">Depolama</h3>
            <p className="mt-1 flex items-center gap-1.5 text-xs font-semibold text-emerald-700"><CheckCircle2 className="h-4 w-4" aria-hidden="true" /> {storageLabel}</p>
            <p className="mt-1 text-xs text-slate-500">{storageDetail}</p>
          </div>
        </div>
        <dl className="mt-4 grid gap-3 sm:grid-cols-2">
          <div className="rounded-xl bg-slate-50 px-3 py-2.5"><dt className="text-[10px] font-bold uppercase tracking-wide text-slate-500">Son kayıt</dt><dd className="mt-1 text-sm font-semibold text-slate-800">{formatSavedAt(lastSavedAt)}</dd></div>
          <div className="rounded-xl bg-slate-50 px-3 py-2.5"><dt className="text-[10px] font-bold uppercase tracking-wide text-slate-500">Durum</dt><dd className="mt-1 text-sm font-semibold text-slate-800">{hasData ? 'Kayıtlı veri var' : 'Boş başlangıç verisi'}</dd></div>
        </dl>
      </div>

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
        <h3 className="text-sm font-bold text-slate-900">Yedek işlemleri</h3>
        <div className="mt-3 flex flex-wrap gap-2">
          <button type="button" onClick={onExportBackup} className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-xs font-bold text-white hover:bg-indigo-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"><Download className="h-4 w-4" aria-hidden="true" /> Yedek İndir</button>
          <button type="button" onClick={() => fileInputRef.current?.click()} className="inline-flex items-center gap-2 rounded-xl border border-slate-300 bg-white px-4 py-2.5 text-xs font-bold text-slate-700 hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"><Upload className="h-4 w-4" aria-hidden="true" /> Yedekten Geri Yükle</button>
          <input ref={fileInputRef} type="file" accept=".json,application/json" onChange={handleFileChange} className="hidden" />
        </div>
      </div>

      <div className="rounded-2xl border border-amber-200 bg-amber-50 p-5">
        <h3 className="text-sm font-bold text-amber-950">Gelişmiş</h3>
        <p className="mt-1 text-xs leading-relaxed text-amber-900">Örnek verileri yüklemek mevcut kayıtları değiştirir. Gerçek kurum verisiyle çalışırken yalnızca test amacıyla kullanın.</p>
        <button type="button" onClick={onResetSampleData} className="mt-3 inline-flex items-center gap-2 rounded-xl border border-amber-300 bg-white px-4 py-2.5 text-xs font-bold text-amber-900 hover:bg-amber-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500"><RotateCcw className="h-4 w-4" aria-hidden="true" /> Örnek Verileri Yeniden Yükle</button>
      </div>
    </section>
  );
};
