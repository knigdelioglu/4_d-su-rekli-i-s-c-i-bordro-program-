/**
 * 4/D Sürekli İşçi Bordro Programı - Preloaded Sample Dataset
 */

import {
  BordroDonemi,
  DönemselKurumDegerleri,
  AnnualPayrollParameters,
  Personel,
  PersonelPuantaj,
} from '../types/payroll';
import {
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

  // Active period: July in the generated sample year.
  const activeDonem = donemler.find((d) => d.ay === 7) || donemler[donemler.length - 1];

  const puantajlar: PersonelPuantaj[] = [];

  // Örnek veri yalnızca personel, dönem, kurum ayarı ve puantaj yükler.
  // Bordro sonucu Rust motoru tarafından üretilmelidir; reset/import akışı
  // TypeScript hesap sonucu üreterek native veritabanına yazmaz.
  donemler.forEach((donem) => {
    INITIAL_PERSONELLER.forEach((p, idx) => {
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

      puantajlar.push({
        id: puantajId,
        personelId: p.id,
        donemId: donem.id,
        gunler,
      });
    });
  });

  return {
    donemler,
    aktifDonemId: activeDonem.id,
    personeller: INITIAL_PERSONELLER,
    kurumDegerleriMap,
    puantajlar,
    bordrolar: [],
    taxOpenings: [],
    sickLeaveRecords: [],
    annualPayrollParameters: [
      {
        year: currentYear,
        gelirVergisiDilimleri: [
          { limit: 190000, oran: 0.15 },
          { limit: 400000, oran: 0.2 },
          { limit: 1500000, oran: 0.27 },
          { limit: 5300000, oran: 0.35 },
          { limit: 1_000_000_000_000_000, oran: 0.4 },
        ],
      } satisfies AnnualPayrollParameters,
    ],
  };
}
