from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 exact match, found {count}")
    return text.replace(old, new, 1)


def regex_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 regex match, found {count}")
    return updated


# 1) Rust domain model: explicit manual payroll income input.
path = "src-tauri/src/domain/models.rs"
text = read(path)
marker = """#[derive(Debug, Clone, Serialize, Deserialize, Default)]\n#[serde(rename_all = \"camelCase\")]\npub struct KesintiKalemleri {\n"""
insert = """#[derive(Debug, Clone, Serialize, Deserialize, Default)]\n#[serde(rename_all = \"camelCase\")]\npub struct ManualPayrollIncomeInput {\n    /// Tediye tutarı kullanıcı tarafından kişi+dönem bazında girilir.\n    pub tediye: Option<Decimal>,\n    /// TİS ikramiyesi tutarı kullanıcı tarafından kişi+dönem bazında girilir.\n    pub tisIkramiyesi: Option<Decimal>,\n}\n\n""" + marker
text = replace_once(text, marker, insert, "models manual input")
write(path, text)


# 2) Calculation helper must never manufacture Tediye/TİS from legacy period lists.
path = "src-tauri/src/domain/calculations.rs"
text = read(path)
pattern = r"""\n    let mut tediye: Option<Decimal> = None;\n    if let Some\(ref t_list\) = kurum_degerleri\.tediyeListesi \{.*?\n    gelirler\.tediye = tediye;\n    gelirler\.tisIkramiyesi = tis_ikramiyesi;"""
replacement = """
    // Tediye ve TİS ikramiyesi authoritative olarak otomatik üretilmez.
    // Dönem ayarlarındaki legacy takvim/listeler yalnız referans/migration verisidir;
    // production bordro yolu kişi+dönem bazındaki ManualPayrollIncomeInput'u uygular.
    gelirler.tediye = None;
    gelirler.tisIkramiyesi = None;"""
text = regex_once(text, pattern, replacement, "remove automatic tediye/tis")
write(path, text)


# 3) Production payroll service: validate/apply explicit manual input before statutory math.
path = "src-tauri/src/services/payroll_service.rs"
text = read(path)
service_marker = "pub struct PayrollService;\n\nimpl PayrollService {\n"
service_helpers = """fn validate_manual_payroll_income_input(input: &ManualPayrollIncomeInput) -> Result<()> {
    for (field, value) in [
        ("tediye", input.tediye),
        ("tisIkramiyesi", input.tisIkramiyesi),
    ] {
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(DomainError::ValidationError(format!(
                "Manuel {} tutarı negatif olamaz.",
                field
            )));
        }
    }
    Ok(())
}

fn apply_manual_payroll_income(
    gelirler: &mut GelirKalemleri,
    input: Option<&ManualPayrollIncomeInput>,
) -> Result<()> {
    if let Some(input) = input {
        validate_manual_payroll_income_input(input)?;
        gelirler.tediye = input.tediye.map(|value| value.round_dp(2));
        gelirler.tisIkramiyesi = input.tisIkramiyesi.map(|value| value.round_dp(2));
    } else {
        // Manual-only contract: an omitted input means no Tediye/TİS amount.
        // Never fall back to period-level day × wage formulas.
        gelirler.tediye = None;
        gelirler.tisIkramiyesi = None;
    }
    Ok(())
}

pub struct PayrollService;

impl PayrollService {
"""
text = replace_once(text, service_marker, service_helpers, "payroll service helpers")
old_sig = """    pub fn calculate_payroll_for_personnel(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<BordroKaydi> {
        let personel = PersonnelRepository::get_by_id(conn, personnel_id)?.ok_or_else(|| {
"""
new_sig = """    pub fn calculate_payroll_for_personnel(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<BordroKaydi> {
        Self::calculate_payroll_for_personnel_with_manual_income(
            conn,
            personnel_id,
            period_id,
            None,
        )
    }

    pub fn calculate_payroll_for_personnel_with_manual_income(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        manual_income: Option<&ManualPayrollIncomeInput>,
    ) -> Result<BordroKaydi> {
        if let Some(input) = manual_income {
            validate_manual_payroll_income_input(input)?;
        }
        let personel = PersonnelRepository::get_by_id(conn, personnel_id)?.ok_or_else(|| {
"""
text = replace_once(text, old_sig, new_sig, "payroll service signature")
old_apply = """        let (mut gelirler, mut is_primi_detay) = auto_fill_gelirler_from_puantaj(
            &summary,
            &kurum_degerleri,
            personel.hizmetYili,
            Some(&personel.grup),
        )?;

        if let Some(cutoff) = zam_tarihi {
"""
new_apply = """        let (mut gelirler, mut is_primi_detay) = auto_fill_gelirler_from_puantaj(
            &summary,
            &kurum_degerleri,
            personel.hizmetYili,
            Some(&personel.grup),
        )?;
        apply_manual_payroll_income(&mut gelirler, manual_income)?;

        if let Some(cutoff) = zam_tarihi {
"""
text = replace_once(text, old_apply, new_apply, "apply manual income")
write(path, text)


