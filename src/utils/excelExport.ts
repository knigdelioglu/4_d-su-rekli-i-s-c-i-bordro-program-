/**
 * 4/D Sürekli İşçi Bordro Programı - Excel Export Utility
 */

import * as XLSX from 'xlsx';

export interface ExcelExportColumn {
  header: string;
  key: string;
  width?: number;
}

/**
 * Export data array to XLSX file
 */
export function exportToExcel<T extends Record<string, any>>(
  fileName: string,
  sheetName: string,
  columns: ExcelExportColumn[],
  data: T[],
  summaryRows?: Record<string, any>[]
) {
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

  XLSX.writeFile(workbook, `${fileName}.xlsx`);
}

/**
 * Triggers browser window print for a given DOM element ID
 */
export function printElement(elementId: string) {
  const elem = document.getElementById(elementId);
  if (!elem) return;

  const printWindow = window.open('', '_blank');
  if (!printWindow) {
    window.print();
    return;
  }

  const styles = Array.from(document.querySelectorAll('style, link[rel="stylesheet"]'))
    .map((style) => style.outerHTML)
    .join('');

  printWindow.document.write(`
    <!DOCTYPE html>
    <html>
      <head>
        <title>Yazdır - 4/D Bordro Sistem</title>
        ${styles}
        <style>
          body { background: white !important; color: black !important; padding: 20px; font-family: system-ui, sans-serif; }
          @page { size: auto; margin: 15mm; }
          .no-print { display: none !important; }
        </style>
      </head>
      <body>
        ${elem.innerHTML}
        <script>
          setTimeout(() => {
            window.print();
            window.close();
          }, 300);
        </script>
      </body>
    </html>
  `);
  printWindow.document.close();
}
