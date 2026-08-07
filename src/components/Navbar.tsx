/**
 * Navbar component for 4/D Sürekli İşçi Bordro Programı
 */

import React, { useRef } from 'react';
import {
  Building2,
  Calendar,
  Users,
  Clock,
  Calculator,
  Building,
  Layers,
  Settings,
  Download,
  Upload,
  RotateCcw,
} from 'lucide-react';
import { BordroDonemi } from '../types/payroll';

export type TabType =
  | 'personel'
  | 'puantaj'
  | 'bordro'
  | 'banka'
  | 'kesintiler'
  | 'parametrelar';

interface NavbarProps {
  activeTab: TabType;
  onTabChange: (tab: TabType) => void;
  donemler: BordroDonemi[];
  aktifDonemId: string;
  onSelectDonem: (donemId: string) => void;
  onOpenPeriodManager: () => void;
  onExportBackup: () => void;
  onImportBackup: (jsonStr: string) => void;
  onResetSampleData: () => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  activeTab,
  onTabChange,
  donemler,
  aktifDonemId,
  onSelectDonem,
  onOpenPeriodManager,
  onExportBackup,
  onImportBackup,
  onResetSampleData,
}) => {
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
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

  const activeDonem = donemler.find((d) => d.id === aktifDonemId);

  return (
    <header className="bg-slate-900 text-white shadow-lg sticky top-0 z-40">
      {/* Top Bar */}
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-3 flex flex-col md:flex-row items-center justify-between gap-3 border-b border-slate-800">
        {/* Brand */}
        <div className="flex items-center gap-3">
          <div className="p-2.5 bg-gradient-to-tr from-indigo-600 to-blue-500 rounded-2xl shadow-md text-white">
            <Building2 className="w-6 h-6" />
          </div>
          <div>
            <h1 className="font-extrabold text-base sm:text-lg tracking-tight text-white">
              4/D Sürekli İşçi Bordro Programı
            </h1>
          </div>
        </div>

        {/* Period Selector & Top Quick Tools */}
        <div className="flex items-center gap-2 flex-wrap">
          {/* Active Period Dropdown */}
          <div className="flex items-center bg-slate-800/90 border border-slate-700 rounded-xl px-2.5 py-1 gap-2">
            <Calendar className="w-4 h-4 text-indigo-400 shrink-0" />
            <select
              value={aktifDonemId}
              onChange={(e) => onSelectDonem(e.target.value)}
              className="bg-transparent text-xs font-bold text-white border-none focus:ring-0 cursor-pointer"
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

          {/* Backup / Restore / Reset */}
          <div className="flex items-center border-l border-slate-800 pl-2 gap-1">
            <button
              type="button"
              onClick={onExportBackup}
              title="Yedek İndir (JSON)"
              className="p-1.5 hover:bg-slate-800 text-slate-400 hover:text-white rounded-lg transition-colors"
            >
              <Download className="w-4 h-4" />
            </button>

            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              title="Yedek Yükle"
              className="p-1.5 hover:bg-slate-800 text-slate-400 hover:text-white rounded-lg transition-colors"
            >
              <Upload className="w-4 h-4" />
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
              title="Örnek Veriyi Yeniden Yükle"
              className="p-1.5 hover:bg-slate-800 text-slate-400 hover:text-amber-400 rounded-lg transition-colors"
            >
              <RotateCcw className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Main Navigation Tabs */}
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <nav className="flex items-center gap-1 overflow-x-auto py-2 no-scrollbar">
          <button
            type="button"
            onClick={() => onTabChange('personel')}
            className={`px-4 py-2 text-xs font-bold rounded-xl transition-all flex items-center gap-2 shrink-0 ${
              activeTab === 'personel'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <Users className="w-4 h-4" />
            <span>1. Personel Bilgileri</span>
          </button>

          <button
            type="button"
            onClick={() => onTabChange('puantaj')}
            className={`px-4 py-2 text-xs font-bold rounded-xl transition-all flex items-center gap-2 shrink-0 ${
              activeTab === 'puantaj'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <Clock className="w-4 h-4" />
            <span>2. Puantaj Cetveli</span>
          </button>

          <button
            type="button"
            onClick={() => onTabChange('bordro')}
            className={`px-4 py-2 text-xs font-bold rounded-xl transition-all flex items-center gap-2 shrink-0 ${
              activeTab === 'bordro'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <Calculator className="w-4 h-4" />
            <span>3. Bordro Hesaplama</span>
          </button>

          <button
            type="button"
            onClick={() => onTabChange('banka')}
            className={`px-4 py-2 text-xs font-bold rounded-xl transition-all flex items-center gap-2 shrink-0 ${
              activeTab === 'banka'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <Building className="w-4 h-4" />
            <span>4. Banka Listesi</span>
          </button>

          <button
            type="button"
            onClick={() => onTabChange('kesintiler')}
            className={`px-4 py-2 text-xs font-bold rounded-xl transition-all flex items-center gap-2 shrink-0 ${
              activeTab === 'kesintiler'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <Layers className="w-4 h-4" />
            <span>5. Kesinti Listeleri</span>
          </button>

          <button
            type="button"
            onClick={() => onTabChange('parametrelar')}
            className={`px-4 py-2 text-xs font-bold rounded-xl transition-all flex items-center gap-2 shrink-0 ${
              activeTab === 'parametrelar'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-300 hover:text-white hover:bg-slate-800'
            }`}
          >
            <Settings className="w-4 h-4" />
            <span>6. Dönem Parametreleri</span>
          </button>
        </nav>
      </div>
    </header>
  );
};