# 4) Tauri command accepts only the explicit manual input object.
path = "src-tauri/src/commands/payroll_cmd.rs"
text = read(path)
old = """pub fn calculate_payroll(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
) -> Result<BordroKaydi> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollService::calculate_payroll_for_personnel(&conn, &personnel_id, &period_id)
}
"""
new = """pub fn calculate_payroll(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
    manual_income: Option<ManualPayrollIncomeInput>,
) -> Result<BordroKaydi> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        &personnel_id,
        &period_id,
        manual_income.as_ref(),
    )
}
"""
text = replace_once(text, old, new, "payroll command manual input")
write(path, text)


# 5) TS contract and Tauri bridge.
path = "src/types/payroll.ts"
text = read(path)
marker = """export interface GelirKalemleri {\n"""
insert = """export interface ManualPayrollIncomeInput {\n  /** Kişi+dönem bazında kullanıcı tarafından girilen brüt Tediye tutarı. */\n  tediye?: number | null;\n  /** Kişi+dönem bazında kullanıcı tarafından girilen brüt TİS ikramiyesi tutarı. */\n  tisIkramiyesi?: number | null;\n}\n\n""" + marker
text = replace_once(text, marker, insert, "TS manual income type")
write(path, text)

path = "src/services/tauriBridge.ts"
text = read(path)
text = replace_once(
    text,
    "  SickLeaveRecord,\n} from '../types/payroll';",
    "  SickLeaveRecord,\n  ManualPayrollIncomeInput,\n} from '../types/payroll';",
    "bridge type import",
)
old = """  async calculatePayroll(personnelId: string, periodId: string): Promise<BordroKaydi> {
    return invokeTauri<BordroKaydi>('calculate_payroll', { personnelId, periodId });
  },
"""
new = """  async calculatePayroll(
    personnelId: string,
    periodId: string,
    manualIncome?: ManualPayrollIncomeInput
  ): Promise<BordroKaydi> {
    return invokeTauri<BordroKaydi>('calculate_payroll', {
      personnelId,
      periodId,
      manualIncome: manualIncome ?? null,
    });
  },
"""
text = replace_once(text, old, new, "bridge calculate payroll")
write(path, text)


