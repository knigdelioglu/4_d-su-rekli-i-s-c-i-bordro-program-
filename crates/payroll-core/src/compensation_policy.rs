//! Full-state normal compensation policy validation and timeline resolution.
//!
//! Resolver input order is part of the contract: effective dates ascend, and
//! corrections at one date follow their supersession chain from root to leaf.
//! Durable repositories may first validate a stored graph and request its
//! canonical projection with `order_compensation_policy_versions`.

use crate::errors::DomainError;
use crate::models::{
    BordroKaydi, CompensationBoundaryScope, CompensationPolicyState, CompensationPolicyVersion,
    ResolvedCompensationSegment,
};
use crate::Result;
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap, HashSet};

fn invalid(message: impl Into<String>) -> DomainError {
    DomainError::ValidationError(message.into())
}

fn invalid_graph(message: impl Into<String>) -> DomainError {
    DomainError::InvalidData(message.into())
}

pub fn parse_canonical_work_date(value: &str, field: &str) -> Result<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        invalid(format!(
            "{field} geçerli YYYY-MM-DD çalışma tarihi olmalıdır: {value}"
        ))
    })?;
    if parsed.format("%Y-%m-%d").to_string() != value {
        return Err(invalid(format!(
            "{field} canonical YYYY-MM-DD biçiminde olmalıdır: {value}"
        )));
    }
    Ok(parsed)
}

pub fn validate_compensation_policy_state(state: &CompensationPolicyState) -> Result<()> {
    if state.gunlukTabanUcret <= Decimal::ZERO {
        return Err(invalid("Günlük taban ücret sıfırdan büyük olmalıdır."));
    }
    for (name, value) in [
        ("gunlukYemek", state.gunlukYemek),
        (
            "birlestirilmisSosyalYardim",
            state.birlestirilmisSosyalYardim,
        ),
        ("gunlukVasitaYol", state.gunlukVasitaYol),
        ("giyimYardimi", state.giyimYardimi),
        ("hizmetZammiBirimi", state.hizmetZammiBirimi),
    ] {
        if value < Decimal::ZERO {
            return Err(invalid(format!("{name} negatif olamaz.")));
        }
    }
    for (name, value) in [
        ("isPrimiYuzde", state.isPrimiYuzde),
        ("geceCalismaPrimiYuzde", Some(state.geceCalismaPrimiYuzde)),
        (
            "geceCalismaTatiliPrimiYuzde",
            Some(state.geceCalismaTatiliPrimiYuzde),
        ),
    ] {
        if value.is_some_and(|rate| rate < Decimal::ZERO || rate > Decimal::from(100)) {
            return Err(invalid(format!(
                "{name} yüzde 0 ile 100 arasında olmalıdır."
            )));
        }
    }
    for (name, value) in [
        ("ekOdeme", state.ekOdeme),
        ("digerGelirVarsayilan", state.digerGelirVarsayilan),
    ] {
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(invalid(format!("{name} negatif olamaz.")));
        }
    }
    if state.isPrimiGruplari.is_empty() {
        return Err(invalid("İş primi grupları tam state içinde boş olamaz."));
    }

    let mut identifier_owners: HashMap<&str, usize> = HashMap::new();
    for (group_index, group) in state.isPrimiGruplari.iter().enumerate() {
        if group.id.trim().is_empty()
            || group.ad.trim().is_empty()
            || group.id.trim() != group.id
            || group.ad.trim() != group.ad
            || group.oran < Decimal::ZERO
            || group.oran > Decimal::from(100)
        {
            return Err(invalid(
                "İş primi grubu id/ad alanları boşluksuz olmalı; oran 0 ile 100 arasında olmalıdır.",
            ));
        }
        // Only active groups participate in matching, as in the existing
        // payroll resolver. The same token may be both id and name on one
        // group; ambiguity exists only when another active group owns it.
        if !group.aktif {
            continue;
        }
        for identifier in [&group.id, &group.ad] {
            match identifier_owners.get(identifier.as_str()) {
                Some(owner) if *owner != group_index => {
                    return Err(invalid(format!(
                        "Aktif iş primi gruplarında yinelenen veya belirsiz id/ad: {}.",
                        identifier
                    )))
                }
                Some(_) => {}
                None => {
                    identifier_owners.insert(identifier.as_str(), group_index);
                }
            }
        }
    }
    Ok(())
}

/// Returns whether two complete monetary states differ only in the road
/// amount. Boundary flags never authorize changes to any other state field.
pub fn compensation_states_equal_except_road(
    left: &CompensationPolicyState,
    right: &CompensationPolicyState,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.gunlukVasitaYol = Decimal::ZERO;
    right.gunlukVasitaYol = Decimal::ZERO;
    left == right
}

