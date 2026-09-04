import { BordroDonemi } from '../types/payroll';
import {
  PayrollExportLine,
  PayrollExportModel,
  payrollExportFileStem,
  periodExportFileStem,
} from './payrollExportModel';

const CANVAS_WIDTH = 1240;
const CANVAS_HEIGHT = 1754;
const PDF_WIDTH = 595.28;
const PDF_HEIGHT = 841.89;

function money(value: number): string {
  return `${new Intl.NumberFormat('tr-TR', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value)} TL`;
}

function clippedText(
  ctx: CanvasRenderingContext2D,
  value: string,
  maxWidth: number
): string {
  if (ctx.measureText(value).width <= maxWidth) return value;
  let result = value;
  while (result.length > 2 && ctx.measureText(`${result}…`).width > maxWidth) {
    result = result.slice(0, -1);
  }
  return `${result}…`;
}

function text(
  ctx: CanvasRenderingContext2D,
  value: string,
  x: number,
  y: number,
  maxWidth: number,
  align: CanvasTextAlign = 'left'
): void {
  ctx.textAlign = align;
  ctx.fillText(clippedText(ctx, value, maxWidth), x, y);
  ctx.textAlign = 'left';
}

function sectionTitle(
  ctx: CanvasRenderingContext2D,
  title: string,
  x: number,
  y: number,
  width: number
): number {
  ctx.fillStyle = '#eef2ff';
  ctx.fillRect(x, y, width, 34);
  ctx.strokeStyle = '#c7d2fe';
  ctx.strokeRect(x, y, width, 34);
  ctx.fillStyle = '#1e293b';
  ctx.font = '700 19px Arial, Segoe UI, sans-serif';
  text(ctx, title, x + 12, y + 23, width - 24);
  return y + 44;
}

function moneyRows(
  ctx: CanvasRenderingContext2D,
  lines: PayrollExportLine[],
  x: number,
  y: number,
  width: number,
  maxRows: number
): number {
  const visible = lines.filter((line) => Math.abs(line.amount) > 0.0001).slice(0, maxRows);
  let cursor = y;
  if (visible.length === 0) {
    ctx.fillStyle = '#64748b';
    ctx.font = '500 15px Arial, Segoe UI, sans-serif';
    text(ctx, 'Tutar oluşmadı.', x, cursor, width);
    return cursor + 28;
  }
  for (const line of visible) {
    ctx.fillStyle = '#475569';
    ctx.font = '500 15px Arial, Segoe UI, sans-serif';
    text(ctx, line.label, x, cursor, width - 170);
    ctx.fillStyle = '#0f172a';
    ctx.font = '700 15px Arial, Segoe UI, sans-serif';
    text(ctx, money(line.amount), x + width, cursor, 160, 'right');
    ctx.strokeStyle = '#e2e8f0';
    ctx.beginPath();
    ctx.moveTo(x, cursor + 8);
    ctx.lineTo(x + width, cursor + 8);
    ctx.stroke();
    cursor += 26;
  }
  return cursor;
}

function keyValue(
  ctx: CanvasRenderingContext2D,
  label: string,
  value: string,
  x: number,
  y: number,
  width: number
): void {
  ctx.fillStyle = '#64748b';
  ctx.font = '600 15px Arial, Segoe UI, sans-serif';
  text(ctx, label, x, y, width * 0.4);
  ctx.fillStyle = '#0f172a';
  ctx.font = '700 15px Arial, Segoe UI, sans-serif';
  text(ctx, value, x + width * 0.4, y, width * 0.6);
}

