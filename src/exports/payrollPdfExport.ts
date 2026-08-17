import { BordroDonemi } from '../types/payroll';
import {
  PayrollExportLine,
  PayrollExportModel,
  payrollExportFileStem,
  periodExportFileStem,
} from './payrollExportModel';

const PAGE_WIDTH = 1400;
const PAGE_HEIGHT = 1980;
const PDF_WIDTH = 595.28;
const PDF_HEIGHT = 841.89;

function formatMoney(value: number): string {
  return new Intl.NumberFormat('tr-TR', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value) + ' TL';
}

function safeText(value: string | number | null | undefined): string {
  return value === null || value === undefined ? '—' : String(value);
}

function fitText(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number
): string {
  if (ctx.measureText(text).width <= maxWidth) return text;
  let value = text;
  while (value.length > 3 && ctx.measureText(`${value}…`).width > maxWidth) {
    value = value.slice(0, -1);
  }
  return `${value}…`;
}

function drawText(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  maxWidth?: number,
  align: CanvasTextAlign = 'left'
): void {
  ctx.textAlign = align;
  ctx.fillText(maxWidth ? fitText(ctx, text, maxWidth) : text, x, y);
  ctx.textAlign = 'left';
}

function drawSectionHeader(
  ctx: CanvasRenderingContext2D,
  title: string,
  x: number,
  y: number,
  width: number
): number {
  ctx.fillStyle = '#eef2ff';
  ctx.fillRect(x, y, width, 38);
  ctx.strokeStyle = '#c7d2fe';
  ctx.strokeRect(x, y, width, 38);
  ctx.fillStyle = '#1e293b';
  ctx.font = '700 22px Arial, Segoe UI, sans-serif';
  drawText(ctx, title, x + 14, y + 26, width - 28);
  return y + 48;
}

function drawKeyValue(
  ctx: CanvasRenderingContext2D,
  label: string,
  value: string,
  x: number,
  y: number,
  width: number
): void {
  ctx.fillStyle = '#64748b';
  ctx.font = '600 18px Arial, Segoe UI, sans-serif';
  drawText(ctx, label, x, y, width * 0.43);
  ctx.fillStyle = '#0f172a';
  ctx.font = '700 18px Arial, Segoe UI, sans-serif';
  drawText(ctx, value, x + width * 0.43, y, width * 0.57);
}

function drawMoneyLines(
  ctx: CanvasRenderingContext2D,
  lines: PayrollExportLine[],
  x: number,
  y: number,
  width: number,
  maxRows: number
): number {
  const visible = lines.filter((line) => Math.abs(line.amount) > 0.0001).slice(0, maxRows);
  let cursor = y;
  for (const line of visible) {
    ctx.fillStyle = '#334155';
    ctx.font = '500 17px Arial, Segoe UI, sans-serif';
    drawText(ctx, line.label, x, cursor, width - 190);
    ctx.fillStyle = '#0f172a';
    ctx.font = '700 17px Arial, Segoe UI, sans-serif';
    drawText(ctx, formatMoney(line.amount), x + width, cursor, 180, 'right');
    ctx.strokeStyle = '#e2e8f0';
    ctx.beginPath();
    ctx.moveTo(x, cursor + 10);
    ctx.lineTo(x + width, cursor + 10);
    ctx.stroke();
    cursor += 31;
  }
  if (visible.length === 0) {
    ctx.fillStyle = '#94a3b8';
    ctx.font = '500 17px Arial, Segoe UI, sans-serif';
    drawText(ctx, 'Tutar oluşmadı.', x, cursor);
    cursor += 31;
  }
  return cursor;
}

