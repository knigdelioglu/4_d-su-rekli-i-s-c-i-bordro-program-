import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import type { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import { BankaListesi } from './BankaListesi';

describe('BankaListesi print recipient names', () => {
  test('keeps long Turkish names intact in the printable payment table and allows wrapping', () => {
    const fullName = 'Ayşegül Çağıl Şenol Özçelik Karaçınar Yılmaz Balcı';
    const person = {
      id: 'worker-long-name',
      ad: 'Ayşegül Çağıl Şenol',
      soyad: 'Özçelik Karaçınar Yılmaz Balcı',
      tcNo: '12345678901',
      iban: 'TR330006100519786457841326',
      grup: '1. Grup',
      unvan: '1. Grup',
    } as Personel;
    const period = {
      id: '2028-06',
      yil: 2028,
      ay: 6,
      baslangicTarihi: '2028-06-15',
      bitisTarihi: '2028-07-14',
      donemAdi: 'Haziran 2028',
      taxYear: 2028,
      taxMonth: 7,
    } as BordroDonemi;
    const payroll = {
      id: 'worker-long-name_2028-06',
      personelId: person.id,
      donemId: period.id,
      accrualId: 'worker-long-name_2028-06',
      accrualType: 'NORMAL',
      paymentDate: '2028-07-14',
      sequence: 0,
      status: 'FINALIZED',
      netOdeme: 1744.65,
    } as unknown as BordroKaydi;

    const markup = renderToStaticMarkup(
      <BankaListesi aktifDonem={period} personeller={[person]} bordrolar={[payroll]} />
    );

    expect(Array.from(fullName).length >= 45).toBe(true);
    expect(markup).toContain(fullName);
    expect(markup).toContain('break-words whitespace-normal [overflow-wrap:anywhere]');
    expect(markup).not.toContain('text-ellipsis');
  });
});