fn validate_version_fields(version: &CompensationPolicyVersion) -> Result<NaiveDate> {
    if version.id.trim().is_empty() || version.id.trim() != version.id {
        return Err(invalid(
            "Ücret politikası sürüm id alanı boş veya canonical değil.",
        ));
    }
    if version.createdAt.trim().is_empty()
        || version.source.trim().is_empty()
        || version.reason.trim().is_empty()
    {
        return Err(invalid(
            "Ücret politikası createdAt, source ve reason alanları zorunludur.",
        ));
    }
    validate_compensation_policy_state(&version.state)?;
    parse_canonical_work_date(&version.effectiveFrom, "effectiveFrom")
}

/// Validates a whole graph independent of vector order and returns the sole
/// canonical order accepted by the resolver. This is intended for durable
/// storage projections; direct resolver callers must provide that order.
pub fn order_compensation_policy_versions(
    versions: &[CompensationPolicyVersion],
) -> Result<Vec<CompensationPolicyVersion>> {
    let mut by_id = HashMap::with_capacity(versions.len());
    let mut by_date: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, version) in versions.iter().enumerate() {
        validate_version_fields(version)?;
        if by_id.insert(version.id.as_str(), index).is_some() {
            return Err(invalid_graph(format!(
                "Yinelenen ücret politikası sürüm kimliği: {}.",
                version.id
            )));
        }
        by_date
            .entry(version.effectiveFrom.as_str())
            .or_default()
            .push(index);
    }

    let mut child_by_parent = HashMap::new();
    for (index, version) in versions.iter().enumerate() {
        let Some(parent_id) = version.supersedesVersionId.as_deref() else {
            continue;
        };
        if parent_id == version.id {
            return Err(invalid_graph(format!(
                "Ücret politikası sürümü kendisini supersede edemez: {}.",
                version.id
            )));
        }
        let Some(parent_index) = by_id.get(parent_id).copied() else {
            return Err(invalid_graph(format!(
                "Ücret politikası supersedes referansı bulunamadı: {}.",
                parent_id
            )));
        };
        if versions[parent_index].effectiveFrom != version.effectiveFrom {
            return Err(invalid_graph(
                "Ücret politikası düzeltmesi yalnız aynı effectiveFrom tarihindeki sürümü supersede edebilir.",
            ));
        }
        if child_by_parent.insert(parent_index, index).is_some() {
            return Err(invalid_graph(format!(
                "Ücret politikası düzeltme zincirinde fork var: {}.",
                parent_id
            )));
        }
    }

    let mut ordered_indices = Vec::with_capacity(versions.len());
    for (date, indices) in by_date {
        let roots = indices
            .iter()
            .copied()
            .filter(|index| versions[*index].supersedesVersionId.is_none())
            .collect::<Vec<_>>();
        if roots.len() != 1 {
            return Err(invalid_graph(format!(
                "{} tarihinde tek bir baseline/düzeltme zinciri olmalıdır.",
                date
            )));
        }
        let mut cursor = roots[0];
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(cursor) {
                return Err(invalid_graph(format!(
                    "{} tarihinde ücret politikası supersedes döngüsü var.",
                    date
                )));
            }
            ordered_indices.push(cursor);
            match child_by_parent.get(&cursor).copied() {
                Some(child) => cursor = child,
                None => break,
            }
        }
        if visited.len() != indices.len() {
            return Err(invalid_graph(format!(
                "{} tarihinde ücret politikası zincirinde erişilemeyen sürüm veya döngü var.",
                date
            )));
        }
    }

    Ok(ordered_indices
        .into_iter()
        .map(|index| versions[index].clone())
        .collect())
}

/// Validates the complete institution timeline before a durable write or a
/// full-timeline backup. A clipped period projection uses the resolver's
/// separate baseline rule because its earlier state is intentionally absent.
pub fn validate_compensation_policy_timeline(
    versions: &[CompensationPolicyVersion],
) -> Result<Vec<CompensationPolicyVersion>> {
    let ordered = order_compensation_policy_versions(versions)?;
    let mut superseded = HashSet::new();
    for version in &ordered {
        if let Some(parent_id) = version.supersedesVersionId.as_deref() {
            superseded.insert(parent_id);
        }
    }
    let active = ordered
        .iter()
        .filter(|version| !superseded.contains(version.id.as_str()))
        .collect::<Vec<_>>();

    for pair in active.windows(2) {
        let previous = pair[0];
        let current = pair[1];
        if current.boundaryScope == CompensationBoundaryScope::RoadOnly
            && !compensation_states_equal_except_road(&previous.state, &current.state)
        {
            return Err(invalid_graph(format!(
                "{} ROAD_ONLY boundary yalnız gunlukVasitaYol değerini değiştirebilir.",
                current.id
            )));
        }
    }
    Ok(ordered)
}

