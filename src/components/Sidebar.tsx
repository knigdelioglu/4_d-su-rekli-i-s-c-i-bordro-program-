/**
 * Primary application navigation for the 4/D Sürekli İşçi Bordro Programı.
 *
 * Navigation only selects a presentation. Payroll calculations remain owned by
 * the shared PayrollEngine and the existing accrual records.
 */

import React, { useEffect } from 'react';
import {
  Baby,
  BarChart3,
  Building,
  Calendar,
  Calculator,
  ChevronDown,
  ChevronRight,
  Clock,
  FileText,
  Gift,
  HardDrive,
  HeartPulse,
  Layers,
  MoreHorizontal,
  Percent,
  Plus,
  Receipt,
  Scale,
  Settings,
  ShieldCheck,
  Users,
  Users2,
  Wallet,
  type LucideIcon,
} from 'lucide-react';
import type {
  KesintiTipi,
  ParametreSection,
  PayrollViewType,
  TabType,
} from '../types/navigation';
import { PAYROLL_VIEW_LABELS } from '../types/navigation';

interface SidebarProps {
  activeTab: TabType;
  activeKesintiType: KesintiTipi;
  activeParametreSection: ParametreSection;
  activePayrollView: PayrollViewType;
  onTabChange: (tab: TabType) => void;
  onKesintiTypeChange: (type: KesintiTipi) => void;
  onParametreSectionChange: (section: ParametreSection) => void;
  onPayrollViewChange: (view: PayrollViewType) => void;
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

interface PayrollNavigationItem {
  id: PayrollViewType;
  label: string;
  testId: string;
  icon: LucideIcon;
}

const NAVIGATION_ITEMS: readonly NavigationItem[] = [
  { id: 'ozet', label: 'Dönem Özeti', icon: BarChart3 },
  { id: 'personel', label: 'Personel', icon: Users },
  { id: 'puantaj', label: 'Puantaj', icon: Clock },
];

const PAYROLL_NAVIGATION_ITEMS: readonly PayrollNavigationItem[] = [
  { id: 'normal', label: PAYROLL_VIEW_LABELS.normal, testId: 'nav-bordro-normal', icon: Calculator },
  { id: 'tediye', label: PAYROLL_VIEW_LABELS.tediye, testId: 'nav-bordro-tediye', icon: Gift },
  { id: 'tis', label: PAYROLL_VIEW_LABELS.tis, testId: 'nav-bordro-tis', icon: Gift },
  { id: 'supplemental', label: PAYROLL_VIEW_LABELS.supplemental, testId: 'nav-bordro-ek-odeme', icon: Receipt },
];

const DEDUCTION_NAVIGATION_ITEMS: readonly DeductionNavigationItem[] = [
  { id: 'sendika', label: 'Sendika', testId: 'nav-kesinti-sendika', icon: Users2 },
  { id: 'bes', label: 'BES / OKS', testId: 'nav-kesinti-bes', icon: Wallet },
  { id: 'icra', label: 'İcra', testId: 'nav-kesinti-icra', icon: Scale },
  { id: 'kisiBorcu', label: 'Kişi Borcu', testId: 'nav-kesinti-kisi-borcu', icon: Receipt },
  { id: 'dogumAskerlik', label: 'Doğum / Askerlik', testId: 'nav-kesinti-dogum-askerlik', icon: Baby },
  { id: 'hayatSaglik', label: 'Sağlık Sigortası', testId: 'nav-kesinti-saglik', icon: HeartPulse },
  { id: 'digerKesinti', label: 'Diğer', testId: 'nav-kesinti-diger', icon: MoreHorizontal },
];

const PARAMETRE_NAVIGATION_ITEMS: readonly ParametreNavigationItem[] = [
  { id: 'gelir', label: 'Ücretler', testId: 'nav-parametre-gelir', icon: Settings },
  { id: 'kesinti', label: 'Vergi & Yasal Oranlar', testId: 'nav-parametre-kesinti', icon: Percent },
  { id: 'annualTax', label: 'Yıllık GV Tarifesi', testId: 'nav-parametre-gv', icon: Percent },
  { id: 'tediyeTis', label: 'TİS / Tediye Takvimi', testId: 'nav-parametre-tediye-tis', icon: Gift },
  { id: 'sickLeave', label: 'Raporlar', testId: 'nav-parametre-rapor', icon: FileText },
  { id: 'donemler', label: 'Dönemler', testId: 'nav-parametre-donemler', icon: Calendar },
  { id: 'newPeriod', label: 'Yeni Dönem Aç', testId: 'nav-parametre-yeni-donem', icon: Plus },
];

function groupButtonClass(isActive: boolean): string {
  return `group flex min-h-11 w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-xs font-bold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
    isActive
      ? 'bg-indigo-50 text-indigo-700'
      : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900'
  }`;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  activeKesintiType,
  activeParametreSection,
  activePayrollView,
  onTabChange,
  onKesintiTypeChange,
  onParametreSectionChange,
  onPayrollViewChange,
  isOpen,
  onClose,
}) => {
  // Keep the lists open on desktop for backwards-compatible direct access to
  // Banka and Kesinti screens. The group can still be collapsed by the user.
  const [isListsExpanded, setIsListsExpanded] = React.useState(true);
  const [isKesintilerExpanded, setIsKesintilerExpanded] = React.useState(
    activeTab === 'kesintiler'
  );
  const [isPayrollExpanded, setIsPayrollExpanded] = React.useState(activeTab === 'bordro');
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
    if (activeTab === 'kesintiler') {
      setIsListsExpanded(true);
      setIsKesintilerExpanded(true);
    }
    if (activeTab === 'banka' || activeTab === 'sgkKontrol') setIsListsExpanded(true);
    if (activeTab === 'bordro') setIsPayrollExpanded(true);
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
        className={`fixed inset-y-0 left-0 z-50 flex w-60 shrink-0 flex-col border-r border-slate-200 bg-white shadow-xl transition-transform duration-200 ease-out lg:static lg:z-auto lg:flex lg:h-auto lg:translate-x-0 lg:shadow-none ${
          isOpen ? 'translate-x-0' : 'hidden -translate-x-full'
        }`}
      >
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="flex items-center justify-between border-b border-slate-200 px-4 py-4 lg:hidden">
            <span className="text-xs font-bold uppercase tracking-wider text-slate-500">Ana Menü</span>
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
            <div className="px-3 pb-1 pt-1 text-[10px] font-bold uppercase tracking-[0.16em] text-slate-400">
              Dönem
            </div>
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

            <div className="px-3 pb-1 pt-4 text-[10px] font-bold uppercase tracking-[0.16em] text-slate-400">
              Bordro
            </div>
            <div>
              <button
                type="button"
                data-testid="nav-bordro"
                onClick={() => {
                  if (activeTab !== 'bordro') {
                    onPayrollViewChange('normal');
                    setIsPayrollExpanded(true);
                    onClose();
                    return;
                  }
                  setIsPayrollExpanded((expanded) => !expanded);
                }}
                aria-expanded={isPayrollExpanded}
                aria-controls="sidebar-payroll-items"
                aria-current={activeTab === 'bordro' ? 'page' : undefined}
                className={groupButtonClass(activeTab === 'bordro')}
              >
                <Calculator
                  aria-hidden="true"
                  className={`h-4 w-4 shrink-0 ${
                    activeTab === 'bordro' ? 'text-indigo-600' : 'text-slate-400 group-hover:text-indigo-600'
                  }`}
                />
                <span className="flex-1">Bordro</span>
                {isPayrollExpanded ? <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0" /> : <ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0" />}
              </button>

              {isPayrollExpanded && (
                <div id="sidebar-payroll-items" className="ml-2 space-y-0.5 border-l border-slate-200 pl-2">
                  {PAYROLL_NAVIGATION_ITEMS.map(({ id, label, testId, icon: Icon }) => {
                    const isActive = activeTab === 'bordro' && activePayrollView === id;
                    return (
                      <button
                        key={id}
                        type="button"
                        data-testid={testId}
                        onClick={() => {
                          onPayrollViewChange(id);
                          onClose();
                        }}
                        aria-current={isActive ? 'page' : undefined}
                        className={`group flex min-h-10 w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                          isActive ? 'bg-indigo-600 text-white shadow-sm' : 'text-slate-600 hover:bg-indigo-50 hover:text-indigo-700'
                        }`}
                      >
                        <Icon aria-hidden="true" className={`h-3.5 w-3.5 shrink-0 ${isActive ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                        <span>{label}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            <div className="px-3 pb-1 pt-4 text-[10px] font-bold uppercase tracking-[0.16em] text-slate-400">
              Listeler
            </div>
            <div>
              <button
                type="button"
                data-testid="nav-listeler"
                onClick={() => {
                  if (activeTab !== 'banka' && activeTab !== 'sgkKontrol' && activeTab !== 'kesintiler') {
                    onTabChange('banka');
                    setIsListsExpanded(true);
                    return;
                  }
                  setIsListsExpanded((expanded) => !expanded);
                }}
                aria-expanded={isListsExpanded}
                aria-controls="sidebar-list-items"
                className={groupButtonClass(activeTab === 'banka' || activeTab === 'sgkKontrol' || activeTab === 'kesintiler')}
              >
                <Layers aria-hidden="true" className={`h-4 w-4 shrink-0 ${activeTab === 'banka' || activeTab === 'sgkKontrol' || activeTab === 'kesintiler' ? 'text-indigo-600' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                <span className="flex-1">Listeler</span>
                {isListsExpanded ? <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0" /> : <ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0" />}
              </button>

              {isListsExpanded && (
                <div id="sidebar-list-items" className="ml-2 space-y-0.5 border-l border-slate-200 pl-2">
                  <button
                    type="button"
                    data-testid="nav-banka"
                    onClick={() => {
                      onTabChange('banka');
                      onClose();
                    }}
                    aria-current={activeTab === 'banka' ? 'page' : undefined}
                    className={`group flex min-h-10 w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                      activeTab === 'banka' ? 'bg-indigo-600 text-white shadow-sm' : 'text-slate-600 hover:bg-indigo-50 hover:text-indigo-700'
                    }`}
                  >
                    <Building aria-hidden="true" className={`h-3.5 w-3.5 shrink-0 ${activeTab === 'banka' ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                    <span>Banka Listesi</span>
                  </button>

                  <button
                    type="button"
                    data-testid="nav-sgk-kontrol"
                    onClick={() => {
                      onTabChange('sgkKontrol');
                      onClose();
                    }}
                    aria-current={activeTab === 'sgkKontrol' ? 'page' : undefined}
                    className={`group flex min-h-10 w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                      activeTab === 'sgkKontrol' ? 'bg-indigo-600 text-white shadow-sm' : 'text-slate-600 hover:bg-indigo-50 hover:text-indigo-700'
                    }`}
                  >
                    <ShieldCheck aria-hidden="true" className={`h-3.5 w-3.5 shrink-0 ${activeTab === 'sgkKontrol' ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                    <span>SGK Prim Kontrolü</span>
                  </button>

                  <button
                    type="button"
                    data-testid="nav-kesintiler"
                    onClick={() => {
                      if (activeTab !== 'kesintiler') {
                        onKesintiTypeChange('sendika');
                        setIsListsExpanded(true);
                        setIsKesintilerExpanded(true);
                        return;
                      }
                      setIsKesintilerExpanded((expanded) => !expanded);
                    }}
                    aria-expanded={isKesintilerExpanded}
                    aria-controls="sidebar-deduction-items"
                    className={groupButtonClass(activeTab === 'kesintiler')}
                  >
                    <Receipt aria-hidden="true" className={`h-4 w-4 shrink-0 ${activeTab === 'kesintiler' ? 'text-indigo-600' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                    <span className="flex-1">Kesinti Listeleri</span>
                    {isKesintilerExpanded ? <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0" /> : <ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0" />}
                  </button>

                  {isKesintilerExpanded && (
                    <div id="sidebar-deduction-items" className="ml-2 space-y-0.5 border-l border-slate-200 pl-2">
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
                            className={`group flex min-h-9 w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                              isActive ? 'bg-indigo-600 text-white shadow-sm' : 'text-slate-600 hover:bg-indigo-50 hover:text-indigo-700'
                            }`}
                          >
                            <Icon aria-hidden="true" className={`h-3.5 w-3.5 shrink-0 ${isActive ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                            <span>{label}</span>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              )}
            </div>

            <div className="px-3 pb-1 pt-4 text-[10px] font-bold uppercase tracking-[0.16em] text-slate-400">
              Ayarlar
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
                className={groupButtonClass(activeTab === 'parametrelar')}
              >
                <Settings aria-hidden="true" className={`h-4 w-4 shrink-0 ${activeTab === 'parametrelar' ? 'text-indigo-600' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                <span className="flex-1">Dönem Ayarları</span>
                {isParametrelerExpanded ? <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0" /> : <ChevronRight aria-hidden="true" className="h-4 w-4 shrink-0" />}
              </button>

              {isParametrelerExpanded && (
                <div id="sidebar-parametre-items" className="ml-2 space-y-0.5 border-l border-slate-200 pl-2">
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
                        className={`group flex min-h-9 w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 ${
                          isActive ? 'bg-indigo-600 text-white shadow-sm' : 'text-slate-600 hover:bg-indigo-50 hover:text-indigo-700'
                        }`}
                      >
                        <Icon aria-hidden="true" className={`h-3.5 w-3.5 shrink-0 ${isActive ? 'text-white' : 'text-slate-400 group-hover:text-indigo-600'}`} />
                        <span>{label}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            <button
              type="button"
              data-testid="nav-veri"
              onClick={() => {
                onTabChange('veri');
                onClose();
              }}
              aria-current={activeTab === 'veri' ? 'page' : undefined}
              className={`mt-4 flex min-h-10 w-full items-center gap-3 rounded-xl border-t border-slate-200 px-3 pt-4 text-left text-xs font-bold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 ${
                activeTab === 'veri' ? 'text-indigo-700' : 'text-slate-600 hover:text-slate-900'
              }`}
            >
              <HardDrive aria-hidden="true" className="h-4 w-4 shrink-0 text-slate-400" />
              <span>Veri / Yedekleme</span>
            </button>
          </nav>
        </div>
      </aside>
    </>
  );
};
