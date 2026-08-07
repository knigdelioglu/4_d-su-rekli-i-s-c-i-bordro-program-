use crate::domain::models::*;
use crate::domain::Result;
use super::{dec_to_kurus, kurus_to_dec, opt_dec_to_kurus, opt_kurus_to_dec};
use rusqlite::{params, Connection};
use chrono::Utc;

pub struct PayrollRepository;

impl PayrollRepository {
    pub fn get_all(conn: &Connection) -> Result<Vec<BordroKaydi>> {
        let mut stmt = conn.prepare(
            "SELECT id, personnel_id, period_id, gross_total, sgk_base, gv_base, previous_cumulative_gv,
                    new_cumulative_gv, income_tax, stamp_tax, total_deductions, net_payment, status,
                    puantaj_summary_json, pek_detail_json, devreden_pek_gelen_json, sonraki_devreden_pek_json,
                    calculated_at, updated_at, raporlu_gun, odenen_raporlu_gun, is_primi_snapshot_json
             FROM payroll_records",
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let personnel_id: String = row.get(1)?;
            let period_id: String = row.get(2)?;
            let gross_total: i64 = row.get(3)?;
            let _sgk_base: i64 = row.get(4)?;
            let _gv_base: i64 = row.get(5)?;
            let previous_cumulative_gv: i64 = row.get(6)?;
            let _new_cumulative_gv: i64 = row.get(7)?;
            let _income_tax: i64 = row.get(8)?;
            let _stamp_tax: i64 = row.get(9)?;
            let total_deductions: i64 = row.get(10)?;
            let net_payment: i64 = row.get(11)?;
            let status_str: String = row.get(12)?;
            let puantaj_summary_json: String = row.get(13)?;
            let pek_detail_json: Option<String> = row.get(14)?;
            let devreden_pek_gelen_json: Option<String> = row.get(15)?;
            let sonraki_devreden_pek_json: Option<String> = row.get(16)?;
            let calculated_at: String = row.get(17)?;
            let updated_at: String = row.get(18)?;
            let raporlu_gun: Option<i32> = row.get(19)?;
            let odenen_raporlu_gun: Option<i32> = row.get(20)?;
            let is_primi_snapshot_json: Option<String> = row.get(21)?;

            let status = match status_str.as_str() {
                "DRAFT" => BordroStatus::DRAFT,
                "FINALIZED" => BordroStatus::FINALIZED,
                _ => BordroStatus::CALCULATED,
            };

            let puantaj_summary: PuantajOzeti = serde_json::from_str(&puantaj_summary_json).unwrap_or_default();
            let pek_detay: Option<PekDetayi> = pek_detail_json.and_then(|j| serde_json::from_str(&j).ok());
            let devreden_pek_gelen: Option<Vec<DevredenPekKaydi>> = devreden_pek_gelen_json.and_then(|j| serde_json::from_str(&j).ok());
            let sonraki_devreden_pek: Option<Vec<DevredenPekKaydi>> = sonraki_devreden_pek_json.and_then(|j| serde_json::from_str(&j).ok());
            let is_primi_detay: Option<IsPrimiHesapDetayi> = is_primi_snapshot_json.and_then(|j| serde_json::from_str(&j).ok());

            Ok((id, personnel_id, period_id, gross_total, previous_cumulative_gv, total_deductions, net_payment, status, puantaj_summary, pek_detay, devreden_pek_gelen, sonraki_devreden_pek, calculated_at, updated_at, raporlu_gun, odenen_raporlu_gun, is_primi_detay))
        }).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut record_tuples = Vec::new();
        for r in rows {
            record_tuples.push(r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?);
        }

        let mut result = Vec::new();

