use super::*;

#[derive(Clone)]
struct DatedPolicySlice<'a> {
    state: &'a CompensationPolicyState,
    start: NaiveDate,
    end: NaiveDate,
}

pub(super) struct CompensationTimeline {
    segments: Vec<ResolvedCompensationSegment>,
    road_rounding: RoadRoundingMode,
}

impl CompensationTimeline {
    pub(super) fn resolved_segments(&self) -> &[ResolvedCompensationSegment] {
        &self.segments
    }

    #[cfg(test)]
    pub(super) fn earning_slice_count(&self) -> Result<usize> {
        Ok(self.earning_slices()?.len())
    }

    fn period_end_state(&self) -> Result<&CompensationPolicyState> {
        self.segments
            .last()
            .map(|segment| &segment.state)
            .ok_or_else(|| DomainError::NotFound("Ücret politikası baseline'ı bulunamadı.".into()))
    }

    fn slices(&self) -> Result<Vec<DatedPolicySlice<'_>>> {
        dated_policy_slices(&self.segments)
    }

    /// Full earning boundaries always start a new rounding slice, even when
    /// all rates match. A typed ROAD_ONLY boundary changes only the road rate
    /// and merges its interval into the surrounding earning slice.
    fn earning_slices(&self) -> Result<Vec<DatedPolicySlice<'_>>> {
        let mut earning_slices: Vec<DatedPolicySlice<'_>> = Vec::new();
        for (index, (segment, slice)) in self.segments.iter().zip(self.slices()?).enumerate() {
            if index > 0 && segment.boundaryScope == CompensationBoundaryScope::RoadOnly {
                let previous = earning_slices.last_mut().ok_or_else(|| {
                    DomainError::InvalidData(
                        "İlk policy segmenti yol-only ara sınır olamaz.".into(),
                    )
                })?;
                previous.end = slice.end;
            } else {
                earning_slices.push(slice);
            }
        }
        Ok(earning_slices)
    }

    fn road_rate_on(&self, slices: &[DatedPolicySlice<'_>], date: NaiveDate) -> Result<Decimal> {
        slices
            .iter()
            .find(|slice| date >= slice.start && date <= slice.end)
            .map(|slice| slice.state.gunlukVasitaYol)
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} tarihinde road state'i çözümlenemedi.",
                    date.format("%Y-%m-%d")
                ))
            })
    }
}

pub(super) fn resolve_compensation_timeline(
    period: &BordroDonemi,
    segments: Vec<ResolvedCompensationSegment>,
) -> Result<CompensationTimeline> {
    if segments.is_empty() {
        return Err(DomainError::NotFound(format!(
            "{} dönemi için ücret politikası baseline'ı bulunamadı.",
            period.id
        )));
    }

    let road_rounding = segments[0].roadRoundingMode;

    Ok(CompensationTimeline {
        segments,
        road_rounding,
    })
}

pub(super) fn resolve_timeline_for_period(
    period: &BordroDonemi,
    versions: &[CompensationPolicyVersion],
) -> Result<CompensationTimeline> {
    let segments = crate::resolve_compensation_segments(
        &period.baslangicTarihi,
        &period.bitisTarihi,
        versions,
    )?;
    resolve_compensation_timeline(period, segments)
}

pub(super) fn policy_date(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        DomainError::InvalidData(format!(
            "Ücret politikası tarihi geçersiz: {value} ({error})."
        ))
    })
}

fn dated_policy_slices<'a>(
    segments: &'a [ResolvedCompensationSegment],
) -> Result<Vec<DatedPolicySlice<'a>>> {
    segments
        .iter()
        .map(|segment| {
            let start = policy_date(&segment.startDate)?;
            let end = policy_date(&segment.endDate)?;
            if start > end {
                return Err(DomainError::InvalidData(format!(
                    "Ücret politikası segment aralığı ters: {}..{}.",
                    segment.startDate, segment.endDate
                )));
            }
            Ok(DatedPolicySlice {
                state: &segment.state,
                start,
                end,
            })
        })
        .collect()
}