export function renderPayrollPdfCanvas(model: PayrollExportModel): HTMLCanvasElement {
  if (typeof document === 'undefined') {
    throw new Error('PDF üretimi için tarayıcı/Tauri belge bağlamı gerekli.');
  }
  const canvas = document.createElement('canvas');
  canvas.width = CANVAS_WIDTH;
  canvas.height = CANVAS_HEIGHT;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('PDF canvas bağlamı oluşturulamadı.');

  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
  const margin = 55;
  const contentWidth = CANVAS_WIDTH - margin * 2;
  const gap = 36;
  const columnWidth = (contentWidth - gap) / 2;

  ctx.fillStyle = '#0f172a';
  ctx.font = '800 29px Arial, Segoe UI, sans-serif';
  text(ctx, '4/D SÜREKLİ İŞÇİ ÜCRET PUSULASI', CANVAS_WIDTH / 2, 58, contentWidth, 'center');
  ctx.fillStyle = '#475569';
  ctx.font = '600 16px Arial, Segoe UI, sans-serif';
  text(
    ctx,
    `${model.periodName} | ${model.periodStart} - ${model.periodEnd} | Vergi ${model.taxYear}-${String(model.taxMonth).padStart(2, '0')}`,
    CANVAS_WIDTH / 2,
    88,
    contentWidth,
    'center'
  );
  ctx.strokeStyle = '#0f172a';
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(margin, 108);
  ctx.lineTo(CANVAS_WIDTH - margin, 108);
  ctx.stroke();

  let y = 136;
  y = sectionTitle(ctx, 'PERSONEL BİLGİLERİ', margin, y, contentWidth);
  const leftMeta = [
    ['T.C. Kimlik No', model.employee.tcNo],
    ['Adı Soyadı', model.employee.fullName],
    ['SGK Sicil No', model.employee.sgkRegistryNo],
    ['İş Primi Grubu', model.employee.group],
  ];
  const rightMeta = [
    ['Ünvan', model.employee.title],
    ['Hizmet Yılı', String(model.employee.serviceYears)],
    ['IBAN', model.employee.iban],
    ['Bordro Durumu', model.status],
  ];
  for (let i = 0; i < 4; i += 1) {
    keyValue(ctx, leftMeta[i][0], leftMeta[i][1], margin + 8, y + i * 26, columnWidth - 8);
    keyValue(ctx, rightMeta[i][0], rightMeta[i][1], margin + columnWidth + gap + 8, y + i * 26, columnWidth - 8);
  }
  keyValue(ctx, 'Tahakkuk Türü', model.accrualType, margin + 8, y + 4 * 26, columnWidth - 8);
  keyValue(
    ctx,
    'Tahakkuk Tarihi',
    model.paymentDate,
    margin + columnWidth + gap + 8,
    y + 4 * 26,
    columnWidth - 8
  );
  keyValue(ctx, 'Tahakkuk Sıra No', String(model.sequence), margin + 8, y + 5 * 26, columnWidth - 8);
  keyValue(
    ctx,
    'Tahakkuk Açıklaması',
    model.accrualDescription || '—',
    margin + columnWidth + gap + 8,
    y + 5 * 26,
    columnWidth - 8
  );
  y += 168;

  y = sectionTitle(ctx, 'PUANTAJ ÖZETİ (15-14)', margin, y, contentWidth);
  const attendance = model.attendanceSummary;
  const boxGap = 7;
  const boxWidth = (contentWidth - boxGap * Math.max(0, attendance.length - 1)) / Math.max(1, attendance.length);
  attendance.forEach((item, index) => {
    const x = margin + index * (boxWidth + boxGap);
    ctx.fillStyle = '#f8fafc';
    ctx.fillRect(x, y, boxWidth, 62);
    ctx.strokeStyle = '#cbd5e1';
    ctx.strokeRect(x, y, boxWidth, 62);
    ctx.fillStyle = '#64748b';
    ctx.font = '600 12px Arial, Segoe UI, sans-serif';
    text(ctx, `${item.label} (${item.code})`, x + boxWidth / 2, y + 22, boxWidth - 10, 'center');
    ctx.fillStyle = '#0f172a';
    ctx.font = '800 22px Arial, Segoe UI, sans-serif';
    text(ctx, String(item.count), x + boxWidth / 2, y + 49, boxWidth - 10, 'center');
  });
  y += 84;

  const leftX = margin;
  const rightX = margin + columnWidth + gap;
  let leftY = sectionTitle(ctx, 'GELİRLER', leftX, y, columnWidth);
  let rightY = sectionTitle(ctx, 'KESİNTİLER', rightX, y, columnWidth);
  leftY = moneyRows(ctx, model.incomes, leftX + 6, leftY + 4, columnWidth - 12, 13);
  rightY = moneyRows(ctx, model.deductions, rightX + 6, rightY + 4, columnWidth - 12, 13);

  ctx.fillStyle = '#eef2ff';
  ctx.fillRect(leftX, leftY + 2, columnWidth, 36);
  ctx.fillStyle = '#1e1b4b';
  ctx.font = '800 16px Arial, Segoe UI, sans-serif';
  text(ctx, 'BRÜT GELİR TOPLAMI', leftX + 10, leftY + 26, columnWidth - 175);
  text(ctx, money(model.totals.gross), leftX + columnWidth - 10, leftY + 26, 165, 'right');

  ctx.fillStyle = '#fff7ed';
  ctx.fillRect(rightX, rightY + 2, columnWidth, 36);
  ctx.fillStyle = '#7c2d12';
  text(ctx, 'KESİNTİ TOPLAMI', rightX + 10, rightY + 26, columnWidth - 175);
  text(ctx, money(model.totals.deductions), rightX + columnWidth - 10, rightY + 26, 165, 'right');

  y = Math.max(leftY, rightY) + 58;
  let sgkY = sectionTitle(ctx, 'SGK / VERGİ DENETİMİ', leftX, y, columnWidth);
  let employerY = sectionTitle(ctx, 'KURUM MALİYET BİLGİSİ', rightX, y, columnWidth);
  sgkY = moneyRows(ctx, model.sgkTax, leftX + 6, sgkY + 4, columnWidth - 12, 13);
  employerY = moneyRows(ctx, model.employer, rightX + 6, employerY + 4, columnWidth - 12, 8);

  y = Math.max(sgkY, employerY) + 20;
  ctx.fillStyle = '#0f172a';
  ctx.fillRect(margin, y, contentWidth, 76);
  ctx.fillStyle = '#cbd5e1';
  ctx.font = '700 16px Arial, Segoe UI, sans-serif';
  text(ctx, 'NET ÖDEME', margin + 18, y + 31, 220);
  ctx.fillStyle = '#ffffff';
  ctx.font = '900 32px Arial, Segoe UI, sans-serif';
  text(ctx, money(model.totals.net), CANVAS_WIDTH - margin - 18, y + 49, 380, 'right');
  y += 96;

  const notices = model.notices.filter((notice) => notice.severity !== 'SUCCESS').slice(0, 3);
  if (notices.length > 0 && y < CANVAS_HEIGHT - 135) {
    y = sectionTitle(ctx, 'BORDRO KONTROL NOTLARI', margin, y, contentWidth);
    ctx.font = '500 13px Arial, Segoe UI, sans-serif';
    notices.forEach((notice, index) => {
      ctx.fillStyle = notice.severity === 'CRITICAL' ? '#991b1b' : notice.severity === 'WARNING' ? '#92400e' : '#334155';
      text(ctx, `• ${notice.title}: ${notice.message}`, margin + 8, y + index * 24, contentWidth - 16);
    });
  }

  ctx.fillStyle = '#64748b';
  ctx.font = '500 12px Arial, Segoe UI, sans-serif';
  text(
    ctx,
    `Kaynak güncelleme: ${model.sourceUpdatedAt} | Belge ${model.status} bordro snapshot'ından üretilmiştir.`,
    margin,
    CANVAS_HEIGHT - 38,
    contentWidth
  );
  return canvas;
}

