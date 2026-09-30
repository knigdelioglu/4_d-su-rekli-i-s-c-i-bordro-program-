/**
 * 4/D Sürekli İşçi Bordro Programı - Excel Export Utility
 */

import * as XLSX from 'xlsx';
import { tauriBridge } from '../services/tauriBridge';

export interface ExcelExportColumn {
  header: string;
  key: string;
  width?: number;
  numFmt?: string;
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
): Promise<boolean> {
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

  columns.forEach((column, columnIndex) => {
    if (!column.numFmt) return;
    for (let rowIndex = 1; rowIndex < worksheetData.length; rowIndex += 1) {
      const cell = worksheet[XLSX.utils.encode_cell({ r: rowIndex, c: columnIndex })];
      if (cell && typeof cell.v === 'number') cell.z = column.numFmt;
    }
  });

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
    if (tauriBridge.isTauriAvailable()) {
      const excelBytes = Array.from(new Uint8Array(excelBuffer));
      return tauriBridge.exportExcel(excelBytes, `${fileName}.xlsx`);
    }

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
    return true;
  } else {
    XLSX.writeFile(workbook, `${fileName}.xlsx`);
    return true;
  }
}

/**
 * Prints one DOM element through the current WebView's native print flow.
 * Opening a secondary window is unreliable in Tauri's WKWebView on macOS.
 */
export async function printElement(elementId: string): Promise<void> {
  const elem = document.getElementById(elementId);
  if (!elem) return;
  if (document.body.classList.contains('print-element-active')) return;

  const printStyle = document.createElement('style');
  printStyle.dataset.printElementStyle = 'true';
  printStyle.textContent = `
    @media print {
      body.print-element-active > *:not(.print-element-host) { display: none !important; }
      body.print-element-active .print-element-host {
        display: block !important;
        position: static !important;
        inset: auto !important;
        width: 100% !important;
        height: auto !important;
        max-height: none !important;
        overflow: visible !important;
        background: white !important;
        color: black !important;
      }
      body.print-element-active .print-element-host .no-print { display: none !important; }
      body.print-element-active .print-element-host > * {
        display: block !important;
        height: auto !important;
        max-height: none !important;
        overflow: visible !important;
      }
      body.print-element-active .print-element-host #payslip-print-container {
        flex: none !important;
        height: auto !important;
        max-height: none !important;
        overflow: visible !important;
      }
      body.print-element-active .print-element-host #payslip-print-container * {
        max-height: none !important;
        overflow: visible !important;
      }
      body.print-element-active .print-element-host #payslip-print-container {
        padding: 0 !important;
        font-size: 9pt;
        line-height: 1.2;
      }
      body.print-element-active .print-element-host .payslip-print-header { padding-bottom: 2mm !important; }
      body.print-element-active .print-element-host .payslip-print-identity {
        margin-top: 2mm !important;
        gap: 2mm !important;
      }
      body.print-element-active .print-element-host .payslip-print-identity-card {
        padding: 2mm !important;
        line-height: 1.2 !important;
        break-inside: avoid-page;
        page-break-inside: avoid;
      }
      body.print-element-active .print-element-host .payslip-print-audits {
        margin-top: 2mm !important;
        gap: 2mm !important;
      }
      body.print-element-active .print-element-host .payslip-print-audit-card {
        padding: 2mm !important;
        line-height: 1.15 !important;
        break-inside: avoid-page;
        page-break-inside: avoid;
      }
      body.print-element-active .print-element-host .payslip-print-audit-card h2 { margin-bottom: 1mm !important; }
      body.print-element-active .print-element-host .payslip-print-sgk-tax-section {
        break-inside: avoid-page;
        page-break-inside: avoid;
      }
      body.print-element-active .print-element-host .payslip-print-final-amount-sections {
        display: block !important;
      }
      body.print-element-active .print-element-host .payslip-print-attendance { margin-top: 2mm !important; }
      body.print-element-active .print-element-host .payslip-print-attendance h2,
      body.print-element-active .print-element-host .payslip-print-amount-sections h2 { margin-bottom: 1mm !important; }
      body.print-element-active .print-element-host .payslip-print-attendance > div > div { padding: 1mm !important; }
      body.print-element-active .print-element-host .payslip-print-amount-sections {
        margin-top: 2mm !important;
        column-gap: 3mm !important;
        row-gap: 2mm !important;
      }
      body.print-element-active .print-element-host .payslip-print-amount-sections section > div > div {
        padding-top: 1mm !important;
        padding-bottom: 1mm !important;
      }
      body.print-element-active .print-element-host .payslip-print-net {
        margin-top: 3mm !important;
        padding: 3mm 4mm !important;
        break-inside: avoid-page;
        page-break-inside: avoid;
      }
      body.print-element-active .print-element-host .payslip-print-footer {
        margin-top: 2mm !important;
        padding-top: 1mm !important;
      }
      @page { size: auto; margin: 8mm; }
    }
  `;
  const printHost = document.createElement('div');
  printHost.className = 'print-element-host';
  printHost.appendChild(elem.cloneNode(true));
  let cleanedUp = false;
  const cleanup = () => {
    if (cleanedUp) return;
    cleanedUp = true;
    window.removeEventListener('afterprint', cleanup);
    document.body.classList.remove('print-element-active');
    printHost.remove();
    printStyle.remove();
  };

  document.head.appendChild(printStyle);
  document.body.classList.add('print-element-active');
  document.body.appendChild(printHost);
  // Ensure WebKit applies the print host and isolation styles before native printing snapshots the view.
  void document.body.offsetHeight;
  window.addEventListener('afterprint', cleanup);
  try {
    const printedNatively = tauriBridge.isTauriAvailable()
      ? await tauriBridge.printCurrentWebview()
      : false;
    if (!printedNatively) {
      window.print();
      cleanup();
    }
  } catch (error) {
    cleanup();
    throw error;
  }
}
