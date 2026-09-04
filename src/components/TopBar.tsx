/**
 * Top application bar for the 4/D Sürekli İşçi Bordro Programı.
 */

import React, { useRef } from 'react';
import {
  Building2,
  Calendar,
  Download,
  Menu,
  RotateCcw,
  Upload,
  X,
} from 'lucide-react';
import { BordroDonemi } from '../types/payroll';

interface TopBarProps {
  donemler: BordroDonemi[];
  aktifDonemId: string;
  onSelectDonem: (donemId: string) => void;
  onExportBackup: () => void;
  onImportBackup: (jsonStr: string) => void;
  onResetSampleData: () => void;
  isSidebarOpen: boolean;
  onToggleSidebar: () => void;
}

export const TopBar: React.FC<TopBarProps> = ({
  donemler,
  aktifDonemId,
  onSelectDonem,
  onExportBackup,
  onImportBackup,
  onResetSampleData,
  isSidebarOpen,
  onToggleSidebar,
}) => {
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const confirmed = window.confirm(
      'Yedekten geri yükleme mevcut personel, dönem, puantaj, bordro, vergi açılışı, rapor kayıtları ve dönem parametrelerini yedekteki verilerle değiştirecek. Bu işlem geri alınamaz. Devam etmek istiyor musunuz?'
    );
    if (!confirmed) {
      e.target.value = '';
      return;
    }

    const reader = new FileReader();
    reader.onload = (event) => {
      const content = event.target?.result as string;
      if (content) {
        onImportBackup(content);
      }
    };
    reader.readAsText(file);
    e.target.value = '';
  };

  return (
    <header className="sticky top-0 z-40 bg-slate-900 text-white shadow-lg">
      <div className="w-full px-4 py-3 sm:px-6 lg:px-8">
        <div className="flex flex-col items-center justify-between gap-3 md:flex-row">
          {/* Brand */}
          <div className="flex min-w-0 items-center gap-2 sm:gap-3">
            <button
              type="button"
              data-testid="sidebar-toggle"
              onClick={onToggleSidebar}
              aria-label={isSidebarOpen ? 'Ana menüyü kapat' : 'Ana menüyü aç'}
              aria-expanded={isSidebarOpen}
              aria-controls="main-sidebar"
              className="rounded-lg p-2 text-slate-300 hover:bg-slate-800 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 lg:hidden"
            >
              {isSidebarOpen ? <X className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
            </button>
            <div className="shrink-0 rounded-2xl bg-gradient-to-tr from-indigo-600 to-blue-500 p-2.5 text-white shadow-md">
              <Building2 className="h-6 w-6" />
            </div>
            <h1 className="truncate text-sm font-extrabold tracking-tight text-white sm:text-lg">
              4/D Sürekli İşçi Bordro Programı
            </h1>
          </div>

          {/* Active period selector and backup tools */}
          <div className="flex w-full items-center justify-end gap-2 md:w-auto">
            <div className="flex items-center gap-2 rounded-xl border border-slate-700 bg-slate-800/90 px-2.5 py-1">
              <Calendar className="h-4 w-4 shrink-0 text-indigo-400" />
              <select
                data-testid="active-period-selector"
                value={aktifDonemId}
                onChange={(e) => onSelectDonem(e.target.value)}
                className="cursor-pointer border-none bg-transparent text-xs font-bold text-white focus:ring-0"
              >
                {donemler.length === 0 && (
                  <option value="" className="bg-slate-900 text-white">
                    Henüz Dönem Yok
                  </option>
                )}
                {donemler.map((d) => {
                  const dateRange = d.donemAdi.match(/\(([^)]+)\)/)?.[1] || d.donemAdi;
                  return (
                    <option key={d.id} value={d.id} className="bg-slate-900 text-white">
                      {dateRange}
                    </option>
                  );
                })}
              </select>
            </div>

            <div className="flex items-center gap-1 border-l border-slate-800 pl-2">
              <button
                type="button"
                onClick={onExportBackup}
                aria-label="Yedek İndir"
                title="Yedek İndir (JSON)"
                className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-slate-800 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
              >
                <Download className="h-4 w-4" />
              </button>

              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                aria-label="Yedek Yükle"
                title="Yedek Yükle"
                className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-slate-800 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
              >
                <Upload className="h-4 w-4" />
              </button>
              <input
                type="file"
                ref={fileInputRef}
                onChange={handleFileChange}
                accept=".json"
                className="hidden"
              />

              <button
                type="button"
                onClick={onResetSampleData}
                aria-label="Örnek Veriyi Yeniden Yükle"
                title="Örnek Veriyi Yeniden Yükle"
                className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-slate-800 hover:text-amber-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
              >
                <RotateCcw className="h-4 w-4" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
};
