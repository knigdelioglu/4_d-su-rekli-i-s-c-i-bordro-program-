/**
 * Primary application navigation for the 4/D Sürekli İşçi Bordro Programı.
 */

import React, { useEffect } from 'react';
import {
  Building,
  Calculator,
  Clock,
  Layers,
  Settings,
  Users,
  type LucideIcon,
} from 'lucide-react';

export type TabType =
  | 'personel'
  | 'puantaj'
  | 'bordro'
  | 'banka'
  | 'kesintiler'
  | 'parametrelar';

interface SidebarProps {
  activeTab: TabType;
  onTabChange: (tab: TabType) => void;
  isOpen: boolean;
  onClose: () => void;
}

interface NavigationItem {
  id: TabType;
  label: string;
  icon: LucideIcon;
}

const NAVIGATION_ITEMS: readonly NavigationItem[] = [
  { id: 'personel', label: '1. Personel Bilgileri', icon: Users },
  { id: 'puantaj', label: '2. Puantaj Cetveli', icon: Clock },
  { id: 'bordro', label: '3. Bordro Hesaplama', icon: Calculator },
  { id: 'banka', label: '4. Banka Listesi', icon: Building },
  { id: 'kesintiler', label: '5. Kesinti Listeleri', icon: Layers },
  { id: 'parametrelar', label: '6. Dönem Parametreleri', icon: Settings },
];

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  onTabChange,
  isOpen,
  onClose,
}) => {
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

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
          </nav>
        </div>
      </aside>
    </>
  );
};
