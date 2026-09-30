/**
 * 4/D Sürekli İşçi Bordro Programı - Excel Export Utility
 */

import * as XLSX from 'xlsx';
import { tauriBridge } from '../services/tauriBridge';

export interface ExcelExportColumn {
  header: string;
  key: string;
  width?: number;
}

/**
 * Export data array to XLSX file
 */
export async function exportToExcel<T extends Record<string, any>>(
  fileName: string,
  sheetName: string,
  columns: ExcelExportColumn[],
  data: T[],
  summaryRows?: Record<string, any>[]
): Promise<void> {
  // Yield thread so caller UI state (e.g. loading spinner) can render
  await new Promise((resolve) => setTimeout(resolve, 0));

  // Format headers and mapping
  const headers = columns.map((col) => col.header);
  
  const rows = data.map((item) => {
    return columns.map((col) => {
      const val = item[col.key];
      return val === null || val === undefined ? '' : val;
    });
  });

  if (summaryRows && summaryRows.length > 0) {
    summaryRows.forEach((sRow) => {
      rows.push(
        columns.map((col) => {
          const val = sRow[col.key];
          return val === null || val === undefined ? '' : val;
        })
      );
    });
  }

  const worksheetData = [headers, ...rows];
  const worksheet = XLSX.utils.aoa_to_sheet(worksheetData);

  // Set column widths
  worksheet['!cols'] = columns.map((col) => ({
    wch: col.width || Math.max(col.header.length + 5, 15),
  }));

  const workbook = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(workbook, worksheet, sheetName);

  // Yield before binary serialization and download
  await new Promise((resolve) => setTimeout(resolve, 0));

  if (typeof document !== 'undefined') {
    const excelBuffer = XLSX.write(workbook, { bookType: 'xlsx', type: 'array' });
    const blob = new Blob([excelBuffer], {
      type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;charset=UTF-8',
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${fileName}.xlsx`;
    try {
      document.body.appendChild(anchor);
      anchor.click();
    } finally {
      document.body.removeChild(anchor);
      // Browsers may consume the object URL after click() returns.
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    }
  } else {
    XLSX.writeFile(workbook, `${fileName}.xlsx`);
  }
}

/**
 * Prints one DOM element through the current WebView's native print flow.
 * Opening a secondary window is unreliable in Tauri's WKWebView on macOS.
 */
export async function printElement(elementId: string): Promise<void> {
  const elem = document.getElementById(elementId);
  if (!elem) return;

  const printStyle = document.createElement('style');
  printStyle.dataset.printElementStyle = 'true';
  printStyle.textContent = `
    @media print {
      body.print-element-active * { visibility: hidden !important; }
      body.print-element-active .print-element-host,
      body.print-element-active .print-element-host * { visibility: visible !important; }
      body.print-element-active .print-element-host {
        position: fixed !important;
        inset: 0 auto auto 0 !important;
        width: 100% !important;
        height: auto !important;
        max-height: none !important;
        overflow: visible !important;
        background: white !important;
        color: black !important;
      }
      body.print-element-active .print-element-host .no-print { display: none !important; }
      @page { size: auto; margin: 15mm; }
    }
  `;
  const printHost = document.createElement('div');
  printHost.className = 'print-element-host';
  printHost.appendChild(elem.cloneNode(true));
  const cleanup = () => {
    document.body.classList.remove('print-element-active');
    printHost.remove();
    printStyle.remove();
  };

  document.head.appendChild(printStyle);
  document.body.classList.add('print-element-active');
  document.body.appendChild(printHost);
  window.addEventListener('afterprint', cleanup, { once: true });
  try {
    const printedNatively = tauriBridge.isTauriAvailable()
      ? await tauriBridge.printCurrentWebview()
      : false;
    if (!printedNatively) window.print();
  } finally {
    window.removeEventListener('afterprint', cleanup);
    cleanup();
  }
}
