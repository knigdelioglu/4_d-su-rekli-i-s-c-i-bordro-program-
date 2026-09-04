export type TabType =
  | 'personel'
  | 'puantaj'
  | 'bordro'
  | 'banka'
  | 'kesintiler'
  | 'parametrelar';

export const TAB_TYPES: readonly TabType[] = [
  'personel',
  'puantaj',
  'bordro',
  'banka',
  'kesintiler',
  'parametrelar',
];

export function isTabType(value: string | null): value is TabType {
  return value !== null && TAB_TYPES.some((type) => type === value);
}

export type KesintiTipi =
  | 'sendika'
  | 'bes'
  | 'icra'
  | 'kisiBorcu'
  | 'dogumAskerlik'
  | 'hayatSaglik'
  | 'digerKesinti';

export const KESINTI_TIPLERI: readonly KesintiTipi[] = [
  'sendika',
  'bes',
  'icra',
  'kisiBorcu',
  'dogumAskerlik',
  'hayatSaglik',
  'digerKesinti',
];

export function isKesintiTipi(value: string | null): value is KesintiTipi {
  return value !== null && KESINTI_TIPLERI.some((type) => type === value);
}

export type ParametreSection =
  | 'gelir'
  | 'kesinti'
  | 'annualTax'
  | 'tediyeTis'
  | 'sickLeave'
  | 'donemler'
  | 'newPeriod';

export const PARAMETRE_SECTIONS: readonly ParametreSection[] = [
  'gelir',
  'kesinti',
  'annualTax',
  'tediyeTis',
  'sickLeave',
  'donemler',
  'newPeriod',
];

export function isParametreSection(value: string | null): value is ParametreSection {
  return value !== null && PARAMETRE_SECTIONS.some((section) => section === value);
}
