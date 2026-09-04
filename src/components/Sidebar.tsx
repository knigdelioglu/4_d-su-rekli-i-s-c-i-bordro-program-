/**
 * Primary application navigation for the 4/D Sürekli İşçi Bordro Programı.
 */

import React, { useEffect } from 'react';
import {
  Building,
  Calendar,
  Calculator,
  Clock,
  ChevronDown,
  ChevronRight,
  FileText,
  Gift,
  Layers,
  MoreHorizontal,
  Baby,
  HeartPulse,
  Percent,
  Plus,
  Receipt,
  Scale,
  ShieldAlert,
  Settings,
  Users,
  Users2,
  Wallet,
  type LucideIcon,
} from 'lucide-react';
import type { KesintiTipi, ParametreSection, TabType } from '../types/navigation';

interface SidebarProps {
  activeTab: TabType;
  activeKesintiType: KesintiTipi;
  activeParametreSection: ParametreSection;
  onTabChange: (tab: TabType) => void;
  onKesintiTypeChange: (type: KesintiTipi) => void;
  onParametreSectionChange: (section: ParametreSection) => void;
  isOpen: boolean;
  onClose: () => void;
}

interface NavigationItem {
  id: TabType;
  label: string;
  icon: LucideIcon;
}

interface DeductionNavigationItem {
  id: KesintiTipi;
  label: string;
  testId: string;
  icon: LucideIcon;
}

interface ParametreNavigationItem {
  id: ParametreSection;
  label: string;
  testId: string;
  icon: LucideIcon;
}

const NAVIGATION_ITEMS: readonly NavigationItem[] = [
  { id: 'personel', label: '1. Personel Bilgileri', icon: Users },
  { id: 'puantaj', label: '2. Puantaj Cetveli', icon: Clock },
  { id: 'bordro', label: '3. Bordro Hesaplama', icon: Calculator },
  { id: 'banka', label: '4. Banka Listesi', icon: Building },
];

const DEDUCTION_NAVIGATION_ITEMS: readonly DeductionNavigationItem[] = [
  { id: 'sendika', label: 'Sendika Aidatı', testId: 'nav-kesinti-sendika', icon: Users2 },
  { id: 'bes', label: 'BES Kesintisi', testId: 'nav-kesinti-bes', icon: Wallet },
  { id: 'icra', label: 'İcra Kesintisi', testId: 'nav-kesinti-icra', icon: Scale },
  { id: 'kisiBorcu', label: 'Kişi Borcu', testId: 'nav-kesinti-kisi-borcu', icon: Receipt },
  {
    id: 'dogumAskerlik',
    label: 'Doğum / Askerlik',
    testId: 'nav-kesinti-dogum-askerlik',
    icon: Baby,
  },
  { id: 'hayatSaglik', label: 'Sağlık Sigortası', testId: 'nav-kesinti-saglik', icon: HeartPulse },
  { id: 'digerKesinti', label: 'Diğer Kesintiler', testId: 'nav-kesinti-diger', icon: MoreHorizontal },
];

