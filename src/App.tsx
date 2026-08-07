/**
 * 4/D Sürekli İşçi Bordro Programı — Main App Component
 */

import React, { useState, useEffect, useCallback } from 'react';
import { Navbar, TabType } from './components/Navbar';
import { PersonelList } from './components/PersonelList';
import { PuantajGrid } from './components/PuantajGrid';
import { BordroHesaplama } from './components/BordroHesaplama';
import { BankaListesi } from './components/Listeler/BankaListesi';
import { KesintiListesi } from './components/Listeler/KesintiListesi';
import { PeriodManagerModal } from './components/PeriodManagerModal';
import {
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
} from './types/payroll';
import { tauriBridge } from './services/tauriBridge';
import { getInitialDataset } from './utils/sampleData';
import { createBordroDonemi, DEFAULT_KURUM_DEGERLERI } from './utils/payrollUtils';

const STORAGE_KEY = '4d_bordro_programi_mvp_v2';

export default function App() {
  const [activeTab, setActiveTab] = useState<TabType>(() => {
    try {
      const saved = localStorage.getItem('4d_bordro_active_tab');
      if (
        saved &&
        ['personel', 'puantaj', 'bordro', 'banka', 'kesintiler'].includes(saved)
      ) {
        return saved as TabType;
      }
    } catch {
      // fallback
    }
    return 'personel';
  });

  useEffect(() => {
    try {
      localStorage.setItem('4d_bordro_active_tab', activeTab);
    } catch {
      // ignore
    }
  }, [activeTab]);
  
  // Data State
  const [donemler, setDonemler] = useState<BordroDonemi[]>([]);
  const [aktifDonemId, setAktifDonemId] = useState<string>('');
  const [personeller, setPersoneller] = useState<Personel[]>([]);
  const [kurumDegerleriMap, setKurumDegerleriMap] = useState<
    Record<string, DönemselKurumDegerleri>
  >({});
  const [puantajlar, setPuantajlar] = useState<PersonelPuantaj[]>([]);
  const [bordrolar, setBordrolar] = useState<BordroKaydi[]>([]);

  // Navigation state
  const [targetPersonelIdForBordro, setTargetPersonelIdForBordro] = useState<
    string | undefined
  >(undefined);

  // Modals state
  const [isPeriodManagerOpen, setIsPeriodManagerOpen] = useState<boolean>(false);

  // Load data from Tauri IPC or fallback
  const loadData = useCallback(async () => {
    if (tauriBridge.isTauriAvailable()) {
      try {
        // 1. Check legacy migration status
        const isMigrated = await tauriBridge.checkLegacyMigrated();
        if (!isMigrated) {
          const legacyStr = localStorage.getItem(STORAGE_KEY);
          if (legacyStr) {
            try {
              await tauriBridge.migrateLegacyPayload(legacyStr);
            } catch (err) {
              console.error('Legacy migration failed:', err);
            }
          }
        }

        // 2. Fetch data from SQLite via Rust IPC
        let fetchedPeriods = await tauriBridge.getPeriods();
        const fetchedPersonnel = await tauriBridge.getPersonnelList();
        const fetchedAttendance = await tauriBridge.getAttendanceList();
        const fetchedPayrolls = await tauriBridge.getPayrollList();
        const fetchedSettings = await tauriBridge.getInstitutionSettings();
        let savedActivePeriodId = await tauriBridge.getAppSetting('active_period_id');

        // If database has no periods yet, initialize standard empty periods
        if (fetchedPeriods.length === 0) {
          const currentYear = new Date().getFullYear();
          for (let m = 1; m <= 8; m++) {
            const d = createBordroDonemi(currentYear, m);
            await tauriBridge.savePeriod(d);
            const defaultSetting: DönemselKurumDegerleri = {
              donemId: d.id,
              ...DEFAULT_KURUM_DEGERLERI,
            };
            await tauriBridge.saveInstitutionSettings(defaultSetting);
          }
          fetchedPeriods = await tauriBridge.getPeriods();
          savedActivePeriodId = `${currentYear}-05`;
          await tauriBridge.setAppSetting('active_period_id', savedActivePeriodId);
        }

        const activeId = savedActivePeriodId || fetchedPeriods[0]?.id || '';

        setDonemler(fetchedPeriods);
        setPersoneller(fetchedPersonnel);
        setPuantajlar(fetchedAttendance);
        setBordrolar(fetchedPayrolls);
        setKurumDegerleriMap(fetchedSettings);
        setAktifDonemId(activeId);
        return;
      } catch (err) {
        console.error('Tauri IPC load error, falling back to local memory:', err);
      }
    }

    // Fallback load for standalone browser / testing
    let loadedDonemler: BordroDonemi[] = [];
    let loadedAktifDonemId = '';
    let loadedPersoneller: Personel[] = [];
    let loadedKurumMap: Record<string, DönemselKurumDegerleri> = {};
    let loadedPuantajlar: PersonelPuantaj[] = [];
    let loadedBordrolar: BordroKaydi[] = [];

    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        loadedDonemler = parsed.donemler || [];
        loadedAktifDonemId = parsed.aktifDonemId || '';
        loadedPersoneller = parsed.personeller || [];
        loadedKurumMap = parsed.kurumDegerleriMap || {};
        loadedPuantajlar = parsed.puantajlar || [];
        loadedBordrolar = parsed.bordrolar || [];
      }
    } catch (err) {
      console.error('Local storage load error:', err);
    }

    if (loadedDonemler.length === 0) {
      const currentYear = new Date().getFullYear();
      for (let m = 1; m <= 8; m++) {
        const d = createBordroDonemi(currentYear, m);
        loadedDonemler.push(d);
        loadedKurumMap[d.id] = { donemId: d.id, ...DEFAULT_KURUM_DEGERLERI };
      }
      loadedAktifDonemId = `${currentYear}-05`;
    }

    setDonemler(loadedDonemler);
    setAktifDonemId(loadedAktifDonemId);
    setPersoneller(loadedPersoneller);
    setKurumDegerleriMap(loadedKurumMap);
    setPuantajlar(loadedPuantajlar);
    setBordrolar(loadedBordrolar);
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Handle active period change
  const handleSelectDonem = async (id: string) => {
    setAktifDonemId(id);
    if (tauriBridge.isTauriAvailable()) {
      try {
        await tauriBridge.setAppSetting('active_period_id', id);
      } catch (err) {
        console.error('Failed to set active_period_id in app_settings:', err);
      }
    }
  };

  const aktifDonem = donemler.find((d) => d.id === aktifDonemId) || donemler[0];

  // Seed sample data explicitly ONLY when user clicks seed button
  const handleResetSampleData = async () => {
    if (
      window.confirm(
        'Tüm mevcut veriler sıfırlanıp örnek 4/D bordro verileri yüklenecek. Emin misiniz?'
      )
    ) {
      const initialData = getInitialDataset();
      if (tauriBridge.isTauriAvailable()) {
        const payloadStr = JSON.stringify(initialData);
        try {
          await tauriBridge.setAppSetting('legacy_migrated', 'false');
          await tauriBridge.migrateLegacyPayload(payloadStr);
          await loadData();
          return;
        } catch (err) {
          console.error('Tauri seed error:', err);
        }
      }
      setDonemler(initialData.donemler);
      setAktifDonemId(initialData.aktifDonemId);
      setPersoneller(initialData.personeller);
      setKurumDegerleriMap(initialData.kurumDegerleriMap);
      setPuantajlar(initialData.puantajlar);
      setBordrolar(initialData.bordrolar);
    }
  };

  // Export JSON backup
  const handleExportBackup = () => {
    const dataObj = {
      donemler,
      aktifDonemId,
      personeller,
      kurumDegerleriMap,
      puantajlar,
      bordrolar,
      exportedAt: new Date().toISOString(),
    };
    const jsonStr = JSON.stringify(dataObj, null, 2);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `4D_Bordro_Yedek_${aktifDonemId}_${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  // Import JSON backup
  const handleImportBackup = async (jsonStr: string) => {
    try {
      if (tauriBridge.isTauriAvailable()) {
        await tauriBridge.setAppSetting('legacy_migrated', 'false');
        await tauriBridge.migrateLegacyPayload(jsonStr);
        await loadData();
        alert('Yedek başarıyla yüklendi!');
        return;
      }
      const parsed = JSON.parse(jsonStr);
      if (parsed.personeller && parsed.donemler) {
        setDonemler(parsed.donemler);
        setAktifDonemId(parsed.aktifDonemId);
        setPersoneller(parsed.personeller);
        setKurumDegerleriMap(parsed.kurumDegerleriMap || {});
        setPuantajlar(parsed.puantajlar || []);
        setBordrolar(parsed.bordrolar || []);
        alert('Yedek başarıyla yüklendi!');
      } else {
        alert('Geçersiz yedek dosyası formatı.');
      }
    } catch (err) {
      alert('Yedek dosyası okunamadı.');
    }
  };

  // Handlers
  const handleSavePersonel = async (newP: Personel) => {
    if (tauriBridge.isTauriAvailable()) {
      try {
        await tauriBridge.savePersonnel(newP);
        setPersoneller(await tauriBridge.getPersonnelList());
        return;
      } catch (err) {
        console.error('Save personnel error:', err);
      }
    }
    const exists = personeller.some((p) => p.id === newP.id);
    if (exists) {
      setPersoneller(personeller.map((p) => (p.id === newP.id ? newP : p)));
    } else {
      setPersoneller([...personeller, newP]);
    }
  };

  const handleDeletePersonel = async (pId: string) => {
    if (tauriBridge.isTauriAvailable()) {
      try {
        await tauriBridge.deletePersonnel(pId);
        setPersoneller(await tauriBridge.getPersonnelList());
        setBordrolar(await tauriBridge.getPayrollList());
        setPuantajlar(await tauriBridge.getAttendanceList());
        return;
      } catch (err) {
        console.error('Delete personnel error:', err);
      }
    }
    setPersoneller(personeller.filter((p) => p.id !== pId));
    setBordrolar(bordrolar.filter((b) => b.personelId !== pId));
    setPuantajlar(puantajlar.filter((pj) => pj.personelId !== pId));
  };

  const handleCreateDonem = async (
    newDonem: BordroDonemi,
    kurumDegerleri: DönemselKurumDegerleri
  ) => {
    if (tauriBridge.isTauriAvailable()) {
      try {
        await tauriBridge.savePeriod(newDonem);
        await tauriBridge.saveInstitutionSettings(kurumDegerleri);
        setDonemler(await tauriBridge.getPeriods());
        setKurumDegerleriMap(await tauriBridge.getInstitutionSettings());
        return;
      } catch (err) {
        console.error('Create donem error:', err);
      }
    }
    setDonemler((prev) => {
      const exists = prev.some((d) => d.id === newDonem.id);
      return exists ? prev : [...prev, newDonem];
    });
    setKurumDegerleriMap((prev) => ({
      ...prev,
      [newDonem.id]: kurumDegerleri,
    }));
  };

  const handleSaveKurumDegerleri = async (kDegerleri: DönemselKurumDegerleri) => {
    if (tauriBridge.isTauriAvailable()) {
      try {
        await tauriBridge.saveInstitutionSettings(kDegerleri);
        setKurumDegerleriMap(await tauriBridge.getInstitutionSettings());
        return;
      } catch (err) {
        console.error('Save institution settings error:', err);
      }
    }
    setKurumDegerleriMap((prev) => ({
      ...prev,
      [kDegerleri.donemId]: kDegerleri,
    }));
  };

  const handleSavePuantaj = async (updatedPuantaj: PersonelPuantaj) => {
    if (tauriBridge.isTauriAvailable()) {
      try {
        await tauriBridge.saveAttendance(updatedPuantaj);
        setPuantajlar(await tauriBridge.getAttendanceList());
        return;
      } catch (err) {
        console.error('Save attendance error:', err);
      }
    }
    setPuantajlar((prev) => {
      const idx = prev.findIndex((pj) => pj.id === updatedPuantaj.id);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = updatedPuantaj;
        return next;
      }
      return [...prev, updatedPuantaj];
    });
  };

  const handleSaveBordro = async (updatedBordro: BordroKaydi) => {
    setBordrolar((prev) => {
      const idx = prev.findIndex((b) => b.id === updatedBordro.id);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = updatedBordro;
        return next;
      }
      return [...prev, updatedBordro];
    });
  };

  const handleSelectPersonelForBordro = (personelId: string) => {
    setTargetPersonelIdForBordro(personelId);
    setActiveTab('bordro');
  };

  if (!aktifDonem) {
    return (
      <div className="min-h-screen bg-slate-100 flex items-center justify-center p-4">
        <div className="text-center font-semibold text-slate-600">Yükleniyor...</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-slate-100 text-slate-900 flex flex-col font-sans">
      {/* Navbar */}
      <Navbar
        activeTab={activeTab}
        onTabChange={(tab) => {
          if (tab === 'parametrelar') {
            setIsPeriodManagerOpen(true);
          } else {
            setActiveTab(tab);
          }
        }}
        donemler={donemler}
        aktifDonemId={aktifDonemId}
        onSelectDonem={handleSelectDonem}
        onOpenPeriodManager={() => setIsPeriodManagerOpen(true)}
        onExportBackup={handleExportBackup}
        onImportBackup={handleImportBackup}
        onResetSampleData={handleResetSampleData}
      />

      {/* Main Content Body */}
      <main className="max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-6 flex-1">
        {activeTab === 'personel' && (
          <PersonelList
            personeller={personeller}
            onSavePersonel={handleSavePersonel}
            onDeletePersonel={handleDeletePersonel}
            onSelectPersonelForBordro={handleSelectPersonelForBordro}
            isPrimiGruplari={kurumDegerleriMap[aktifDonemId]?.isPrimiGruplari}
          />
        )}

        {activeTab === 'puantaj' && (
          <PuantajGrid
            aktifDonem={aktifDonem}
            personeller={personeller}
            puantajlar={puantajlar}
            onSavePuantaj={handleSavePuantaj}
            onSelectPersonelForBordro={handleSelectPersonelForBordro}
          />
        )}

        {activeTab === 'bordro' && (
          <BordroHesaplama
            aktifDonem={aktifDonem}
            donemler={donemler}
            personeller={personeller}
            kurumDegerleriMap={kurumDegerleriMap}
            puantajlar={puantajlar}
            bordrolar={bordrolar}
            onSaveBordro={handleSaveBordro}
            onSavePersonel={handleSavePersonel}
            initialPersonelId={targetPersonelIdForBordro}
            onGoToPuantaj={(personelId) => {
              if (personelId) {
                setTargetPersonelIdForBordro(personelId);
              }
              setActiveTab('puantaj');
            }}
          />
        )}

        {activeTab === 'banka' && (
          <BankaListesi
            aktifDonem={aktifDonem}
            personeller={personeller}
            bordrolar={bordrolar}
          />
        )}

        {activeTab === 'kesintiler' && (
          <KesintiListesi
            aktifDonem={aktifDonem}
            personeller={personeller}
            bordrolar={bordrolar}
          />
        )}
      </main>

      {/* Modals */}
      <PeriodManagerModal
        isOpen={isPeriodManagerOpen}
        onClose={() => setIsPeriodManagerOpen(false)}
        donemler={donemler}
        aktifDonemId={aktifDonemId}
        onSelectDonem={handleSelectDonem}
        onCreateDonem={handleCreateDonem}
        kurumDegerleriMap={kurumDegerleriMap}
        onSaveKurumDegerleri={handleSaveKurumDegerleri}
      />
    </div>
  );
}