fn attendance_dates(attendance: &PersonelPuantaj) -> Result<Vec<(NaiveDate, &str)>> {
    attendance
        .gunler
        .iter()
        .map(|(date, code)| Ok((policy_date(date)?, code.as_str())))
        .collect()
}

fn summary_in_range(
    dates: &[(NaiveDate, &str)],
    start: NaiveDate,
    end: NaiveDate,
    period_id: &str,
) -> Result<PuantajOzeti> {
    let mut summary = PuantajOzeti::default();
    for (date, code) in dates {
        if *date >= start && *date <= end {
            add_puantaj_kodu(&mut summary, code, period_id)?;
        }
    }
    Ok(summary)
}

fn earned_days(summary: &PuantajOzeti) -> i32 {
    summary.c + summary.t + summary.g + summary.i + summary.gc + summary.gct
}

fn meal_days(summary: &PuantajOzeti) -> i32 {
    summary.c + summary.gc
}

fn service_days(start: NaiveDate, end: NaiveDate) -> Result<i64> {
    end.signed_duration_since(start)
        .num_days()
        .checked_add(1)
        .filter(|days| *days > 0)
        .ok_or_else(|| DomainError::InvalidData("Ücret politikası takvim aralığı geçersiz.".into()))
}

fn legacy_group_items(state: &CompensationPolicyState) -> Vec<IsPrimiGrupItem> {
    state
        .isPrimiGruplari
        .iter()
        .map(|group| IsPrimiGrupItem {
            id: group.id.clone(),
            ad: group.ad.clone(),
            oran: group.oran,
            aktif: group.aktif,
        })
        .collect()
}

fn policy_premium_detail(
    state: &CompensationPolicyState,
    personnel_group: &str,
    earning_days: i32,
) -> Result<IsPrimiHesapDetayi> {
    let groups = legacy_group_items(state);
    calculate_is_primi_detayi(
        state.gunlukTabanUcret,
        earning_days,
        Some(personnel_group),
        Some(&groups),
    )
}