# 6) Payroll screen: two explicit manual amount cells per person/period.
path = "src/components/BordroHesaplama.tsx"
text = read(path)
text = replace_once(
    text,
    "  PersonelTaxOpening,\n} from '../types/payroll';",
    "  PersonelTaxOpening,\n  ManualPayrollIncomeInput,\n} from '../types/payroll';",
    "payroll screen type import",
)
state_marker = """  const [errorMessage, setErrorMessage] = useState<string | null>(null);\n\n  // Manual cumulative GV override state per person for the current period\n"""
state_insert = """  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [manualIncomeMap, setManualIncomeMap] = useState<
    Record<string, { tediye?: string; tisIkramiyesi?: string }>
  >({});

  // Manual cumulative GV override state per person for the current period
"""
text = replace_once(text, state_marker, state_insert, "manual income state")
helper_marker = """  const activeKurumDegerleri = kurumDegerleriMap[aktifDonem.id];\n\n  const getDevirGvMatrahiForActiveYear = (person: Personel): number => {\n"""
helper_insert = """  const activeKurumDegerleri = kurumDegerleriMap[aktifDonem.id];

  const getManualIncomeInput = (personId: string): ManualPayrollIncomeInput => {
    const existingPayroll = bordrolar.find(
      (item) => item.personelId === personId && item.donemId === aktifDonem.id
    );
    const draft = manualIncomeMap[personId];
    const resolveAmount = (
      field: 'tediye' | 'tisIkramiyesi'
    ): number | null => {
      const raw = draft?.[field];
      if (raw !== undefined) {
        if (raw.trim() === '') return null;
        const parsed = Number(raw);
        return Number.isFinite(parsed) ? parsed : null;
      }
      return existingPayroll?.gelirler[field] ?? null;
    };

    return {
      tediye: resolveAmount('tediye'),
      tisIkramiyesi: resolveAmount('tisIkramiyesi'),
    };
  };

  const updateManualIncomeDraft = (
    personId: string,
    field: 'tediye' | 'tisIkramiyesi',
    value: string
  ) => {
    setManualIncomeMap((current) => ({
      ...current,
      [personId]: {
        ...current[personId],
        [field]: value,
      },
    }));
  };

  const getDevirGvMatrahiForActiveYear = (person: Personel): number => {
"""
text = replace_once(text, helper_marker, helper_insert, "manual income helpers")
text = replace_once(
    text,
    "      const rustBordro = await tauriBridge.calculatePayroll(person.id, aktifDonem.id);",
    "      const rustBordro = await tauriBridge.calculatePayroll(\n        person.id,\n        aktifDonem.id,\n        getManualIncomeInput(person.id)\n      );",
    "send manual income",
)
text = replace_once(
    text,
    """                <th className=\"py-3 px-4 text-right\">Brüt Gelir</th>\n                <th className=\"py-3 px-4 text-right\">Kesintiler</th>""",
    """                <th className=\"py-3 px-4 text-right\">Brüt Gelir</th>
                <th className=\"py-3 px-4 text-right\">Tediye (Manuel)</th>
                <th className=\"py-3 px-4 text-right\">TİS İkramiye (Manuel)</th>
                <th className=\"py-3 px-4 text-right\">Kesintiler</th>""",
    "manual table headers",
)
text = replace_once(text, "<td colSpan={10}", "<td colSpan={12}", "manual table colspan")
vars_old = """                  const brut = bordro?.gelirToplam || 0;
                  const kesinti = bordro?.kesintiToplam || 0;
                  const net = bordro?.netOdeme || 0;

                  return (
"""
vars_new = """                  const brut = bordro?.gelirToplam || 0;
                  const kesinti = bordro?.kesintiToplam || 0;
                  const net = bordro?.netOdeme || 0;
                  const tediyeInputValue =
                    manualIncomeMap[person.id]?.tediye ??
                    (bordro?.gelirler.tediye != null ? String(bordro.gelirler.tediye) : '');
                  const tisInputValue =
                    manualIncomeMap[person.id]?.tisIkramiyesi ??
                    (bordro?.gelirler.tisIkramiyesi != null
                      ? String(bordro.gelirler.tisIkramiyesi)
                      : '');

                  return (
"""
text = replace_once(text, vars_old, vars_new, "manual row values")
brut_cell = """                      {/* Brüt */}
                      <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-slate-800'}`}>
                        {hasPayrollSnapshot ? formatTL(brut) : '—'}
                      </td>

                      {/* Kesintiler */}
"""
manual_cells = """                      {/* Brüt */}
                      <td className={`py-3 px-4 text-right font-mono font-medium ${isStale ? 'text-amber-700 line-through' : 'text-slate-800'}`}>
                        {hasPayrollSnapshot ? formatTL(brut) : '—'}
                      </td>

                      {/* Manuel Tediye */}
                      <td className="py-3 px-4 text-right" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="number"
                          min="0"
                          step="0.01"
                          inputMode="decimal"
                          disabled={isFinalized}
                          value={tediyeInputValue}
                          onChange={(e) => updateManualIncomeDraft(person.id, 'tediye', e.target.value)}
                          placeholder="Boş"
                          title="Brüt Tediye tutarını manuel girin. Boş bırakılırsa Tediye hesaplanmaz."
                          className="w-28 px-2 py-1.5 text-right bg-white border border-amber-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-amber-500 disabled:bg-slate-100 disabled:text-slate-500"
                        />
                      </td>

                      {/* Manuel TİS İkramiyesi */}
                      <td className="py-3 px-4 text-right" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="number"
                          min="0"
                          step="0.01"
                          inputMode="decimal"
                          disabled={isFinalized}
                          value={tisInputValue}
                          onChange={(e) => updateManualIncomeDraft(person.id, 'tisIkramiyesi', e.target.value)}
                          placeholder="Boş"
                          title="Brüt TİS ikramiyesi tutarını manuel girin. Boş bırakılırsa TİS ikramiyesi hesaplanmaz."
                          className="w-28 px-2 py-1.5 text-right bg-white border border-indigo-200 rounded-lg text-xs font-mono text-slate-900 focus:ring-2 focus:ring-indigo-500 disabled:bg-slate-100 disabled:text-slate-500"
                        />
                      </td>

                      {/* Kesintiler */}
"""
text = replace_once(text, brut_cell, manual_cells, "manual income cells")
write(path, text)


