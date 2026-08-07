/**
 * 4/D Sürekli İşçi Bordro Programı - Preloaded Sample Dataset
 */

import {
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
} from '../types/payroll';
import {
  autoFillGelirlerFromPuantaj,
  calculateGelirToplam,
  calculateKesintiToplam,
  calculateNetOdeme,
  calculatePuantajOzeti,
  calculateStatutoryDeductions,
  createBordroDonemi,
  DEFAULT_KURUM_DEGERLERI,
  generateDefaultPuantajGunler,
} from './payrollUtils';

export const INITIAL_PERSONELLER: Personel[] = [
  {
    id: 'p-1',
    tcNo: '12345678901',
    ad: 'Ahmet',
    soyad: 'Yılmaz',
    grup: '1. Grup',
    unvan: '1. Grup (%9 İş Primi)',
    sgkSicilNo: '48201938201',
    iban: 'TR120006200000012345678901',
    hizmetYili: 5,
    aciklama: 'Destek Hizmetleri Birimi',
    kesintiler: {
      sendikaUyesi: true,
      besUyesi: true,
    },
  },
  {
    id: 'p-2',
    tcNo: '98765432109',
    ad: 'Ayşe',
    soyad: 'Kaya',
    grup: '2. Grup',
    unvan: '2. Grup (%8 İş Primi)',
    sgkSicilNo: '48201938202',
    iban: 'TR340006200000098765432109',
    hizmetYili: 3,
    aciklama: 'A Blok Nöbet Birimi',
    kesintiler: {
      sendikaUyesi: true,
      besUyesi: true,
      icraTutar: 2500,
    },
  },
  {
    id: 'p-3',
    tcNo: '45678912304',
    ad: 'Mehmet',
    soyad: 'Demir',
    grup: '3. Grup',
    unvan: '3. Grup (%7 İş Primi)',
    sgkSicilNo: '48201938203',
    iban: 'TR620006200000045678912304',
    hizmetYili: 8,
    aciklama: 'Bilgi İşlem Merkezi',
    kesintiler: {
      sendikaUyesi: true,
      besUyesi: false,
    },
  },
  {
    id: 'p-4',
    tcNo: '32165498705',
    ad: 'Fatma',
    soyad: 'Şahin',
    grup: '2. Grup',
    unvan: '2. Grup (%8 İş Primi)',
    sgkSicilNo: '48201938204',
    iban: 'TR880006200000032165498705',
    hizmetYili: 10,
    aciklama: 'Elektrik & İklimlendirme Atölyesi',
    kesintiler: {
      sendikaUyesi: true,
      besUyesi: true,
      kisiBorcuTutar: 1200,
      hayatSaglikSigortasiTutar: 450,
    },
  },
  {
    id: 'p-5',
    tcNo: '65498732108',
    ad: 'Mustafa',
    soyad: 'Çelik',
    grup: '3. Grup',
    unvan: '3. Grup (%7 İş Primi)',
    sgkSicilNo: '48201938205',
    iban: 'TR990006200000065498732108',
    hizmetYili: 2,
    aciklama: 'Yemekhane Hizmetleri',
    kesintiler: {
      sendikaUyesi: false,
      besUyesi: true,
      dogumAskerlikBorclanmasiTutar: 600,
    },
  },
];