pub(super) fn calculate_policy_income(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    timeline: &CompensationTimeline,
    paid_sick_dates: &[NaiveDate],
    service_year: i32,
    personnel_group: &str,
) -> Result<(GelirKalemleri, IsPrimiHesapDetayi)> {
    let all_slices = timeline.slices()?;
    let slices = timeline.earning_slices()?;
    let dates = attendance_dates(attendance)?;
    let mut income = GelirKalemleri::default();

    let mut base = Some(Decimal::ZERO);
    for slice in &slices {
        let summary = summary_in_range(&dates, slice.start, slice.end, &period.id)?;
        let days = earned_days(&summary);
        base = sum_income_field(
            base,
            Some(round2(Decimal::from(days) * slice.state.gunlukTabanUcret)),
        );
        let sick_days = paid_sick_dates
            .iter()
            .filter(|date| **date >= slice.start && **date <= slice.end)
            .count() as i32;
        add_paid_sick_wage(&mut base, sick_days, slice.state.gunlukTabanUcret);
    }
    income.tabanBrutAylik = base;

    let mut meal = Decimal::ZERO;
    for slice in &slices {
        let summary = summary_in_range(&dates, slice.start, slice.end, &period.id)?;
        meal = round2(meal + calculate_meal_income(meal_days(&summary), slice.state.gunlukYemek));
    }
    income.yemek = Some(meal);

    let road = if timeline.road_rounding == RoadRoundingMode::PeriodTotal {
        let mut total = Decimal::ZERO;
        for (date, code) in &dates {
            if matches!(*code, "Ç" | "GÇ") {
                total += timeline.road_rate_on(&all_slices, *date)?;
            }
        }
        round2(total)
    } else {
        let mut rounded_total = Decimal::ZERO;
        for slice in &slices {
            let mut slice_total = Decimal::ZERO;
            for (date, code) in &dates {
                if *date >= slice.start && *date <= slice.end && matches!(*code, "Ç" | "GÇ") {
                    slice_total += timeline.road_rate_on(&all_slices, *date)?;
                }
            }
            rounded_total = round2(rounded_total + round2(slice_total));
        }
        rounded_total
    };
    income.vasitaYol = Some(road);

    let mut premium_detail = None;
    for slice in &slices {
        let summary = summary_in_range(&dates, slice.start, slice.end, &period.id)?;
        let detail = policy_premium_detail(slice.state, personnel_group, meal_days(&summary))?;
        premium_detail = Some(match premium_detail {
            Some(previous) => super::merge_is_primi_details(&previous, &detail),
            None => detail,
        });
    }
    let premium_detail = premium_detail.ok_or_else(|| {
        DomainError::InvalidData("İş primi için aktif personel grubu çözümlenemedi.".into())
    })?;
    income.isPrimi = Some(premium_detail.tutar);

    let mut night = Decimal::ZERO;
    for slice in &slices {
        let summary = summary_in_range(&dates, slice.start, slice.end, &period.id)?;
        night = round2(
            night
                + NightWorkPolicy::calculate_gece_calismasi_primi(
                    slice.state.gunlukTabanUcret,
                    slice.state.geceCalismaPrimiYuzde,
                    summary.gc,
                ),
        );
    }
    income.geceCalismasiUcreti = Some(night);

    let mut night_holiday = Decimal::ZERO;
    for slice in &slices {
        let summary = summary_in_range(&dates, slice.start, slice.end, &period.id)?;
        night_holiday = round2(
            night_holiday
                + NightWorkPolicy::calculate_gece_calismasi_tatili_primi(
                    slice.state.gunlukTabanUcret,
                    slice.state.geceCalismaTatiliPrimiYuzde,
                    summary.gct,
                ),
        );
    }
    income.geceCalismasiTatiliUcreti = Some(night_holiday);

    let period_start = policy_date(&period.baslangicTarihi)?;
    let period_end = policy_date(&period.bitisTarihi)?;
    let total_calendar_days = Decimal::from(service_days(period_start, period_end)?);
    let weighted = |amount_for: &dyn Fn(&CompensationPolicyState) -> Decimal| -> Result<Decimal> {
        let mut amount = Decimal::ZERO;
        for slice in &all_slices {
            amount +=
                amount_for(slice.state) * Decimal::from(service_days(slice.start, slice.end)?);
        }
        Ok(round2(amount / total_calendar_days))
    };
    income.birlestirilmisSosyalYardim = Some(weighted(&|state| state.birlestirilmisSosyalYardim)?);
    income.giyimYardimi = Some(weighted(&|state| state.giyimYardimi)?);
    income.hizmetZammi = Some(weighted(&|state| {
        Decimal::from(service_year) * state.hizmetZammiBirimi
    })?);
    let period_end_state = timeline.period_end_state()?;
    income.ekOdeme = period_end_state.ekOdeme.map(round2);
    income.digerGelir = period_end_state.digerGelirVarsayilan.map(round2);

    Ok((income, premium_detail))
}

