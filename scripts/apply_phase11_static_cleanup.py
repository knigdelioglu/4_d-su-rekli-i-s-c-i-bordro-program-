from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected exactly one match, found {count}")
    return text.replace(old, new, 1)


# Prevent unsaved manual payroll amounts from leaking between periods.
path = "src/components/BordroHesaplama.tsx"
text = read(path)
text = replace_once(
    text,
    """  const getManualIncomeInput = (personId: string): ManualPayrollIncomeInput => {
    const existingPayroll = bordrolar.find(
      (item) => item.personelId === personId && item.donemId === aktifDonem.id
    );
    const draft = manualIncomeMap[personId];
""",
    """  const getManualIncomeStateKey = (personId: string): string =>
    `${aktifDonem.id}:${personId}`;

  const getManualIncomeInput = (personId: string): ManualPayrollIncomeInput => {
    const existingPayroll = bordrolar.find(
      (item) => item.personelId === personId && item.donemId === aktifDonem.id
    );
    const draft = manualIncomeMap[getManualIncomeStateKey(personId)];
""",
    "manual income period key helper",
)
text = replace_once(
    text,
    """    setManualIncomeMap((current) => ({
      ...current,
      [personId]: {
        ...current[personId],
        [field]: value,
      },
    }));
""",
    """    const stateKey = getManualIncomeStateKey(personId);
    setManualIncomeMap((current) => ({
      ...current,
      [stateKey]: {
        ...current[stateKey],
        [field]: value,
      },
    }));
""",
    "manual income period-scoped state update",
)
text = replace_once(
    text,
    """                  const tediyeInputValue =
                    manualIncomeMap[person.id]?.tediye ??
                    (bordro?.gelirler.tediye != null ? String(bordro.gelirler.tediye) : '');
                  const tisInputValue =
                    manualIncomeMap[person.id]?.tisIkramiyesi ??
                    (bordro?.gelirler.tisIkramiyesi != null
""",
    """                  const manualIncomeStateKey = getManualIncomeStateKey(person.id);
                  const tediyeInputValue =
                    manualIncomeMap[manualIncomeStateKey]?.tediye ??
                    (bordro?.gelirler.tediye != null ? String(bordro.gelirler.tediye) : '');
                  const tisInputValue =
                    manualIncomeMap[manualIncomeStateKey]?.tisIkramiyesi ??
                    (bordro?.gelirler.tisIkramiyesi != null
""",
    "manual income period-scoped row values",
)
write(path, text)

# Keep regression expectations explicit and avoid incidental parse/unwrap logic.
path = "src-tauri/tests/manual_tediye_tis_regression_test.rs"
text = read(path)
text = replace_once(
    text,
    """    for (kind, expected) in [("tediye", dec!(1000.25)), ("tisIkramiyesi", dec!(2000.75))] {
        let (amount, source): (i64, String) = conn
            .query_row(
                "SELECT amount, source FROM payroll_income_items WHERE payroll_id = ?1 AND item_type = ?2",
                params![payroll.id, kind],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        assert_eq!(
            amount,
            (expected * dec!(100))
                .round()
                .to_string()
                .parse::<i64>()
                .unwrap()
        );
        assert_eq!(source, "MANUAL");
    }
""",
    """    for (kind, expected_kurus) in [
        ("tediye", 100_025_i64),
        ("tisIkramiyesi", 200_075_i64),
    ] {
        let (amount, source): (i64, String) = conn
            .query_row(
                "SELECT amount, source FROM payroll_income_items WHERE payroll_id = ?1 AND item_type = ?2",
                params![payroll.id, kind],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        assert_eq!(amount, expected_kurus);
        assert_eq!(source, "MANUAL");
    }
""",
    "manual income persistence expectation",
)
write(path, text)

# Restore normal CI in the final tree. This workflow is not executed here.
write(".github/workflows/ci.yml", """name: CI

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
""")

workflow_path = ROOT / ".github/workflows/apply-phase11-static-cleanup.yml"
if workflow_path.exists():
    workflow_path.unlink()
Path(__file__).unlink()