export function getInitialDataset() {
  const currentYear = new Date().getFullYear();
  
  // Create periods from January (1) to August (8) for the current year
  const donemler: BordroDonemi[] = [];
  const kurumDegerleriMap: Record<string, DönemselKurumDegerleri> = {};

  for (let m = 1; m <= 8; m++) {
    const d = createBordroDonemi(currentYear, m);
    donemler.push(d);
    kurumDegerleriMap[d.id] = {
      donemId: d.id,
      ...DEFAULT_KURUM_DEGERLERI,
    };
  }

  // Active period: Temmuz 2026 (15 Temmuz - 14 Ağustos)
  const activeDonem = donemler.find((d) => d.ay === 7) || donemler[donemler.length - 1];

  const puantajlar: PersonelPuantaj[] = [];
  const bordrolar: BordroKaydi[] = [];

  // Track cumulative tax bases and devreden PEK across months for sample data
  const kumulatifGvMap: Record<string, number> = {};
  const kumulatifAsgariGvMap: Record<string, number> = {};
  const devredenPekMap: Record<string, any[]> = {};

  // Populate puantaj and bordro for all periods, particularly August
  donemler.forEach((donem) => {
    const kDegerleri = kurumDegerleriMap[donem.id];

    INITIAL_PERSONELLER.forEach((p, idx) => {
      if (!kumulatifGvMap[p.id]) kumulatifGvMap[p.id] = 0;
      if (!kumulatifAsgariGvMap[p.id]) kumulatifAsgariGvMap[p.id] = 0;
      if (!devredenPekMap[p.id]) devredenPekMap[p.id] = [];

      // Generate default puantaj days
      const gunler = generateDefaultPuantajGunler(
        donem.baslangicTarihi,
        donem.bitisTarihi
      );

      // Variations
      if (idx === 1) {
        // Ayşe Kaya night shift
        const keys = Object.keys(gunler);
        if (keys[2]) gunler[keys[2]] = 'GÇ';
        if (keys[5]) gunler[keys[5]] = 'GÇ';
        if (keys[8]) gunler[keys[8]] = 'GÇ';
      } else if (idx === 3) {
        // Fatma Şahin report
        const keys = Object.keys(gunler);
        if (keys[10]) gunler[keys[10]] = 'R';
        if (keys[11]) gunler[keys[11]] = 'R';
      }

      const puantajId = `${p.id}_${donem.id}`;
      const puantajOzeti = calculatePuantajOzeti(gunler);

      puantajlar.push({
        id: puantajId,
        personelId: p.id,
        donemId: donem.id,
        gunler,
      });

      // Auto calculate income
      const gelirler = autoFillGelirlerFromPuantaj(
        puantajOzeti,
        kDegerleri,
        p.hizmetYili,
        p.grup
      );

      // Current cumulative bases and incoming PEK carryover before this month
      const prevGvMatrah = kumulatifGvMap[p.id];
      const prevAsgariGvMatrah = kumulatifAsgariGvMap[p.id];
      const devredenPekGelen = devredenPekMap[p.id] || [];

      // Auto calculate deductions using actual cumulative tax bases and devreden PEK
      const statutory = calculateStatutoryDeductions(
        gelirler,
        kDegerleri,
        p,
        puantajOzeti,
        prevGvMatrah,
        devredenPekGelen,
        prevAsgariGvMatrah
      );

      const kesintiler = {
        isciSgkPrimi: statutory.isciSgkPrimi ?? null,
        isciIssizlikPrimi: statutory.isciIssizlikPrimi ?? null,
        gelirVergisi: statutory.gelirVergisi ?? null,
        damgaVergisi: statutory.damgaVergisi ?? null,
        sendikaAidati: statutory.sendikaAidati ?? null,
        bes: statutory.bes ?? null,
        icra: statutory.icra ?? null,
        kisiBorcu: statutory.kisiBorcu ?? null,
        dogumAskerlikBorclanmasi: statutory.dogumAskerlikBorclanmasi ?? null,
        hayatSaglikSigortasi: statutory.hayatSaglikSigortasi ?? null,
        digerKesinti: statutory.digerKesinti ?? null,
      };

      const gelirToplam = calculateGelirToplam(gelirler);
      const kesintiToplam = calculateKesintiToplam(kesintiler);
      const netOdeme = calculateNetOdeme(gelirToplam, kesintiToplam);

      // Update cumulative tax bases for next month
      const isciSgk = kesintiler.isciSgkPrimi || 0;
      const isciIssizlik = kesintiler.isciIssizlikPrimi || 0;
      const curGvMatrah = Math.max(0, gelirToplam - isciSgk - isciIssizlik);
      kumulatifGvMap[p.id] += curGvMatrah;

      const gunlukAsgariUcret = kDegerleri.gunlukAsgariUcret ?? 1101.00;
      const primGun = Math.min(
        30,
        (puantajOzeti.Ç || 0) +
          (puantajOzeti.T || 0) +
          (puantajOzeti.G || 0) +
          (puantajOzeti.İ || 0) +
          (puantajOzeti.R || 0)
      );
      const asgariBrut = gunlukAsgariUcret * primGun;
      const asgariSgk = asgariBrut * 0.15;
      const curAsgariGvMatrah = Math.max(0, asgariBrut - asgariSgk);
      kumulatifAsgariGvMap[p.id] += curAsgariGvMatrah;

      // Update PEK carryover list for next month
      devredenPekMap[p.id] = statutory.pekResult?.sonrakiDevredenList || [];

      bordrolar.push({
        id: `${p.id}_${donem.id}`,
        personelId: p.id,
        donemId: donem.id,
        puantajOzeti,
        gelirler,
        gelirToplam,
        kesintiler,
        kesintiToplam,
        netOdeme,
        olusturulmaTarihi: new Date().toISOString(),
        sonGuncellemeTarihi: new Date().toISOString(),
        notlar: `${donem.donemAdi} bordro kaydı.`,
        devredenPekGelen,
        sonrakiDevredenPek: statutory.pekResult?.sonrakiDevredenList || [],
        pekDetay: statutory.pekResult
          ? {
              hesaplananPek: statutory.pekResult.hesaplananPek,
              finalPek: statutory.pekResult.finalPek,
              devredenPekAşanTutar: statutory.pekResult.devredenPekAşanTutar,
              pekAltSinir: statutory.pekResult.pekAltSinir,
              pekUstSinir: statutory.pekResult.pekUstSinir,
              fiiliYemekGunu: statutory.pekResult.fiiliYemekGunu,
              yemekIstisnasiTutar: statutory.pekResult.yemekIstisnasiTutar,
            }
          : undefined,
      });
    });
  });

  return {
    donemler,
    aktifDonemId: activeDonem.id,
    personeller: INITIAL_PERSONELLER,
    kurumDegerleriMap,
    puantajlar,
    bordrolar,
  };
}