pub(super) fn calculate_policy_meal_exemptions(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    statutory_snapshot: &ResolvedStatutorySnapshot,
    timeline: &CompensationTimeline,
) -> Result<MealExemptionTotals> {
    let slices = timeline.earning_slices()?;
    let dates = attendance_dates(attendance)?;
    let statutory_ranges = statutory_snapshot
        .segments
        .iter()
        .map(|statutory| {
            Ok((
                policy_date(&statutory.effectiveFrom)?,
                policy_date(&statutory.effectiveTo)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut meal_days_by_statutory = vec![0; statutory_snapshot.segments.len()];
    let mut totals = MealExemptionTotals::default();

    // Each earning slice is an independently rounded payroll amount. Statutory
    // boundaries split its legal caps, but must not round the same payment
    // again and create more exempt meal value than was actually paid.
    for compensation_slice in &slices {
        let mut earning_segments = Vec::new();
        for (statutory_index, statutory) in statutory_snapshot.segments.iter().enumerate() {
            let (statutory_start, statutory_end) = statutory_ranges[statutory_index];
            let start = compensation_slice.start.max(statutory_start);
            let end = compensation_slice.end.min(statutory_end);
            if start > end {
                continue;
            }
            let summary = summary_in_range(&dates, start, end, &period.id)?;
            let days = meal_days(&summary);
            meal_days_by_statutory[statutory_index] += days;
            if days > 0 {
                earning_segments.push(MealExemptionSegment {
                    actual: Decimal::from(days) * compensation_slice.state.gunlukYemek,
                    sgk_capacity: statutory.gunlukYemekIstisnasiSGK * Decimal::from(days),
                    gv_capacity: statutory.gunlukYemekIstisnasiGV * Decimal::from(days),
                });
            }
        }

        let slice_totals = calculate_segmented_meal_exemptions(&earning_segments);
        totals.sgk = round2(totals.sgk + slice_totals.sgk);
        totals.gv = round2(totals.gv + slice_totals.gv);
    }

    for (index, statutory) in statutory_snapshot.segments.iter().enumerate() {
        if meal_days_by_statutory[index] != statutory.fiiliYemekGunu {
            return Err(DomainError::InvalidData(format!(
                "{} döneminde statutory yemek günü ile policy meal günü eşleşmiyor ({} / {}).",
                period.id, statutory.fiiliYemekGunu, meal_days_by_statutory[index]
            )));
        }
    }
    Ok(totals)
}

/// Replays a NORMAL entitlement directly from its validated persisted
/// compensation segments and used inputs. Retro replay passes either the
/// saved segments or a derived target-state copy; it never fabricates policy
/// version rows or reconstructs a legacy request.
pub(super) fn replay_snapshot_income(
    payroll: &BordroKaydi,
    period: &BordroDonemi,
    snapshot: &PayrollCalculationSnapshot,
    segments: &[ResolvedCompensationSegment],
) -> Result<(GelirKalemleri, MealExemptionTotals, bool)> {
    let inputs = &snapshot.usedInputs;
    let attendance_input = inputs.attendance.as_ref().ok_or_else(|| {
        DomainError::InvalidData("Persisted NORMAL snapshot has no attendance input.".into())
    })?;
    let attendance = PersonelPuantaj {
        id: format!("{}_{}", payroll.personelId, period.id),
        personelId: payroll.personelId.clone(),
        donemId: period.id.clone(),
        gunler: attendance_input
            .days
            .iter()
            .map(|day| (day.date.clone(), day.code.clone()))
            .collect(),
    };
    let paid_leave = inputs.paidSickLeave.as_ref().ok_or_else(|| {
        DomainError::InvalidData("Persisted NORMAL snapshot has no paid sick-leave input.".into())
    })?;
    let paid_dates = paid_leave
        .paidDates
        .iter()
        .map(|date| policy_date(date))
        .collect::<Result<Vec<_>>>()?;
    let timeline = resolve_compensation_timeline(period, segments.to_vec())?;
    let (mut income, _) = calculate_policy_income(
        &attendance,
        period,
        &timeline,
        &paid_dates,
        inputs.serviceYear,
        &inputs.personnelGroup,
    )?;
    let manual = &inputs.manualIncome;
    let manual_request = manual.supplied.then_some(ManualPayrollIncomeInput {
        tediye: manual.tediye,
        tisIkramiyesi: manual.tisIkramiyesi,
    });
    super::apply_manual_payroll_income(&mut income, manual_request.as_ref())?;
    let statutory = payroll.statutorySnapshot.as_ref().ok_or_else(|| {
        DomainError::InvalidData(
            "Persisted NORMAL snapshot has no resolved statutory output.".into(),
        )
    })?;
    let meal_exemption =
        calculate_policy_meal_exemptions(&attendance, period, statutory, &timeline)?;
    Ok((
        income,
        meal_exemption,
        inputs
            .institutionDeductionSettings
            .yemekTamGvIstisnaDamgaTabi,
    ))
}