export function renderPayrollPdfCanvas(model: PayrollExportModel): HTMLCanvasElement {
  if (typeof document === 'undefined') {
    throw new Error('PDF üretimi için tarayıcı/Tauri belge bağlamı gerekli.');
  }
  const canvas = document.createElement('canvas');
  canvas.width = PAGE_WIDTH;
  canvas.height = PAGE_HEIGHT;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('PDF canvas bağlamı oluşturulamadı.');

  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, PAGE_WIDTH, PAGE_HEIGHT);

  const margin = 70;
  const contentWidth = PAGE_WIDTH - margin * 2;
  const columnGap = 46;
  const columnWidth = (contentWidth - columnGap) / 2;

  ctx.fillStyle = '#0f172a';
  ctx.font = '800 34px Arial, Segoe UI, sans-serif';
  drawText(ctx, '4/D SÜREKLİ İŞÇİ ÜCRET PUSULASI', PAGE_WIDTH / 2, 72, contentWidth, 'center');
  ctx.fillStyle = '#475569';
  ctx.font = '600 20px Arial, Segoe UI, sans-serif';
  drawText(
    ctx,
    `${model.periodName}  |  ${model.periodStart} - ${model.periodEnd}  |  Vergi: ${model.taxYear}-${String(model.taxMonth).padStart(2, '0')}`,
    PAGE_WIDTH / 2,
    108,
    contentWidth,
    'center'
  );
  ctx.strokeStyle = '#0f172a';
  ctx.lineWidth = 3;
  ctx.beginPath();
  ctx.moveTo(margin, 132);
  ctx.lineTo(PAGE_WIDTH - margin, 132);
  ctx.stroke();

  let y = 172;
  y = drawSectionHeader(ctx, 'PERSONEL BİLGİLERİ', margin, y, contentWidth);
  const metaLeft = [
    ['T.C. Kimlik No', model.employee.tcNo],
    ['Adı Soyadı', model.employee.fullName],
    ['SGK Sicil No', model.employee.sgkRegistryNo],
    ['İş Primi Grubu', model.employee.group],
  ];
  const metaRight = [
    ['Ünvan', model.employee.title],
    ['Hizmet Yılı', `${model.employee.serviceYears}`],
    ['IBAN', model.employee.iban],
    ['Bordro Durumu', model.status],
  ];
  for (let index = 0; index < 4; index += 1) {
    drawKeyValue(ctx, metaLeft[index][0], metaLeft[index][1], margin + 12, y + index * 32, columnWidth - 12);
    drawKeyValue(
      ctx,
      metaRight[index][0],
      metaRight[index][1],
      margin + columnWidth + columnGap + 12,
      y + index * 32,
      columnWidth - 12
    );
  }
  y += 142;

  y = drawSectionHeader(ctx, 'PUANTAJ ÖZETİ (15-14)', margin, y, contentWidth);
  const attendance = model.attendanceSummary;
  const boxGap = 10;
  const boxWidth = (contentWidth - boxGap * Math.max(0, attendance.length - 1)) / Math.max(1, attendance.length);
  attendance.forEach((item, index) => {
    const x = margin + index * (boxWidth + boxGap);
    ctx.fillStyle = '#f8fafc';
    ctx.fillRect(x, y, boxWidth, 72);
    ctx.strokeStyle = '#cbd5e1';
    ctx.strokeRect(x, y, boxWidth, 72);
    ctx.fillStyle = '#64748b';
    ctx.font = '600 14px Arial, Segoe UI, sans-serif';
    drawText(ctx, `${item.label} (${item.code})`, x + boxWidth / 2, y + 25, boxWidth - 12, 'center');
    ctx.fillStyle = '#0f172a';
    ctx.font = '800 25px Arial, Segoe UI, sans-serif';
    drawText(ctx, safeText(item.count), x + boxWidth / 2, y + 55, boxWidth - 12, 'center');
  });
  y += 98;

  const leftX = margin;
  const rightX = margin + columnWidth + columnGap;
  const incomeHeaderY = y;
  let leftY = drawSectionHeader(ctx, 'GELİRLER', leftX, incomeHeaderY, columnWidth);
  let rightY = drawSectionHeader(ctx, 'KESİNTİLER', rightX, incomeHeaderY, columnWidth);
  leftY = drawMoneyLines(ctx, model.incomes, leftX + 8, leftY + 6, columnWidth - 16, 13);
  rightY = drawMoneyLines(ctx, model.deductions, rightX + 8, rightY + 6, columnWidth - 16, 13);

  ctx.fillStyle = '#eef2ff';
  ctx.fillRect(leftX, leftY + 4, columnWidth, 42);
  ctx.fillStyle = '#1e1b4b';
  ctx.font = '800 19px Arial, Segoe UI, sans-serif';
  drawText(ctx, 'BRÜT GELİR TOPLAMI', leftX + 12, leftY + 31, columnWidth - 210);
  drawText(ctx, formatMoney(model.totals.gross), leftX + columnWidth - 12, leftY + 31, 190, 'right');

  ctx.fillStyle = '#fff7ed';
  ctx.fillRect(rightX, rightY + 4, columnWidth, 42);
  ctx.fillStyle = '#7c2d12';
  drawText(ctx, 'KESİNTİ TOPLAMI', rightX + 12, rightY + 31, columnWidth - 210);
  drawText(ctx, formatMoney(model.totals.deductions), rightX + columnWidth - 12, rightY + 31, 190, 'right');

  y = Math.max(leftY, rightY) + 78;
  let sgkY = drawSectionHeader(ctx, 'SGK / VERGİ DENETİMİ', leftX, y, columnWidth);
  let employerY = drawSectionHeader(ctx, 'KURUM MALİYET BİLGİSİ', rightX, y, columnWidth);
  sgkY = drawMoneyLines(ctx, model.sgkTax, leftX + 8, sgkY + 6, columnWidth - 16, 13);
  employerY = drawMoneyLines(ctx, model.employer, rightX + 8, employerY + 6, columnWidth - 16, 8);

  y = Math.max(sgkY, employerY) + 34;
  ctx.fillStyle = '#0f172a';
  ctx.fillRect(margin, y, contentWidth, 92);
  ctx.fillStyle = '#cbd5e1';
  ctx.font = '700 19px Arial, Segoe UI, sans-serif';
  drawText(ctx, 'NET ÖDEME', margin + 24, y + 37);
  ctx.fillStyle = '#ffffff';
  ctx.font = '900 38px Arial, Segoe UI, sans-serif';
  drawText(ctx, formatMoney(model.totals.net), PAGE_WIDTH - margin - 24, y + 58, 420, 'right');
  y += 116;

  const relevantNotices = model.notices.filter((notice) => notice.severity !== 'SUCCESS').slice(0, 4);
  if (relevantNotices.length > 0 && y < PAGE_HEIGHT - 150) {
    y = drawSectionHeader(ctx, 'BORDRO KONTROL NOTLARI', margin, y, contentWidth);
    ctx.font = '500 16px Arial, Segoe UI, sans-serif';
    relevantNotices.forEach((notice, index) => {
      ctx.fillStyle = notice.severity === 'CRITICAL' ? '#991b1b' : notice.severity === 'WARNING' ? '#92400e' : '#334155';
      drawText(ctx, `• ${notice.title}: ${notice.message}`, margin + 10, y + index * 28, contentWidth - 20);
    });
  }

  ctx.fillStyle = '#64748b';
  ctx.font = '500 14px Arial, Segoe UI, sans-serif';
  drawText(
    ctx,
    `Kaynak bordro güncelleme: ${model.sourceUpdatedAt}  |  Belge authoritative ${model.status} snapshot'tan üretilmiştir.`,
    margin,
    PAGE_HEIGHT - 52,
    contentWidth
  );

  return canvas;
}

