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
