use crate::domain::{
    CompensationBoundaryScope, CompensationPolicyState, CompensationPolicyVersion, DomainError,
    Result, RoadRoundingMode,
};
use crate::repositories::transaction::with_transaction;
use payroll_core::{
    order_compensation_policy_versions, parse_canonical_work_date,
    validate_compensation_policy_timeline,
};
use rusqlite::{params, Connection, ErrorCode, Params};

pub struct CompensationPolicyRepository;

impl CompensationPolicyRepository {
    fn database_error(error: rusqlite::Error) -> DomainError {
        match &error {
            rusqlite::Error::SqliteFailure(code, _)
                if code.code == ErrorCode::ConstraintViolation =>
            {
                DomainError::InvalidData(format!(
                    "Ücret politikası sürüm çakışması veya immutable kayıt ihlali: {error}"
                ))
            }
            _ => DomainError::DatabaseError(error.to_string()),
        }
    }

    fn read_versions<P: Params>(
        conn: &Connection,
        sql: &str,
        query_params: P,
    ) -> Result<Vec<CompensationPolicyVersion>> {
        let mut statement = conn.prepare(sql).map_err(Self::database_error)?;
        let rows = statement
            .query_map(query_params, |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .map_err(Self::database_error)?;

        let mut versions = Vec::new();
        for row in rows {
            let (
                id,
                effective_from,
                state_json,
                created_at,
                source,
                reason,
                supersedes,
                boundary_scope,
                road_rounding_mode,
            ) = row.map_err(Self::database_error)?;
            let state: CompensationPolicyState =
                serde_json::from_str(&state_json).map_err(|e| {
                    DomainError::InvalidData(format!(
                        "Ücret politikası {id} state_json alanı geçersiz: {e}"
                    ))
                })?;
            let boundary_scope = boundary_scope
                .as_deref()
                .and_then(CompensationBoundaryScope::parse_wire_token)
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "Ücret politikası {id} boundary_scope eksik veya bilinmiyor; kayıt karantinada kalmalı."
                    ))
                })?;
            let road_rounding_mode = road_rounding_mode
                .as_deref()
                .and_then(RoadRoundingMode::parse_wire_token)
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "Ücret politikası {id} road_rounding_mode eksik veya bilinmiyor; kayıt karantinada kalmalı."
                    ))
                })?;
            versions.push(CompensationPolicyVersion {
                id,
                effectiveFrom: effective_from,
                state,
                boundaryScope: boundary_scope,
                roadRoundingMode: road_rounding_mode,
                createdAt: created_at,
                source,
                reason,
                supersedesVersionId: supersedes,
            });
        }
        Ok(versions)
    }

    fn read_and_validate<P: Params>(
        conn: &Connection,
        sql: &str,
        query_params: P,
    ) -> Result<Vec<CompensationPolicyVersion>> {
        let versions = Self::read_versions(conn, sql, query_params)?;
        order_compensation_policy_versions(&versions)
    }

    fn read_all(conn: &Connection) -> Result<Vec<CompensationPolicyVersion>> {
        let versions = Self::read_and_validate(
            conn,
            "SELECT id, effective_from, state_json, created_at, source, reason,
                    supersedes_version_id, boundary_scope, road_rounding_mode
             FROM compensation_policy_versions",
            [],
        )?;
        validate_compensation_policy_timeline(&versions)
    }

    /// Inserts one immutable baseline or same-date correction atomically.
    /// No Tauri command or production service exposes this during Task 2.
    pub fn insert_version(conn: &Connection, version: &CompensationPolicyVersion) -> Result<()> {
        with_transaction(conn, |tx| Self::insert_version_in_transaction(tx, version))
    }

    /// Variant for a caller-owned transaction, including restore and test
    /// composition. The caller remains responsible for commit/rollback.
    pub fn insert_version_in_transaction(
        conn: &Connection,
        version: &CompensationPolicyVersion,
    ) -> Result<()> {
        let mut timeline = Self::read_all(conn)?;
        timeline.push(version.clone());
        validate_compensation_policy_timeline(&timeline)?;

        let state_json = serde_json::to_string(&version.state).map_err(|error| {
            DomainError::InvalidData(format!(
                "Ücret politikası {} state_json üretilemedi: {error}",
                version.id
            ))
        })?;
        conn.execute(
            "INSERT INTO compensation_policy_versions
                (id, effective_from, state_json, created_at, source, reason,
                 supersedes_version_id, boundary_scope, road_rounding_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                version.id,
                version.effectiveFrom,
                state_json,
                version.createdAt,
                version.source,
                version.reason,
                version.supersedesVersionId,
                version.boundaryScope.as_wire_token(),
                version.roadRoundingMode.as_wire_token(),
            ],
        )
        .map_err(Self::database_error)?;
        Ok(())
    }

    /// Returns only the baseline date and active change dates that can affect
    /// this period, with every same-date supersession ancestor needed for pure
    /// graph validation. Payroll, settings and retro histories are not read.
    pub fn get_versions_for_period(
        conn: &Connection,
        start_inclusive: &str,
        end_inclusive: &str,
    ) -> Result<Vec<CompensationPolicyVersion>> {
        let start = parse_canonical_work_date(start_inclusive, "Dönem başlangıç tarihi")?;
        let end = parse_canonical_work_date(end_inclusive, "Dönem bitiş tarihi")?;
        if start > end {
            return Err(DomainError::ValidationError(
                "Dönem başlangıç tarihi bitiş tarihinden sonra olamaz.".into(),
            ));
        }

        Self::read_and_validate(
            conn,
            "WITH RECURSIVE relevant_versions(id) AS (
                 SELECT id
                 FROM compensation_policy_versions
                 WHERE effective_from = (
                     SELECT MAX(effective_from)
                     FROM compensation_policy_versions
                     WHERE effective_from <= ?1
                 )
                 UNION
                 SELECT id
                 FROM compensation_policy_versions
                 WHERE effective_from > ?1 AND effective_from <= ?2
                 UNION
                 SELECT parent.id
                 FROM compensation_policy_versions child
                 JOIN relevant_versions selected ON selected.id = child.id
                 JOIN compensation_policy_versions parent
                   ON parent.id = child.supersedes_version_id
             )
             SELECT policy.id, policy.effective_from, policy.state_json,
                    policy.created_at, policy.source, policy.reason,
                    policy.supersedes_version_id, policy.boundary_scope,
                    policy.road_rounding_mode
             FROM compensation_policy_versions policy
             JOIN relevant_versions relevant ON relevant.id = policy.id",
            params![start_inclusive, end_inclusive],
        )
    }

    /// Exposed only for native schema/repository tests and internal diagnostics.
    pub fn get_all_versions(conn: &Connection) -> Result<Vec<CompensationPolicyVersion>> {
        Self::read_all(conn)
    }

    /// Loads only exact historical IDs referenced by a payroll snapshot.
    /// Supersession ancestors and unrelated experimental rows are not read;
    /// this method validates only the selected records' typed financial data.
    pub fn get_versions_by_ids(
        conn: &Connection,
        ids: &[String],
    ) -> Result<Vec<CompensationPolicyVersion>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (1..=ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, effective_from, state_json, created_at, source, reason,
                    supersedes_version_id, boundary_scope, road_rounding_mode
             FROM compensation_policy_versions
             WHERE id IN ({placeholders})"
        );
        let versions = Self::read_versions(conn, &sql, rusqlite::params_from_iter(ids))?;
        for id in ids {
            if !versions.iter().any(|version| version.id == *id) {
                return Err(DomainError::InvalidData(format!(
                    "Snapshot policy bağlantısı bulunamadı: {id}."
                )));
            }
        }
        Ok(versions)
    }
}