/// Validates historical snapshot links against immutable policy versions.
/// Superseded versions remain valid historical references; comparison is by
/// exact ID/date/state/flags, never by the current active leaf.
pub fn validate_compensation_snapshot_policy_links(
    payroll: &BordroKaydi,
    versions: &[CompensationPolicyVersion],
) -> Result<()> {
    let Some(snapshot) = payroll
        .calculationMetadata
        .as_ref()
        .and_then(|metadata| metadata.snapshot.as_ref())
    else {
        return Ok(());
    };
    let mut by_id = HashMap::with_capacity(versions.len());
    for version in versions {
        if by_id.insert(version.id.as_str(), version).is_some() {
            return Err(invalid_graph(format!(
                "Snapshot policy link doğrulamasında yinelenen policy version ID: {}.",
                version.id
            )));
        }
    }
    for segment in &snapshot.compensation.segments {
        let version = by_id
            .get(segment.versionId.as_str())
            .copied()
            .ok_or_else(|| {
                invalid_graph(format!(
                    "Bordro {} snapshot policy bağlantısı bulunamadı: {}.",
                    payroll.id, segment.versionId
                ))
            })?;
        if version.effectiveFrom > segment.startDate
            || version.state != segment.state
            || version.boundaryScope != segment.boundaryScope
            || version.roadRoundingMode != segment.roadRoundingMode
        {
            return Err(invalid_graph(format!(
                "Bordro {} snapshot segmenti policy {} ID/tarih/state/boundary metadata'sıyla eşleşmiyor.",
                payroll.id, segment.versionId
            )));
        }
    }
    Ok(())
}

/// Resolves a fully inclusive work-date range using only its date inputs and
/// the supplied policy versions. Input must be ordered by effectiveFrom
/// ascending, with same-date corrections ordered supersedes ancestor first.
pub fn resolve_compensation_segments(
    start_inclusive: &str,
    end_inclusive: &str,
    versions: &[CompensationPolicyVersion],
) -> Result<Vec<ResolvedCompensationSegment>> {
    let start = parse_canonical_work_date(start_inclusive, "Dönem başlangıç tarihi")?;
    let end = parse_canonical_work_date(end_inclusive, "Dönem bitiş tarihi")?;
    if start > end {
        return Err(invalid(
            "Dönem başlangıç tarihi bitiş tarihinden sonra olamaz.",
        ));
    }
    let ordered = order_compensation_policy_versions(versions)?;
    if ordered
        .iter()
        .zip(versions)
        .any(|(canonical, input)| canonical.id != input.id)
    {
        return Err(invalid_graph(
            "Ücret politikası resolver girdisi effectiveFrom artan ve aynı tarih düzeltmeleri supersedes zinciri sırasına uygun olmalıdır.",
        ));
    }

    let mut active_by_date: BTreeMap<&str, &CompensationPolicyVersion> = BTreeMap::new();
    let mut superseded = HashSet::new();
    for version in versions {
        if let Some(parent) = version.supersedesVersionId.as_deref() {
            superseded.insert(parent);
        }
    }
    for version in versions {
        if !superseded.contains(version.id.as_str()) {
            active_by_date.insert(version.effectiveFrom.as_str(), version);
        }
    }

    let baseline = active_by_date
        .range(..=start_inclusive)
        .next_back()
        .map(|(_, version)| *version)
        .ok_or_else(|| {
            DomainError::NotFound(format!(
                "Bu tarih için ücret politikası tanımlı değil: {start_inclusive}."
            ))
        })?;

    let mut selected = vec![baseline];
    selected.extend(
        active_by_date
            .iter()
            .filter(|(date, _)| **date > start_inclusive && **date <= end_inclusive)
            .map(|(_, version)| *version),
    );

    let road_modes = selected
        .iter()
        .map(|version| version.roadRoundingMode)
        .collect::<HashSet<_>>();
    if road_modes.len() != 1 {
        return Err(invalid_graph(
            "Road rounding mode bir bordro hizmet dönemi içinde değişemez.",
        ));
    }
    for pair in selected.windows(2) {
        if pair[1].boundaryScope == CompensationBoundaryScope::RoadOnly
            && !compensation_states_equal_except_road(&pair[0].state, &pair[1].state)
        {
            return Err(invalid_graph(format!(
                "{} ROAD_ONLY boundary yalnız gunlukVasitaYol değerini değiştirebilir.",
                pair[1].id
            )));
        }
    }

    let mut segments = Vec::with_capacity(selected.len());
    for (index, version) in selected.iter().enumerate() {
        let segment_start = if index == 0 {
            start
        } else {
            parse_canonical_work_date(&version.effectiveFrom, "effectiveFrom")?
        };
        let segment_end = if let Some(next) = selected.get(index + 1) {
            parse_canonical_work_date(&next.effectiveFrom, "effectiveFrom")?
                .checked_sub_signed(Duration::days(1))
                .ok_or_else(|| invalid("Ücret politikası segment tarihi sınır dışı."))?
        } else {
            end
        };
        if segment_start > segment_end {
            return Err(invalid_graph(
                "Ücret politikası segmentleri dönemi boş veya çakışan aralıklarla örter.",
            ));
        }
        segments.push(ResolvedCompensationSegment {
            versionId: version.id.clone(),
            startDate: segment_start.format("%Y-%m-%d").to_string(),
            endDate: segment_end.format("%Y-%m-%d").to_string(),
            state: version.state.clone(),
            boundaryScope: version.boundaryScope,
            roadRoundingMode: version.roadRoundingMode,
        });
    }
    Ok(segments)
}