# 7) Period manager: legacy schedule is reference-only; no amount preview/formula messaging.
path = "src/components/PeriodManagerModal.tsx"
text = read(path)
text = replace_once(
    text,
    "                15 - 14 Tarih aralıkları, dönemsel parametreler, Tediye ve TİS ikramiyeleri",
    "                15 - 14 tarih aralıkları, dönemsel parametreler ve Tediye/TİS referans takvimi",
    "period manager subtitle",
)
form_marker = """          {activeTab === 'tediyeTis' && (
            <form onSubmit={handleSaveParams} className="space-y-6">
              {/* Note banner (Editable) */}
"""
form_insert = """          {activeTab === 'tediyeTis' && (
            <form onSubmit={handleSaveParams} className="space-y-6">
              <div className="bg-indigo-50 border border-indigo-200 rounded-2xl p-4 text-xs text-indigo-950 leading-relaxed">
                <strong className="block mb-1">Manuel bordro girdisi</strong>
                Bu sekmedeki Tediye/TİS listeleri yalnız referans takvim ve açıklama amacıyla korunur.
                Bordro motoru buradaki gün sayısı, aktiflik veya sabit tutardan otomatik ödeme üretmez.
                Gerçek brüt Tediye ve TİS ikramiyesi tutarını Bordro Hesaplama ekranında her personel için manuel girin.
              </div>
              {/* Note banner (Editable) */}
"""
text = replace_once(text, form_marker, form_insert, "manual-only notice")
text = replace_once(
    text,
    "<span>Tediye ve TİS ikramiye ayarları başarıyla kaydedildi!</span>",
    "<span>Tediye ve TİS referans ayarları başarıyla kaydedildi!</span>",
    "reference success copy",
)
text = regex_once(
    text,
    r"""\s+const calculatedAmount = t\.sabitTutar && t\.sabitTutar > 0\n\s+\? t\.sabitTutar\n\s+: \(t\.gunSayisi \|\| 13\) \* \(paramsForm\.gunlukTabanUcret \|\| 0\);\n""",
    "\n",
    "remove tediye preview formula",
)
text = regex_once(
    text,
    r"""\s+const calculatedAmount = tis\.sabitTutar && tis\.sabitTutar > 0\n\s+\? tis\.sabitTutar\n\s+: \(tis\.gunSayisi \|\| 0\) \* \(paramsForm\.gunlukTabanUcret \|\| 0\);\n""",
    "\n",
    "remove tis preview formula",
)
text = replace_once(
    text,
    """                          <span className="text-slate-500">Tediye Brüt Tutar:</span>
                          <span className="font-bold font-mono text-amber-900">
                            {formatTL(calculatedAmount)}
                          </span>""",
    """                          <span className="text-slate-500">Bordro tutarı:</span>
                          <span className="font-bold text-amber-900">Personel bazında manuel girilir</span>""",
    "tediye manual preview copy",
)
text = replace_once(
    text,
    """                          <span className="text-slate-500">TİS İkramiye Brüt Tutar:</span>
                          <span className="font-bold font-mono text-indigo-900">
                            {formatTL(calculatedAmount)}
                          </span>""",
    """                          <span className="text-slate-500">Bordro tutarı:</span>
                          <span className="font-bold text-indigo-900">Personel bazında manuel girilir</span>""",
    "tis manual preview copy",
)
text = text.replace("<span>Ödeniyor</span>", "<span>Referans aktif</span>")
text = replace_once(
    text,
    "<span>Tediye & TİS Ayarlarını Kaydet</span>",
    "<span>Tediye & TİS Referanslarını Kaydet</span>",
    "reference save copy",
)
write(path, text)


