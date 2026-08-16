from pathlib import Path

calc_path = Path('src-tauri/src/domain/calculations.rs')
calc = calc_path.read_text()

old_pek = '''pub fn calculate_prime_esas_kazanc(
    gelirler: &GelirKalemleri,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    devreden_pek_gelen: &[DevredenPekKaydi],
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    let raw_prim_gun = puantaj_ozeti.map_or(0, |p| p.c + p.t + p.g + p.i + p.gc + p.gct + p.r);
    let prim_gun_sayisi = min(30, raw_prim_gun.max(0));
'''
new_pek = '''pub fn calculate_prime_esas_kazanc(
    gelirler: &GelirKalemleri,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    devreden_pek_gelen: &[DevredenPekKaydi],
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    calculate_prime_esas_kazanc_with_prim_gun_sayisi(
        gelirler,
        puantaj_ozeti,
        kurum_degerleri,
        devreden_pek_gelen,
        None,
    )
}

fn calculate_prime_esas_kazanc_with_prim_gun_sayisi(
    gelirler: &GelirKalemleri,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    devreden_pek_gelen: &[DevredenPekKaydi],
    sgk_prim_gun_sayisi: Option<i32>,
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    let raw_prim_gun = puantaj_ozeti.map_or(0, |p| p.c + p.t + p.g + p.i + p.gc + p.gct + p.r);
    let prim_gun_sayisi = sgk_prim_gun_sayisi
        .unwrap_or(raw_prim_gun)
        .clamp(0, 30);
'''
if calc.count(old_pek) != 1:
    raise SystemExit(f'PEK function anchor count: {calc.count(old_pek)}')
calc = calc.replace(old_pek, new_pek, 1)

old_sig = '''pub fn calculate_statutory_deductions_with_tax_brackets(
    gelirler: &GelirKalemleri,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    personel: Option<&Personel>,
    puantaj_ozeti: Option<&PuantajOzeti>,
    tax_inputs: &StatutoryDeductionTaxInputs<'_>,
) -> (KesintiKalemleri, PekDetayi, Vec<DevredenPekKaydi>) {
'''
new_sig = '''pub fn calculate_statutory_deductions_with_tax_brackets(
    gelirler: &GelirKalemleri,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    personel: Option<&Personel>,
    puantaj_ozeti: Option<&PuantajOzeti>,
    tax_inputs: &StatutoryDeductionTaxInputs<'_>,
    sgk_prim_gun_sayisi: Option<i32>,
) -> (KesintiKalemleri, PekDetayi, Vec<DevredenPekKaydi>) {
'''
if calc.count(old_sig) != 1:
    raise SystemExit(f'statutory signature anchor count: {calc.count(old_sig)}')
calc = calc.replace(old_sig, new_sig, 1)

old_call = '''        let (pek_detay, sonraki) = calculate_prime_esas_kazanc(
            gelirler,
            puantaj_ozeti,
            kurum_degerleri,
            tax_inputs.incoming_devreden_pek,
        );
'''
new_call = '''        let (pek_detay, sonraki) = calculate_prime_esas_kazanc_with_prim_gun_sayisi(
            gelirler,
            puantaj_ozeti,
            kurum_degerleri,
            tax_inputs.incoming_devreden_pek,
            sgk_prim_gun_sayisi,
        );
'''
if calc.count(old_call) != 1:
    raise SystemExit(f'zero-gross PEK call anchor count: {calc.count(old_call)}')
calc = calc.replace(old_call, new_call, 1)

old_call2 = '''    let (pek_detay, sonraki_devreden) = calculate_prime_esas_kazanc(
        gelirler,
        puantaj_ozeti,
        kurum_degerleri,
        tax_inputs.incoming_devreden_pek,
    );
'''
new_call2 = '''    let (pek_detay, sonraki_devreden) = calculate_prime_esas_kazanc_with_prim_gun_sayisi(
        gelirler,
        puantaj_ozeti,
        kurum_degerleri,
        tax_inputs.incoming_devreden_pek,
        sgk_prim_gun_sayisi,
    );
'''
if calc.count(old_call2) != 1:
    raise SystemExit(f'normal PEK call anchor count: {calc.count(old_call2)}')
calc = calc.replace(old_call2, new_call2, 1)

