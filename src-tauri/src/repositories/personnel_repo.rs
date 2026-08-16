use super::{opt_dec_to_kurus, opt_kurus_to_dec};
use crate::domain::models::*;
use crate::domain::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};

pub struct PersonnelRepository;

impl PersonnelRepository {
    fn validate(personel: &Personel) -> Result<()> {
        if personel.hizmetYili < 0 {
            return Err(crate::domain::DomainError::ValidationError(
                "Hizmet yılı negatif olamaz.".into(),
            ));
        }
        if personel
            .devirKumulatifGvMatrahi
            .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
            || personel
                .devirKumulatifAsgariGvMatrahi
                .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
        {
            return Err(crate::domain::DomainError::ValidationError(
                "Devir kümülatif GV matrahları negatif olamaz.".into(),
            ));
        }

        if personel
            .devirKumulatifGvMatrahiBaslangicAyi
            .is_some_and(|month| !(1..=12).contains(&month))
        {
            return Err(crate::domain::DomainError::ValidationError(
                "GV devir başlangıç ayı 1-12 arasında olmalıdır.".into(),
            ));
        }

        if let Some(k) = personel.kesintiler.as_ref() {
            let monetary = [
                ("sabitSendikaAidati", k.sabitSendikaAidati),
                ("sabitBesTutar", k.sabitBesTutar),
                ("icraTutar", k.icraTutar),
                ("kisiBorcuTutar", k.kisiBorcuTutar),
                (
                    "dogumAskerlikBorclanmasiTutar",
                    k.dogumAskerlikBorclanmasiTutar,
                ),
                ("hayatSaglikSigortasiTutar", k.hayatSaglikSigortasiTutar),
                ("digerKesintiTutar", k.digerKesintiTutar),
            ];
            for (field, value) in monetary {
                if value.is_some_and(|amount| amount < rust_decimal::Decimal::ZERO) {
                    return Err(crate::domain::DomainError::ValidationError(format!(
                        "Personel kesinti tutarı negatif olamaz: {}.",
                        field
                    )));
                }
            }

            if k.besUyesi.unwrap_or(false) {
                if let Some(rate) = k.oksOraniYuzde {
                    if rate < rust_decimal_macros::dec!(3) || rate > rust_decimal_macros::dec!(100)
                    {
                        return Err(crate::domain::DomainError::ValidationError(
                            "OKS özel oranı, OKS'ye tabi personelde %3-%100 arasında olmalıdır."
                                .into(),
                        ));
                    }
                }
            }

            if let Some(gv) = k.gvIndirimleri.as_ref() {
                for (field, value) in [
                    (
                        "dogumAskerlikGvIndirimTutar",
                        gv.dogumAskerlikGvIndirimTutar,
                    ),
                    ("hayatSigortasiPrimiTutar", gv.hayatSigortasiPrimiTutar),
                    ("saglikSigortasiPrimiTutar", gv.saglikSigortasiPrimiTutar),
                ] {
                    if value.is_some_and(|amount| amount < rust_decimal::Decimal::ZERO) {
                        return Err(crate::domain::DomainError::ValidationError(format!(
                            "GV indirim girdisi negatif olamaz: {}.",
                            field
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Personel> {
        let sendika_uyesi: i32 = row.get(15)?;
        let sabit_sendika: Option<i64> = row.get(16)?;
        let bes_uyesi: i32 = row.get(17)?;
        let oks_orani: Option<i64> = row.get(18)?;
        let sabit_bes: Option<i64> = row.get(19)?;
        let icra: Option<i64> = row.get(20)?;
        let kisi_borcu: Option<i64> = row.get(21)?;
        let dogum: Option<i64> = row.get(22)?;
        let hayat: Option<i64> = row.get(23)?;
        let diger: Option<i64> = row.get(24)?;
        let dogum_gv: Option<i64> = row.get(25)?;
        let hayat_gv: Option<i64> = row.get(26)?;
        let saglik_gv: Option<i64> = row.get(27)?;

        Ok(Personel {
            id: row.get(0)?,
            tcNo: row.get(1)?,
            ad: row.get(2)?,
            soyad: row.get(3)?,
            grup: row.get(4)?,
            unvan: row.get(5)?,
            sgkSicilNo: row.get(6)?,
            iban: row.get(7)?,
            hizmetYili: row.get(8)?,
            aciklama: row.get(9)?,
            devirKumulatifGvMatrahi: opt_kurus_to_dec(row.get(10)?),
            devirKumulatifGvMatrahiYili: row.get(11)?,
            devirKumulatifGvMatrahiBaslangicAyi: row.get(12)?,
            devirKumulatifAsgariGvMatrahi: opt_kurus_to_dec(row.get(13)?),
            devirKumulatifAsgariGvMatrahiYili: row.get(14)?,
            kesintiler: Some(PersonelKesintileri {
                sendikaUyesi: Some(sendika_uyesi == 1),
                sabitSendikaAidati: opt_kurus_to_dec(sabit_sendika),
                besUyesi: Some(bes_uyesi == 1),
                oksOraniYuzde: opt_kurus_to_dec(oks_orani),
                sabitBesTutar: opt_kurus_to_dec(sabit_bes),
                icraTutar: opt_kurus_to_dec(icra),
                kisiBorcuTutar: opt_kurus_to_dec(kisi_borcu),
                dogumAskerlikBorclanmasiTutar: opt_kurus_to_dec(dogum),
                hayatSaglikSigortasiTutar: opt_kurus_to_dec(hayat),
                digerKesintiTutar: opt_kurus_to_dec(diger),
                gvIndirimleri: Some(GvIndirimGirdileri {
                    dogumAskerlikGvIndirimTutar: opt_kurus_to_dec(dogum_gv),
                    hayatSigortasiPrimiTutar: opt_kurus_to_dec(hayat_gv),
                    saglikSigortasiPrimiTutar: opt_kurus_to_dec(saglik_gv),
                }),
            }),
        })
    }

    pub fn get_all(conn: &Connection) -> Result<Vec<Personel>> {
        let mut stmt = conn.prepare(
            "SELECT id, tc_no, ad, soyad, grup, unvan, sgk_sicil_no, iban, hizmet_yili, aciklama,
                    devir_kumulatif_gv_matrahi, devir_kumulatif_gv_matrahi_yili,
                    devir_kumulatif_gv_matrahi_baslangic_ayi, devir_kumulatif_asgari_gv_matrahi,
                    devir_kumulatif_asgari_gv_matrahi_yili, sendika_uyesi, sabit_sendika_aidati,
                    bes_uyesi, oks_orani_yuzde, sabit_bes_tutar, icra_tutar, kisi_borcu_tutar,
                    dogum_askerlik_borclanmasi_tutar, hayat_saglik_sigortasi_tutar, diger_kesinti_tutar,
                    dogum_askerlik_gv_indirim_tutar, hayat_sigortasi_gv_prim_tutar, saglik_sigortasi_gv_prim_tutar
             FROM personnel ORDER BY ad ASC, soyad ASC",
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], Self::from_row)
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Personel>> {
        conn.query_row(
            "SELECT id, tc_no, ad, soyad, grup, unvan, sgk_sicil_no, iban, hizmet_yili, aciklama,
                    devir_kumulatif_gv_matrahi, devir_kumulatif_gv_matrahi_yili,
                    devir_kumulatif_gv_matrahi_baslangic_ayi, devir_kumulatif_asgari_gv_matrahi,
                    devir_kumulatif_asgari_gv_matrahi_yili, sendika_uyesi, sabit_sendika_aidati,
                    bes_uyesi, oks_orani_yuzde, sabit_bes_tutar, icra_tutar, kisi_borcu_tutar,
                    dogum_askerlik_borclanmasi_tutar, hayat_saglik_sigortasi_tutar, diger_kesinti_tutar,
                    dogum_askerlik_gv_indirim_tutar, hayat_sigortasi_gv_prim_tutar, saglik_sigortasi_gv_prim_tutar
             FROM personnel WHERE id = ?1",
            params![id],
            Self::from_row,
        )
        .optional()
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))
    }

    pub fn save(conn: &Connection, p: &Personel) -> Result<()> {
        Self::validate(p)?;
        let now = Utc::now().to_rfc3339();
        let k = p.kesintiler.as_ref();

        let sendika_uyesi = if k.and_then(|k| k.sendikaUyesi).unwrap_or(false) {
            1
        } else {
            0
        };
        let bes_uyesi = if k.and_then(|k| k.besUyesi).unwrap_or(false) {
            1
        } else {
            0
        };

        let sabit_sendika = opt_dec_to_kurus(k.and_then(|k| k.sabitSendikaAidati))?;
        let oks_orani = opt_dec_to_kurus(k.and_then(|k| k.oksOraniYuzde))?;
        let sabit_bes = opt_dec_to_kurus(k.and_then(|k| k.sabitBesTutar))?;
        let icra = opt_dec_to_kurus(k.and_then(|k| k.icraTutar))?;
        let kisi_borcu = opt_dec_to_kurus(k.and_then(|k| k.kisiBorcuTutar))?;
        let dogum = opt_dec_to_kurus(k.and_then(|k| k.dogumAskerlikBorclanmasiTutar))?;
        let hayat = opt_dec_to_kurus(k.and_then(|k| k.hayatSaglikSigortasiTutar))?;
        let diger = opt_dec_to_kurus(k.and_then(|k| k.digerKesintiTutar))?;
        let gv = k.and_then(|k| k.gvIndirimleri.as_ref());
        let dogum_gv_indirim = opt_dec_to_kurus(gv.and_then(|g| g.dogumAskerlikGvIndirimTutar))?;
        let hayat_gv_prim = opt_dec_to_kurus(gv.and_then(|g| g.hayatSigortasiPrimiTutar))?;
        let saglik_gv_prim = opt_dec_to_kurus(gv.and_then(|g| g.saglikSigortasiPrimiTutar))?;

        // Aşağıdaki dal, migration'ları henüz tamamlanmamış eski bir SQLite
        // dosyasını açan bakım/test akışları için geriye dönük uyumludur.
        let has_devir_columns: bool = conn
            .prepare("PRAGMA table_info(personnel)")
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
                let mut found = false;
                for row in rows {
                    if row.map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                        == "devir_kumulatif_gv_matrahi"
                    {
                        found = true;
                        break;
                    }
                }
                Ok(found)
            })
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        if !has_devir_columns {
            conn.execute(
                "INSERT INTO personnel (
                    id, tc_no, ad, soyad, grup, unvan, sgk_sicil_no, iban, hizmet_yili, aciklama,
                    sendika_uyesi, sabit_sendika_aidati, bes_uyesi, oks_orani_yuzde, sabit_bes_tutar,
                    icra_tutar, kisi_borcu_tutar, dogum_askerlik_borclanmasi_tutar, hayat_saglik_sigortasi_tutar,
                    diger_kesinti_tutar, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
                ON CONFLICT(id) DO UPDATE SET
                    tc_no=?2, ad=?3, soyad=?4, grup=?5, unvan=?6, sgk_sicil_no=?7, iban=?8, hizmet_yili=?9, aciklama=?10,
                    sendika_uyesi=?11, sabit_sendika_aidati=?12, bes_uyesi=?13, oks_orani_yuzde=?14, sabit_bes_tutar=?15,
                    icra_tutar=?16, kisi_borcu_tutar=?17, dogum_askerlik_borclanmasi_tutar=?18, hayat_saglik_sigortasi_tutar=?19,
                    diger_kesinti_tutar=?20, updated_at=?22",
                params![
                    p.id, p.tcNo, p.ad, p.soyad, p.grup, p.unvan, p.sgkSicilNo, p.iban, p.hizmetYili, p.aciklama,
                    sendika_uyesi, sabit_sendika, bes_uyesi, oks_orani, sabit_bes,
                    icra, kisi_borcu, dogum, hayat, diger, now, now
                ],
            )
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            return Ok(());
        }

        conn.execute(
            "INSERT INTO personnel (
                id, tc_no, ad, soyad, grup, unvan, sgk_sicil_no, iban, hizmet_yili, aciklama,
                devir_kumulatif_gv_matrahi, devir_kumulatif_gv_matrahi_yili,
                devir_kumulatif_gv_matrahi_baslangic_ayi, devir_kumulatif_asgari_gv_matrahi,
                devir_kumulatif_asgari_gv_matrahi_yili, sendika_uyesi, sabit_sendika_aidati,
                bes_uyesi, oks_orani_yuzde, sabit_bes_tutar, icra_tutar, kisi_borcu_tutar,
                dogum_askerlik_borclanmasi_tutar, hayat_saglik_sigortasi_tutar, diger_kesinti_tutar,
                dogum_askerlik_gv_indirim_tutar, hayat_sigortasi_gv_prim_tutar, saglik_sigortasi_gv_prim_tutar,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30)
            ON CONFLICT(id) DO UPDATE SET
                tc_no=?2, ad=?3, soyad=?4, grup=?5, unvan=?6, sgk_sicil_no=?7, iban=?8, hizmet_yili=?9, aciklama=?10,
                devir_kumulatif_gv_matrahi=?11, devir_kumulatif_gv_matrahi_yili=?12,
                devir_kumulatif_gv_matrahi_baslangic_ayi=?13, devir_kumulatif_asgari_gv_matrahi=?14,
                devir_kumulatif_asgari_gv_matrahi_yili=?15, sendika_uyesi=?16, sabit_sendika_aidati=?17,
                bes_uyesi=?18, oks_orani_yuzde=?19, sabit_bes_tutar=?20, icra_tutar=?21,
                kisi_borcu_tutar=?22, dogum_askerlik_borclanmasi_tutar=?23, hayat_saglik_sigortasi_tutar=?24,
                diger_kesinti_tutar=?25, dogum_askerlik_gv_indirim_tutar=?26, hayat_sigortasi_gv_prim_tutar=?27,
                saglik_sigortasi_gv_prim_tutar=?28, updated_at=?30",
            params![
                p.id, p.tcNo, p.ad, p.soyad, p.grup, p.unvan, p.sgkSicilNo, p.iban, p.hizmetYili, p.aciklama,
                opt_dec_to_kurus(p.devirKumulatifGvMatrahi)?, p.devirKumulatifGvMatrahiYili,
                p.devirKumulatifGvMatrahiBaslangicAyi, opt_dec_to_kurus(p.devirKumulatifAsgariGvMatrahi)?,
                p.devirKumulatifAsgariGvMatrahiYili, sendika_uyesi, sabit_sendika, bes_uyesi, oks_orani, sabit_bes,
                icra, kisi_borcu, dogum, hayat, diger, dogum_gv_indirim, hayat_gv_prim, saglik_gv_prim, now, now
            ],
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        conn.execute("DELETE FROM personnel WHERE id = ?1", params![id])
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