        for (id, personnel_id, period_id, gross_total, previous_cumulative_gv, total_deductions, net_payment, status, puantaj_summary, pek_detay, devreden_pek_gelen, sonraki_devreden_pek, calculated_at, updated_at, raporlu_gun, odenen_raporlu_gun, is_primi_detay) in record_tuples {
            // Load income items
            let mut inc_stmt = conn.prepare(
                "SELECT item_type, amount FROM payroll_income_items WHERE payroll_id = ?1",
            ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

            let inc_rows = inc_stmt.query_map(params![id], |row| {
                let item_type: String = row.get(0)?;
                let amount: i64 = row.get(1)?;
                Ok((item_type, kurus_to_dec(amount)))
            }).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

            let mut gelirler = GelirKalemleri::default();
            for r in inc_rows {
                if let Ok((t, amt)) = r {
                    match t.as_str() {
                        "tabanBrutAylik" => gelirler.tabanBrutAylik = Some(amt),
                        "tediye" => gelirler.tediye = Some(amt),
                        "tisIkramiyesi" => gelirler.tisIkramiyesi = Some(amt),
                        "ekOdeme" => gelirler.ekOdeme = Some(amt),
                        "yemek" => gelirler.yemek = Some(amt),
                        "birlestirilmisSosyalYardim" => gelirler.birlestirilmisSosyalYardim = Some(amt),
                        "vasitaYol" => gelirler.vasitaYol = Some(amt),
                        "giyimYardimi" => gelirler.giyimYardimi = Some(amt),
                        "isPrimi" => gelirler.isPrimi = Some(amt),
                        "geceCalismasiUcreti" => gelirler.geceCalismasiUcreti = Some(amt),
                        "geceCalismasiTatiliUcreti" => gelirler.geceCalismasiTatiliUcreti = Some(amt),
                        "hizmetZammi" => gelirler.hizmetZammi = Some(amt),
                        "digerGelir" => gelirler.digerGelir = Some(amt),
                        _ => {}
                    }
                }
            }

            // Load deduction items
            let mut ded_stmt = conn.prepare(
                "SELECT item_type, amount FROM payroll_deduction_items WHERE payroll_id = ?1",
            ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

            let ded_rows = ded_stmt.query_map(params![id], |row| {
                let item_type: String = row.get(0)?;
                let amount: i64 = row.get(1)?;
                Ok((item_type, kurus_to_dec(amount)))
            }).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

            let mut kesintiler = KesintiKalemleri::default();
            for r in ded_rows {
                if let Ok((t, amt)) = r {
                    match t.as_str() {
                        "isciSgkPrimi" => kesintiler.isciSgkPrimi = Some(amt),
                        "isciIssizlikPrimi" => kesintiler.isciIssizlikPrimi = Some(amt),
                        "gelirVergisi" => kesintiler.gelirVergisi = Some(amt),
                        "damgaVergisi" => kesintiler.damgaVergisi = Some(amt),
                        "sendikaAidati" => kesintiler.sendikaAidati = Some(amt),
                        "bes" => kesintiler.bes = Some(amt),
                        "icra" => kesintiler.icra = Some(amt),
                        "kisiBorcu" => kesintiler.kisiBorcu = Some(amt),
                        "dogumAskerlikBorclanmasi" => kesintiler.dogumAskerlikBorclanmasi = Some(amt),
                        "hayatSaglikSigortasi" => kesintiler.hayatSaglikSigortasi = Some(amt),
                        "digerKesinti" => kesintiler.digerKesinti = Some(amt),
                        _ => {}
                    }
                }
            }

            result.push(BordroKaydi {
                id,
                personelId: personnel_id,
                donemId: period_id,
                puantajOzeti: puantaj_summary,
                gelirler,
                gelirToplam: kurus_to_dec(gross_total),
                kesintiler,
                kesintiToplam: kurus_to_dec(total_deductions),
                netOdeme: kurus_to_dec(net_payment),
                status,
                olusturulmaTarihi: calculated_at,
                sonGuncellemeTarihi: updated_at,
                notlar: None,
                oncekiKumulatifGvMatrahi: Some(kurus_to_dec(previous_cumulative_gv)),
                oncekiKumulatifAsgariGvMatrahi: None,
                manuelKumulatifGvMatrahi: None,
                devredenPekGelen: devreden_pek_gelen,
                sonrakiDevredenPek: sonraki_devreden_pek,
                pekDetay: pek_detay,
                isPrimiDetay: is_primi_detay,
                odenenRaporluGun: odenen_raporlu_gun,
                raporluGun: raporlu_gun,
            });
        }

        Ok(result)
    }

    pub fn save(conn: &Connection, b: &BordroKaydi) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let status_str = match b.status {
            BordroStatus::DRAFT => "DRAFT",
            BordroStatus::FINALIZED => "FINALIZED",
            _ => "CALCULATED",
        };

        let gross_total = dec_to_kurus(Some(b.gelirToplam));
        let sgk_base = dec_to_kurus(b.pekDetay.as_ref().map(|p| p.finalPek));
        let isci_sgk = b.kesintiler.isciSgkPrimi.unwrap_or_default();
        let isci_issizlik = b.kesintiler.isciIssizlikPrimi.unwrap_or_default();
        let gv_base = dec_to_kurus(Some((b.gelirToplam - isci_sgk - isci_issizlik).max(rust_decimal_macros::dec!(0))));
        let prev_gv = dec_to_kurus(b.oncekiKumulatifGvMatrahi);
        let new_gv = prev_gv + gv_base;
        let income_tax = dec_to_kurus(b.kesintiler.gelirVergisi);
        let stamp_tax = dec_to_kurus(b.kesintiler.damgaVergisi);
        let total_deductions = dec_to_kurus(Some(b.kesintiToplam));
        let net_payment = dec_to_kurus(Some(b.netOdeme));

        let puantaj_summary_json = serde_json::to_string(&b.puantajOzeti).unwrap_or_default();
        let pek_detail_json = b.pekDetay.as_ref().map(|p| serde_json::to_string(p).unwrap_or_default());
        let devreden_pek_gelen_json = b.devredenPekGelen.as_ref().map(|p| serde_json::to_string(p).unwrap_or_default());
        let sonraki_devreden_pek_json = b.sonrakiDevredenPek.as_ref().map(|p| serde_json::to_string(p).unwrap_or_default());
        let is_primi_snapshot_json = b.isPrimiDetay.as_ref().map(|p| serde_json::to_string(p).unwrap_or_default());

        conn.execute(
            "INSERT INTO payroll_records (
                id, personnel_id, period_id, gross_total, sgk_base, gv_base, previous_cumulative_gv,
                new_cumulative_gv, income_tax, stamp_tax, total_deductions, net_payment, status,
                puantaj_summary_json, pek_detail_json, devreden_pek_gelen_json, sonraki_devreden_pek_json,
                raporlu_gun, odenen_raporlu_gun, is_primi_snapshot_json, calculated_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
            ON CONFLICT(personnel_id, period_id) DO UPDATE SET
                gross_total=?4, sgk_base=?5, gv_base=?6, previous_cumulative_gv=?7, new_cumulative_gv=?8,
                income_tax=?9, stamp_tax=?10, total_deductions=?11, net_payment=?12, status=?13,
                puantaj_summary_json=?14, pek_detail_json=?15, devreden_pek_gelen_json=?16,
                sonraki_devreden_pek_json=?17, raporlu_gun=?18, odenen_raporlu_gun=?19,
                is_primi_snapshot_json=?20, updated_at=?22",
            params![
                b.id, b.personelId, b.donemId, gross_total, sgk_base, gv_base, prev_gv, new_gv,
                income_tax, stamp_tax, total_deductions, net_payment, status_str,
                puantaj_summary_json, pek_detail_json, devreden_pek_gelen_json, sonraki_devreden_pek_json,
                b.raporluGun, b.odenenRaporluGun,
                is_primi_snapshot_json,
                now, now
            ],
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        // Replace income items
        conn.execute("DELETE FROM payroll_income_items WHERE payroll_id = ?1", params![b.id])
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let income_map: Vec<(&str, Option<rust_decimal::Decimal>)> = vec![
            ("tabanBrutAylik", b.gelirler.tabanBrutAylik),
            ("tediye", b.gelirler.tediye),
            ("tisIkramiyesi", b.gelirler.tisIkramiyesi),
            ("ekOdeme", b.gelirler.ekOdeme),
            ("yemek", b.gelirler.yemek),
            ("birlestirilmisSosyalYardim", b.gelirler.birlestirilmisSosyalYardim),
            ("vasitaYol", b.gelirler.vasitaYol),
            ("giyimYardimi", b.gelirler.giyimYardimi),
            ("isPrimi", b.gelirler.isPrimi),
            ("geceCalismasiUcreti", b.gelirler.geceCalismasiUcreti),
            ("geceCalismasiTatiliUcreti", b.gelirler.geceCalismasiTatiliUcreti),
            ("hizmetZammi", b.gelirler.hizmetZammi),
            ("digerGelir", b.gelirler.digerGelir),
        ];

        for (item_type, amt_opt) in income_map {
            if let Some(amt) = amt_opt {
                let item_id = format!("{}_{}", b.id, item_type);
                let amt_kurus = dec_to_kurus(Some(amt));
                let src = if item_type == "tediye" || item_type == "tisIkramiyesi" { "MANUAL" } else { "CALCULATED" };
                conn.execute(
                    "INSERT INTO payroll_income_items (id, payroll_id, item_type, description, amount, source)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![item_id, b.id, item_type, item_type, amt_kurus, src],
                ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            }
        }

        // Replace deduction items
        conn.execute("DELETE FROM payroll_deduction_items WHERE payroll_id = ?1", params![b.id])
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let ded_map: Vec<(&str, Option<rust_decimal::Decimal>)> = vec![
            ("isciSgkPrimi", b.kesintiler.isciSgkPrimi),
            ("isciIssizlikPrimi", b.kesintiler.isciIssizlikPrimi),
            ("gelirVergisi", b.kesintiler.gelirVergisi),
            ("damgaVergisi", b.kesintiler.damgaVergisi),
            ("sendikaAidati", b.kesintiler.sendikaAidati),
            ("bes", b.kesintiler.bes),
            ("icra", b.kesintiler.icra),
            ("kisiBorcu", b.kesintiler.kisiBorcu),
            ("dogumAskerlikBorclanmasi", b.kesintiler.dogumAskerlikBorclanmasi),
            ("hayatSaglikSigortasi", b.kesintiler.hayatSaglikSigortasi),
            ("digerKesinti", b.kesintiler.digerKesinti),
        ];

        for (item_type, amt_opt) in ded_map {
            if let Some(amt) = amt_opt {
                let item_id = format!("{}_{}", b.id, item_type);
                let amt_kurus = dec_to_kurus(Some(amt));
                let src = "CALCULATED";
                conn.execute(
                    "INSERT INTO payroll_deduction_items (id, payroll_id, item_type, description, amount, source)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![item_id, b.id, item_type, item_type, amt_kurus, src],
                ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            }
        }

        Ok(())
    }
}