function concatBytes(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const result = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}

function ascii(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

async function canvasToJpeg(canvas: HTMLCanvasElement): Promise<Uint8Array> {
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (value) => (value ? resolve(value) : reject(new Error('PDF sayfası JPEG'e dönüştürülemedi.'))),
      'image/jpeg',
      0.93
    );
  });
  return new Uint8Array(await blob.arrayBuffer());
}

export async function canvasesToPdfBlob(canvases: HTMLCanvasElement[]): Promise<Blob> {
  if (canvases.length === 0) throw new Error('PDF için en az bir sayfa gerekli.');
  const images = await Promise.all(canvases.map(canvasToJpeg));
  const objectCount = 2 + canvases.length * 3;
  const offsets = new Array<number>(objectCount + 1).fill(0);
  const chunks: Uint8Array[] = [];
  let byteLength = 0;

  const push = (part: Uint8Array) => {
    chunks.push(part);
    byteLength += part.length;
  };
  const pushAscii = (value: string) => push(ascii(value));
  const beginObject = (id: number) => {
    offsets[id] = byteLength;
    pushAscii(`${id} 0 obj\n`);
  };
  const endObject = () => pushAscii('endobj\n');

  pushAscii('%PDF-1.4\n');
  push(new Uint8Array([0x25, 0xff, 0xff, 0xff, 0xff, 0x0a]));

  beginObject(1);
  pushAscii('<< /Type /Catalog /Pages 2 0 R >>\n');
  endObject();

  const pageIds = canvases.map((_, index) => 3 + index * 3);
  beginObject(2);
  pushAscii(`<< /Type /Pages /Count ${pageIds.length} /Kids [${pageIds.map((id) => `${id} 0 R`).join(' ')}] >>\n`);
  endObject();

  canvases.forEach((canvas, index) => {
    const pageId = 3 + index * 3;
    const imageId = pageId + 1;
    const contentId = pageId + 2;
    const image = images[index];
    const content = `q\n${PDF_WIDTH} 0 0 ${PDF_HEIGHT} 0 0 cm\n/Im0 Do\nQ\n`;
    const contentBytes = ascii(content);

    beginObject(pageId);
    pushAscii(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${PDF_WIDTH} ${PDF_HEIGHT}] /Resources << /XObject << /Im0 ${imageId} 0 R >> >> /Contents ${contentId} 0 R >>\n`
    );
    endObject();

    beginObject(imageId);
    pushAscii(
      `<< /Type /XObject /Subtype /Image /Width ${canvas.width} /Height ${canvas.height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length ${image.length} >>\nstream\n`
    );
    push(image);
    pushAscii('\nendstream\n');
    endObject();

    beginObject(contentId);
    pushAscii(`<< /Length ${contentBytes.length} >>\nstream\n`);
    push(contentBytes);
    pushAscii('endstream\n');
    endObject();
  });

  const xrefOffset = byteLength;
  pushAscii(`xref\n0 ${objectCount + 1}\n`);
  pushAscii('0000000000 65535 f \n');
  for (let id = 1; id <= objectCount; id += 1) {
    pushAscii(`${String(offsets[id]).padStart(10, '0')} 00000 n \n`);
  }
  pushAscii(`trailer\n<< /Size ${objectCount + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`);

  return new Blob([concatBytes(chunks)], { type: 'application/pdf' });
}

function downloadBlob(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.style.display = 'none';
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export async function exportSinglePayrollPdf(model: PayrollExportModel): Promise<void> {
  const blob = await canvasesToPdfBlob([renderPayrollPdfCanvas(model)]);
  downloadBlob(blob, `${payrollExportFileStem(model)}.pdf`);
}

export async function exportPeriodPayrollPdf(
  period: BordroDonemi,
  models: PayrollExportModel[]
): Promise<void> {
  if (models.length === 0) {
    throw new Error('Bu dönem için CALCULATED veya FINALIZED bordro bulunamadı.');
  }
  const canvases = models.map(renderPayrollPdfCanvas);
  const blob = await canvasesToPdfBlob(canvases);
  downloadBlob(blob, `${periodExportFileStem(period)}_Ucret_Pusulalari.pdf`);
}