old_wrapper = '''    calculate_statutory_deductions_with_tax_brackets(
        gelirler,
        kurum_degerleri,
        personel,
        puantaj_ozeti,
        &tax_inputs,
    )
'''
new_wrapper = '''    calculate_statutory_deductions_with_tax_brackets(
        gelirler,
        kurum_degerleri,
        personel,
        puantaj_ozeti,
        &tax_inputs,
        None,
    )
'''
if calc.count(old_wrapper) != 1:
    raise SystemExit(f'legacy wrapper anchor count: {calc.count(old_wrapper)}')
calc = calc.replace(old_wrapper, new_wrapper, 1)
calc_path.write_text(calc)

service_path = Path('src-tauri/src/services/payroll_service.rs')
service = service_path.read_text()

anchor = '''fn hakedis_gun(ozet: &PuantajOzeti) -> i32 {
    ozet.c + ozet.t + ozet.g + ozet.i + ozet.gc + ozet.gct
}

'''
helper = '''fn hakedis_gun(ozet: &PuantajOzeti) -> i32 {
    ozet.c + ozet.t + ozet.g + ozet.i + ozet.gc + ozet.gct
}

/// SGK kamu sektörü 15-14 bordrolarında aylık PEK alt/üst sınırı 30 günlük
/// tutardır. Şubat gibi 28/29 gerçek takvim günü içeren eksiksiz bir dönemde
/// puantaj gerçek tarihlerle tutulmaya devam eder, ancak PEK sınırı 30 gün
/// üzerinden çözülür. Eksik/parsiyel puantaj sessizce 30 güne tamamlanmaz.
fn resolve_sgk_prim_gun_sayisi(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    summary: &PuantajOzeti,
) -> Result<i32> {
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let calendar_day_count = (end - start).num_days() + 1;
    let raw_prim_gun = summary.c
        + summary.t
        + summary.g
        + summary.i
        + summary.gc
        + summary.gct
        + summary.r;

    if calendar_day_count < 30 && attendance.gunler.len() as i64 == calendar_day_count {
        return Ok(30);
    }

    Ok(raw_prim_gun.clamp(0, 30))
}

'''
if service.count(anchor) != 1:
    raise SystemExit(f'hakedis anchor count: {service.count(anchor)}')
service = service.replace(anchor, helper, 1)

old_summary = '''        let mut summary = PuantajOzeti::default();
        for code in attendance.gunler.values() {
            add_puantaj_kodu(&mut summary, code, period_id)?;
        }

        // Calculate exact payable sick dates before any income is produced. A payable date must
'''
new_summary = '''        let mut summary = PuantajOzeti::default();
        for code in attendance.gunler.values() {
            add_puantaj_kodu(&mut summary, code, period_id)?;
        }
        let sgk_prim_gun_sayisi =
            resolve_sgk_prim_gun_sayisi(&attendance, &period, &summary)?;

        // Calculate exact payable sick dates before any income is produced. A payable date must
'''
if service.count(old_summary) != 1:
    raise SystemExit(f'summary anchor count: {service.count(old_summary)}')
service = service.replace(old_summary, new_summary, 1)

old_prod_call = '''            calculate_statutory_deductions_with_tax_brackets(
                &gelirler,
                Some(&effective_kurum_degerleri),
                Some(&personel),
                Some(&summary),
                &tax_inputs,
            );
'''
new_prod_call = '''            calculate_statutory_deductions_with_tax_brackets(
                &gelirler,
                Some(&effective_kurum_degerleri),
                Some(&personel),
                Some(&summary),
                &tax_inputs,
                Some(sgk_prim_gun_sayisi),
            );
'''
if service.count(old_prod_call) != 1:
    raise SystemExit(f'production statutory call anchor count: {service.count(old_prod_call)}')
service = service.replace(old_prod_call, new_prod_call, 1)
service_path.write_text(service)

# Strengthen the existing regression so it directly documents the 30-day SGK cap.
test_path = Path('src-tauri/tests/domain_tests.rs')
test = test_path.read_text()
old_assert = '''        assert_eq!(b3.pekDetay.as_ref().unwrap().finalPek, dec!(297270.00));
        assert!(b3.sonrakiDevredenPek.as_ref().unwrap().is_empty());
'''
new_assert = '''        assert_eq!(b3.pekDetay.as_ref().unwrap().pekUstSinir, dec!(297270.00));
        assert_eq!(b3.pekDetay.as_ref().unwrap().finalPek, dec!(297270.00));
        assert!(b3.sonrakiDevredenPek.as_ref().unwrap().is_empty());
'''
if test.count(old_assert) != 1:
    raise SystemExit(f'February PEK assertion anchor count: {test.count(old_assert)}')
test_path.write_text(test.replace(old_assert, new_assert, 1))
