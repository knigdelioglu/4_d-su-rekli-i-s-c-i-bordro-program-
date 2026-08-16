from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


path = "src/utils/payrollUtils.ts"
text = read(path)
text = replace_once(
    text,
    '  tediyeTisNotu: "6772 sayılı Kanun uyarınca 4/D kamu çalışanlarına yılda 4 defa ilave tediye (13\'er günlük) ve Toplu İş Sözleşmesi (TİS) hükümlerine göre yılda 2 defa ikramiye ödenir. Aşağıdan ödeme aylarını, gün sayılarını (manuel) ve aktif dönemde ödenip ödenmeyeceğini belirleyebilirsiniz.",',
    '  tediyeTisNotu: "Tediye ve TİS listeleri yalnız referans takvimidir. Ödeme ayı ve gün sayısı burada not edilebilir; bordroya aktarılacak gerçek brüt Tediye/TİS tutarı Bordro Hesaplama ekranında personel ve dönem bazında manuel girilir.",',
    "default Tediye/TIS note",
)
text = replace_once(
    text,
    """  // Active Tediye calculation
  let tediye: number | null = null;
  const activeTediye = kurumDegerleri.tediyeListesi?.find((t) => t.aktifDonemdeOdensin);
  if (activeTediye) {
    tediye =
      activeTediye.sabitTutar && activeTediye.sabitTutar > 0
        ? activeTediye.sabitTutar
        : Math.round(activeTediye.gunSayisi * kurumDegerleri.gunlukTabanUcret * 100) / 100;
  }

  // Active TİS İkramiyesi calculation
  let tisIkramiyesi: number | null = null;
  const activeTis = kurumDegerleri.tisIkramiyeListesi?.find((t) => t.aktifDonemdeOdensin);
  if (activeTis) {
    tisIkramiyesi =
      activeTis.sabitTutar && activeTis.sabitTutar > 0
        ? activeTis.sabitTutar
        : Math.round(activeTis.gunSayisi * kurumDegerleri.gunlukTabanUcret * 100) / 100;
  }
""",
    """  // Tediye ve TİS ikramiyesi manual-only ürün girdileridir.
  // Legacy browser helper da dönem listesinden otomatik tutar üretmez.
  const tediye: number | null = null;
  const tisIkramiyesi: number | null = null;
""",
    "legacy browser auto Tediye/TIS formula",
)
write(path, text)

# Browser-side regression contract. It is intentionally written now and will be
# executed only when final verification is explicitly requested.
write(
    "src/utils/manualTediyeTis.test.ts",
    """import { describe, expect, test } from 'bun:test';
import { DönemselKurumDegerleri, PuantajOzeti } from '../types/payroll';
import {
  autoFillGelirlerFromPuantaj,
  DEFAULT_KURUM_DEGERLERI,
} from './payrollUtils';

describe('Tediye/TİS manual-only browser contract', () => {
  test('aktif legacy listeler browser helper içinde otomatik gelir üretmemeli', () => {
    const kurum: DönemselKurumDegerleri = {
      donemId: '2026-05',
      ...DEFAULT_KURUM_DEGERLERI,
      tediyeListesi: [
        {
          id: 1,
          ad: 'Legacy aktif Tediye',
          odemeAyi: 'Haziran',
          gunSayisi: 13,
          aktifDonemdeOdensin: true,
          sabitTutar: 9999,
        },
      ],
      tisIkramiyeListesi: [
        {
          id: 1,
          ad: 'Legacy aktif TİS',
          odemeAyi: 'Haziran',
          gunSayisi: 30,
          aktifDonemdeOdensin: true,
          sabitTutar: 8888,
        },
      ],
    };
    const puantaj: PuantajOzeti = {
      Ç: 1,
      T: 0,
      G: 0,
      İ: 0,
      GÇ: 0,
      GÇT: 0,
      R: 0,
    };

    const gelirler = autoFillGelirlerFromPuantaj(puantaj, kurum, 1, '1. Grup');
    expect(gelirler.tediye).toBeNull();
    expect(gelirler.tisIkramiyesi).toBeNull();
  });
});
""",
)

# Restore the canonical CI file without triggering it from this bot-authored commit.
write(
    ".github/workflows/ci.yml",
    """name: CI

on:
  push:
  pull_request:

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest

      - name: Install JavaScript dependencies
        run: bun install --frozen-lockfile

      - name: TypeScript lint
        run: bun run lint

      - name: Bun tests
        run: bun test

      - name: Vite build
        run: bun run build

      - name: Install Tauri Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \\
            libwebkit2gtk-4.1-dev \\
            build-essential \\
            curl \\
            wget \\
            file \\
            libxdo-dev \\
            libssl-dev \\
            libayatana-appindicator3-dev \\
            librsvg2-dev \\
            patchelf

      - name: Setup Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy
          cache: false

      - name: Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'
          cache-on-failure: true

      - name: Rust format check
        working-directory: src-tauri
        run: cargo fmt --check

      - name: Rust clippy
        working-directory: src-tauri
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Rust tests
        working-directory: src-tauri
        run: cargo test
""",
)

workflow = ROOT / ".github/workflows/finalize-phase11-static-contract.yml"
if workflow.exists():
    workflow.unlink()
Path(__file__).unlink()