# 8) Regression tests (not executed in this phase).
test_path = ROOT / "src-tauri/tests/manual_tediye_tis_regression_test.rs"
test_path.write_text(r'''use bordro_programi_lib::db::connection::create_in_memory_connection;
use bordro_programi_lib::domain::calculations::auto_fill_gelirler_from_puantaj;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::{DomainError, Result};
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use rusqlite::params;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: format!("10000000{}", id.len()),
        ad: "Manuel".into(),
        soyad: "Gelir".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: String::new(),
        iban: String::new(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-05".into(),
        yil: 2026,
        ay: 5,
        baslangicTarihi: "2026-05-15".into(),
        bitisTarihi: "2026-06-14".into(),
        donemAdi: "Mayıs 2026".into(),
        taxYear: 2026,
        taxMonth: 6,
    }
}

fn setup(personnel_id: &str) -> Result<rusqlite::Connection> {
    let conn = create_in_memory_connection()
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let p = person(personnel_id);
    let d = period();
    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &d)?;
    let mut settings = DonemselKurumDegerleri {
        donemId: d.id.clone(),
        ..DonemselKurumDegerleri::default()
    };
    settings.tediyeListesi = Some(vec![TediyeKalemi {
        id: 1,
        ad: "Legacy aktif tediye".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 13,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(9999)),
    }]);
    settings.tisIkramiyeListesi = Some(vec![TisIkramiyeKalemi {
        id: 1,
        ad: "Legacy aktif TİS".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 30,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(8888)),
    }]);
    SettingsRepository::save_institution_settings(&conn, &settings)?;
    let mut gunler = HashMap::new();
    gunler.insert("2026-05-15".into(), "Ç".into());
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: format!("{}_{}", personnel_id, d.id),
            personelId: personnel_id.into(),
            donemId: d.id,
            gunler,
        },
    )?;
    Ok(conn)
}

#[test]
fn legacy_period_lists_do_not_auto_create_tediye_or_tis() -> Result<()> {
    let mut settings = DonemselKurumDegerleri::default();
    settings.tediyeListesi = Some(vec![TediyeKalemi {
        id: 1,
        ad: "Aktif".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 13,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(5000)),
    }]);
    settings.tisIkramiyeListesi = Some(vec![TisIkramiyeKalemi {
        id: 1,
        ad: "Aktif".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 30,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(6000)),
    }]);
    let summary = PuantajOzeti {
        c: 1,
        ..Default::default()
    };
    let (income, _) =
        auto_fill_gelirler_from_puantaj(&summary, &settings, 1, Some("1. Grup"))?;
    assert_eq!(income.tediye, None);
    assert_eq!(income.tisIkramiyesi, None);
    Ok(())
}

#[test]
fn production_payroll_uses_only_explicit_manual_tediye_and_tis() -> Result<()> {
    let conn = setup("manual-income")?;
    let manual = ManualPayrollIncomeInput {
        tediye: Some(dec!(1000.25)),
        tisIkramiyesi: Some(dec!(2000.75)),
    };
    let payroll = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "manual-income",
        "2026-05",
        Some(&manual),
    )?;
    assert_eq!(payroll.gelirler.tediye, Some(dec!(1000.25)));
    assert_eq!(payroll.gelirler.tisIkramiyesi, Some(dec!(2000.75)));

    for (kind, expected) in [("tediye", dec!(1000.25)), ("tisIkramiyesi", dec!(2000.75))] {
        let (amount, source): (i64, String) = conn
            .query_row(
                "SELECT amount, source FROM payroll_income_items WHERE payroll_id = ?1 AND item_type = ?2",
                params![payroll.id, kind],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        assert_eq!(amount, (expected * dec!(100)).round().to_string().parse::<i64>().unwrap());
        assert_eq!(source, "MANUAL");
    }
    Ok(())
}

#[test]
fn omitted_manual_input_does_not_fall_back_to_legacy_active_lists() -> Result<()> {
    let conn = setup("no-manual")?;
    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, "no-manual", "2026-05")?;
    assert_eq!(payroll.gelirler.tediye, None);
    assert_eq!(payroll.gelirler.tisIkramiyesi, None);
    Ok(())
}

#[test]
fn negative_manual_tediye_fails_before_persistence() -> Result<()> {
    let conn = setup("negative-tediye")?;
    let manual = ManualPayrollIncomeInput {
        tediye: Some(dec!(-0.01)),
        tisIkramiyesi: None,
    };
    let result = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "negative-tediye",
        "2026-05",
        Some(&manual),
    );
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
    assert!(PayrollRepository::get_all(&conn)?.is_empty());
    Ok(())
}

#[test]
fn negative_manual_tis_fails_before_persistence() -> Result<()> {
    let conn = setup("negative-tis")?;
    let manual = ManualPayrollIncomeInput {
        tediye: None,
        tisIkramiyesi: Some(dec!(-0.01)),
    };
    let result = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "negative-tis",
        "2026-05",
        Some(&manual),
    );
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
    assert!(PayrollRepository::get_all(&conn)?.is_empty());
    Ok(())
}

#[test]
fn explicit_blank_manual_input_clears_previous_manual_amounts() -> Result<()> {
    let conn = setup("clear-manual")?;
    let first = ManualPayrollIncomeInput {
        tediye: Some(dec!(1000)),
        tisIkramiyesi: Some(dec!(2000)),
    };
    PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "clear-manual",
        "2026-05",
        Some(&first),
    )?;

    let cleared = ManualPayrollIncomeInput::default();
    let payroll = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "clear-manual",
        "2026-05",
        Some(&cleared),
    )?;
    assert_eq!(payroll.gelirler.tediye, None);
    assert_eq!(payroll.gelirler.tisIkramiyesi, None);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM payroll_income_items WHERE payroll_id = ?1 AND item_type IN ('tediye', 'tisIkramiyesi')",
            params![payroll.id],
            |row| row.get(0),
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    assert_eq!(count, 0);
    Ok(())
}
''', encoding="utf-8")


# 9) Hardening plan: implementation done, final verification intentionally pending.
path = "docs/payroll-engine-hardening-plan.md"
text = read(path)
text = replace_once(
    text,
    "- ⏳ Faz 6–11 bu planın kalan işleri olarak devam ediyor.",
    "- ✅ Faz 6–10 — kalan P1 hardening işleri tamamlandı.\n- 🧪 Faz 11 — Tediye/TİS manuel-only akışı uygulandı; final test/CI doğrulaması bekliyor.",
    "plan top status",
)
text = replace_once(
    text,
    "## Faz 11 — P2: Tediye ve TİS'i manuel ürün kararına hizala",
    "## Faz 11 — P2: Tediye ve TİS'i manuel ürün kararına hizala [UYGULANDI — TEST BEKLİYOR]",
    "plan phase 11 status",
)
write(path, text)


# 10) Restore normal CI definition in the final tree; this bot-authored patch commit will not
# recursively trigger GitHub Actions. Tests remain intentionally unexecuted until requested.
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

# The one-shot workflow and this script are removed from the resulting source tree.
workflow_path = ROOT / ".github/workflows/apply-phase11-manual-income.yml"
if workflow_path.exists():
    workflow_path.unlink()
Path(__file__).unlink()