const PARAMETRE_NAVIGATION_ITEMS: readonly ParametreNavigationItem[] = [
  { id: 'gelir', label: 'Gelir Parametreleri', testId: 'nav-parametre-gelir', icon: Settings },
  {
    id: 'kesinti',
    label: 'Kesinti & Yasal Oranlar',
    testId: 'nav-parametre-kesinti',
    icon: ShieldAlert,
  },
  { id: 'annualTax', label: 'Yıllık GV Tarifesi', testId: 'nav-parametre-gv', icon: Percent },
  { id: 'tediyeTis', label: 'Tediye & TİS', testId: 'nav-parametre-tediye-tis', icon: Gift },
  { id: 'sickLeave', label: 'Rapor / İstirahat', testId: 'nav-parametre-rapor', icon: FileText },
  { id: 'donemler', label: 'Dönemler', testId: 'nav-parametre-donemler', icon: Calendar },
  { id: 'newPeriod', label: 'Yeni Dönem Aç', testId: 'nav-parametre-yeni-donem', icon: Plus },
];

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  activeKesintiType,
  activeParametreSection,
  onTabChange,
  onKesintiTypeChange,
  onParametreSectionChange,
  isOpen,
  onClose,
}) => {
  const [isKesintilerExpanded, setIsKesintilerExpanded] = React.useState(
    activeTab === 'kesintiler'
  );
  const [isParametrelerExpanded, setIsParametrelerExpanded] = React.useState(
    activeTab === 'parametrelar'
  );

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  useEffect(() => {
    if (activeTab === 'kesintiler') setIsKesintilerExpanded(true);
    if (activeTab === 'parametrelar') setIsParametrelerExpanded(true);
  }, [activeTab]);

  return (
    <>
      {isOpen && (
        <div
          aria-hidden="true"
          onClick={onClose}
          className="fixed inset-0 z-40 bg-slate-950/40 lg:hidden"
        />
      )}

      <aside
        id="main-sidebar"
        aria-label="Ana navigasyon"
        className={`fixed inset-y-0 left-0 z-50 w-60 shrink-0 flex-col border-r border-slate-200 bg-white shadow-xl transition-transform duration-200 ease-out lg:static lg:z-auto lg:flex lg:h-auto lg:translate-x-0 lg:shadow-none ${
          isOpen ? 'flex translate-x-0' : 'hidden -translate-x-full'
        }`}
      >
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="flex items-center justify-between border-b border-slate-200 px-4 py-4 lg:hidden">
            <span className="text-xs font-bold uppercase tracking-wider text-slate-500">
              Ana Menü
            </span>
            <button
              type="button"
              onClick={onClose}
              aria-label="Ana menüyü kapat"
              className="rounded-lg p-1.5 text-slate-400 hover:bg-slate-100 hover:text-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
            >
              <span aria-hidden="true">×</span>
            </button>
          </div>

          <nav className="flex-1 space-y-1 overflow-y-auto p-3" aria-label="Ana menü">
            {NAVIGATION_ITEMS.map(({ id, label, icon: Icon }) => {
              const isActive = activeTab === id;
              return (
                <button
                  key={id}
                  type="button"
                  data-testid={`nav-${id}`}
                  onClick={() => {
                    onTabChange(id);
                    onClose();
                  }}
                  aria-current={isActive ? 'page' : undefined}
                  className={`group flex min-h-11 w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-xs font-bold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                    isActive
                      ? 'bg-indigo-600 text-white shadow-md'
                      : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900'
                  }`}
                >
                  <Icon
                    aria-hidden="true"
                    className={`h-4 w-4 shrink-0 ${
                      isActive ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'
                    }`}
                  />
                  <span>{label}</span>
                </button>
              );
            })}

            <div>
              <button
                type="button"
                data-testid="nav-kesintiler"
                onClick={() => {
                  if (activeTab !== 'kesintiler') {
                    onTabChange('kesintiler');
                    onKesintiTypeChange('sendika');
                    setIsKesintilerExpanded(true);
                    return;
                  }
                  setIsKesintilerExpanded((expanded) => !expanded);
                }}
                aria-expanded={isKesintilerExpanded}
                aria-controls="sidebar-deduction-items"
                className={`group flex min-h-11 w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-xs font-bold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                  activeTab === 'kesintiler'
                    ? 'bg-indigo-50 text-indigo-700'
                    : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900'
                }`}
              >
                <Layers
                  aria-hidden="true"
                  className={`h-4 w-4 shrink-0 ${
                    activeTab === 'kesintiler'
                      ? 'text-indigo-600'
                      : 'text-slate-400 group-hover:text-indigo-600'
                  }`}
                />
                <span className="flex-1">5. Kesinti Listeleri</span>
                {isKesintilerExpanded ? (
                  <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0" />
                ) : (
                  <ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0" />
                )}
              </button>

              {isKesintilerExpanded && (
                <div
                  id="sidebar-deduction-items"
                  className="ml-2 space-y-0.5 border-l border-slate-200 pl-2"
                >
                  {DEDUCTION_NAVIGATION_ITEMS.map(({ id, label, testId, icon: Icon }) => {
                    const isActive = activeTab === 'kesintiler' && activeKesintiType === id;
                    return (
                      <button
                        key={id}
                        type="button"
                        data-testid={testId}
                        onClick={() => {
                          onKesintiTypeChange(id);
                          onClose();
                        }}
                        aria-current={isActive ? 'page' : undefined}
                        className={`group flex min-h-10 w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                          isActive
                            ? 'bg-indigo-600 text-white shadow-sm'
                            : 'text-slate-600 hover:bg-indigo-50 hover:text-indigo-700'
                        }`}
                      >
                        <Icon
                          aria-hidden="true"
                          className={`h-3.5 w-3.5 shrink-0 ${
                            isActive ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'
                          }`}
                        />
                        <span>{label}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            <div>
              <button
                type="button"
                data-testid="nav-parametrelar"
                onClick={() => {
                  if (activeTab !== 'parametrelar') {
                    onTabChange('parametrelar');
                    setIsParametrelerExpanded(true);
                    return;
                  }
                  setIsParametrelerExpanded((expanded) => !expanded);
                }}
                aria-expanded={isParametrelerExpanded}
                aria-controls="sidebar-parametre-items"
                className={`group flex min-h-11 w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-xs font-bold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                  activeTab === 'parametrelar'
                    ? 'bg-indigo-50 text-indigo-700'
                    : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900'
                }`}
              >
                <Settings
                  aria-hidden="true"
                  className={`h-4 w-4 shrink-0 ${
                    activeTab === 'parametrelar'
                      ? 'text-indigo-600'
                      : 'text-slate-400 group-hover:text-indigo-600'
                  }`}
                />
                <span className="flex-1">6. Dönem Parametreleri</span>
                {isParametrelerExpanded ? (
                  <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0" />
                ) : (
                  <ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0" />
                )}
              </button>

              {isParametrelerExpanded && (
                <div
                  id="sidebar-parametre-items"
                  className="ml-2 space-y-0.5 border-l border-slate-200 pl-2"
                >
                  {PARAMETRE_NAVIGATION_ITEMS.map(({ id, label, testId, icon: Icon }) => {
                    const isActive = activeTab === 'parametrelar' && activeParametreSection === id;
                    return (
                      <button
                        key={id}
                        type="button"
                        data-testid={testId}
                        onClick={() => {
                          onParametreSectionChange(id);
                          onClose();
                        }}
                        aria-current={isActive ? 'page' : undefined}
                        className={`group flex min-h-10 w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                          isActive
                            ? 'bg-indigo-600 text-white shadow-sm'
                            : 'text-slate-600 hover:bg-indigo-50 hover:text-indigo-700'
                        }`}
                      >
                        <Icon
                          aria-hidden="true"
                          className={`h-3.5 w-3.5 shrink-0 ${
                            isActive ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'
                          }`}
                        />
                        <span>{label}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </nav>
        </div>
      </aside>
    </>
  );
};