function ascii(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

function joinBytes(parts: Uint8Array[]): Uint8Array {
  const size = parts.reduce((sum, part) => sum + part.length, 0);
  const output = new Uint8Array(size);
  let cursor = 0;
  for (const part of parts) {
    output.set(part, cursor);
    cursor += part.length;
  }
  return output;
}

async function canvasJpeg(canvas: HTMLCanvasElement): Promise<Uint8Array> {
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (value) => (value ? resolve(value) : reject(new Error("PDF sayfası JPEG'e dönüştürülemedi."))),
      'image/jpeg',
      0.93
    );
  });
  return new Uint8Array(await blob.arrayBuffer());
}

export async function canvasesToPdfBlob(canvases: HTMLCanvasElement[]): Promise<Blob> {
  if (canvases.length === 0) throw new Error('PDF için en az bir sayfa gerekli.');
  const images = await Promise.all(canvases.map(canvasJpeg));
  const objectCount = 2 + canvases.length * 3;
  const offsets = new Array<number>(objectCount + 1).fill(0);
  const parts: Uint8Array[] = [];
  let length = 0;

  const push = (part: Uint8Array) => {
    parts.push(part);
    length += part.length;
  };
  const pushText = (value: string) => push(ascii(value));
  const begin = (id: number) => {
    offsets[id] = length;
    pushText(`${id} 0 obj\n`);
  };
  const end = () => pushText('endobj\n');

  pushText('%PDF-1.4\n');
  push(new Uint8Array([0x25, 0xff, 0xff, 0xff, 0xff, 0x0a]));

  begin(1);
  pushText('<< /Type /Catalog /Pages 2 0 R >>\n');
  end();

  const pageIds = canvases.map((_, index) => 3 + index * 3);
  begin(2);
  pushText(`<< /Type /Pages /Count ${pageIds.length} /Kids [${pageIds.map((id) => `${id} 0 R`).join(' ')}] >>\n`);
  end();

  canvases.forEach((canvas, index) => {
    const pageId = 3 + index * 3;
    const imageId = pageId + 1;
    const contentId = pageId + 2;
    const image = images[index];
    const content = ascii(`q\n${PDF_WIDTH} 0 0 ${PDF_HEIGHT} 0 0 cm\n/Im0 Do\nQ\n`);

    begin(pageId);
    pushText(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${PDF_WIDTH} ${PDF_HEIGHT}] /Resources << /XObject << /Im0 ${imageId} 0 R >> >> /Contents ${contentId} 0 R >>\n`);
    end();

    begin(imageId);
    pushText(`<< /Type /XObject /Subtype /Image /Width ${canvas.width} /Height ${canvas.height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length ${image.length} >>\nstream\n`);
    push(image);
    pushText('\nendstream\n');
    end();

    begin(contentId);
    pushText(`<< /Length ${content.length} >>\nstream\n`);
    push(content);
    pushText('endstream\n');
    end();
  });

  const xrefOffset = length;
  pushText(`xref\n0 ${objectCount + 1}\n`);
  pushText('0000000000 65535 f \n');
  for (let id = 1; id <= objectCount; id += 1) {
    pushText(`${String(offsets[id]).padStart(10, '0')} 00000 n \n`);
  }
  pushText(`trailer\n<< /Size ${objectCount + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`);

  const bytes = joinBytes(parts);
  return new Blob([bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength)], {
    type: 'application/pdf',
  });
}

function download(blob: Blob, fileName: string): void {
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
  download(
    await canvasesToPdfBlob([renderPayrollPdfCanvas(model)]),
    `${payrollExportFileStem(model)}.pdf`
  );
}

export async function exportPeriodPayrollPdf(
  period: BordroDonemi,
  models: PayrollExportModel[]
): Promise<void> {
  if (models.length === 0) {
    throw new Error('Bu dönem için CALCULATED veya FINALIZED bordro bulunamadı.');
  }
  download(
    await canvasesToPdfBlob(models.map(renderPayrollPdfCanvas)),
    `${periodExportFileStem(period)}_Ucret_Pusulalari.pdf`
  );
}
