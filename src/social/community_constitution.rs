//! Effective community constitution: walk the `parent_community_id` chain and merge active
//! `community_constitution_item` rows for each tier (leaf community first, then ancestors).

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySql, MySqlPool};

#[derive(Debug, Clone)]
pub struct ConstitutionClause {
    pub point: u16,
    pub item: String,
}

#[derive(Debug, Clone)]
pub struct OwnedConstitutionClause {
    pub id: u64,
    pub point: u16,
    pub item: String,
}

#[derive(Debug, Clone)]
pub struct ConstitutionSection {
    pub community_handle: String,
    pub community_label: String,
    pub clauses: Vec<ConstitutionClause>,
}

#[derive(Debug, FromRow)]
struct ClauseRow {
    community_handle: String,
    community_label: String,
    point: u16,
    item: String,
}

/// Active constitution clauses for `community_entity_id`, including all ancestors
/// (ordered leaf-first, then each tier's clauses by `point`).
pub async fn fetch_effective_constitution(
    pool: &MySqlPool,
    community_entity_id: u64,
) -> Result<Vec<ConstitutionSection>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ClauseRow>(
        r#"
        WITH RECURSIVE ancestry AS (
            SELECT sc.social_entity_id AS community_id, sc.parent_community_id, 0 AS depth
            FROM social_community sc
            WHERE sc.social_entity_id = ?
            UNION ALL
            SELECT sc.social_entity_id, sc.parent_community_id, a.depth + 1
            FROM social_community sc
            INNER JOIN ancestry a ON sc.social_entity_id = a.parent_community_id
        )
        SELECT
            se.handle AS community_handle,
            COALESCE(NULLIF(TRIM(sc.label), ''), se.handle) AS community_label,
            cci.point,
            cci.item
        FROM ancestry a
        INNER JOIN social_community sc ON sc.social_entity_id = a.community_id
        INNER JOIN social_entity se ON se.id = sc.social_entity_id
        INNER JOIN community_constitution_item cci
            ON cci.community_id = a.community_id
           AND cci.status = 'active'
        ORDER BY a.depth ASC, cci.point ASC, cci.id ASC
        "#,
    )
    .bind(community_entity_id)
    .fetch_all(pool)
    .await?;

    let mut sections: Vec<ConstitutionSection> = Vec::new();
    for row in rows {
        if let Some(last) = sections.last_mut() {
            if last.community_handle == row.community_handle {
                last.clauses.push(ConstitutionClause {
                    point: row.point,
                    item: row.item,
                });
                continue;
            }
        }
        sections.push(ConstitutionSection {
            community_handle: row.community_handle,
            community_label: row.community_label,
            clauses: vec![ConstitutionClause {
                point: row.point,
                item: row.item,
            }],
        });
    }

    Ok(sections)
}

/// Effective constitution if `moving_community_id` were re-parented under `proposed_parent_id`:
/// this community's owned clauses, then the proposed parent's effective chain (ancestors).
pub async fn fetch_effective_constitution_after_reparent(
    pool: &MySqlPool,
    moving_community_id: u64,
    proposed_parent_id: u64,
) -> Result<Vec<ConstitutionSection>, sqlx::Error> {
    let mut sections = Vec::new();
    let owned = fetch_owned_constitution_clauses(pool, moving_community_id).await?;
    if !owned.is_empty() {
        let row: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT se.handle, COALESCE(NULLIF(TRIM(sc.label), ''), se.handle)
            FROM social_entity se
            INNER JOIN social_community sc ON sc.social_entity_id = se.id
            WHERE se.id = ?
            LIMIT 1
            "#,
        )
        .bind(moving_community_id)
        .fetch_optional(pool)
        .await?;
        if let Some((handle, label)) = row {
            sections.push(ConstitutionSection {
                community_handle: handle,
                community_label: label,
                clauses: owned
                    .into_iter()
                    .map(|c| ConstitutionClause {
                        point: c.point,
                        item: c.item,
                    })
                    .collect(),
            });
        }
    }
    sections.extend(
        fetch_effective_constitution(pool, proposed_parent_id).await?,
    );
    Ok(sections)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionClauseDiff {
    pub tier_handle: String,
    pub tier_label: String,
    pub point: u16,
    pub item: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionStructureDiff {
    pub removed: Vec<ConstitutionClauseDiff>,
    pub added: Vec<ConstitutionClauseDiff>,
}

fn clause_keys(sections: &[ConstitutionSection]) -> std::collections::BTreeSet<(String, u16, String)> {
    let mut out = std::collections::BTreeSet::new();
    for section in sections {
        for clause in &section.clauses {
            out.insert((
                section.community_handle.clone(),
                clause.point,
                clause.item.clone(),
            ));
        }
    }
    out
}

#[must_use]
pub fn diff_effective_constitutions(
    before: &[ConstitutionSection],
    after: &[ConstitutionSection],
) -> ConstitutionStructureDiff {
    let before_keys = clause_keys(before);
    let after_keys = clause_keys(after);

    let mut removed = Vec::new();
    for section in before {
        for clause in &section.clauses {
            let key = (
                section.community_handle.clone(),
                clause.point,
                clause.item.clone(),
            );
            if !after_keys.contains(&key) {
                removed.push(ConstitutionClauseDiff {
                    tier_handle: section.community_handle.clone(),
                    tier_label: section.community_label.clone(),
                    point: clause.point,
                    item: clause.item.clone(),
                });
            }
        }
    }

    let mut added = Vec::new();
    for section in after {
        for clause in &section.clauses {
            let key = (
                section.community_handle.clone(),
                clause.point,
                clause.item.clone(),
            );
            if !before_keys.contains(&key) {
                added.push(ConstitutionClauseDiff {
                    tier_handle: section.community_handle.clone(),
                    tier_label: section.community_label.clone(),
                    point: clause.point,
                    item: clause.item.clone(),
                });
            }
        }
    }

    ConstitutionStructureDiff { removed, added }
}

/// Active clauses owned by this community only (not inherited ancestors).
pub async fn fetch_owned_constitution_clauses(
    pool: &MySqlPool,
    community_entity_id: u64,
) -> Result<Vec<OwnedConstitutionClause>, sqlx::Error> {
    #[derive(Debug, FromRow)]
    struct Row {
        id: u64,
        point: u16,
        item: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT cci.id, cci.point, cci.item
        FROM community_constitution_item cci
        WHERE cci.community_id = ?
          AND cci.status = 'active'
        ORDER BY cci.point ASC, cci.id ASC
        "#,
    )
    .bind(community_entity_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| OwnedConstitutionClause {
            id: r.id,
            point: r.point,
            item: r.item,
        })
        .collect())
}

/// Insert the first owned constitutional clause when a community is created.
pub async fn seed_initial_constitution_clause_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    community_entity_id: u64,
    item: &str,
    judicial_trigger: bool,
    acting_user_id: u64,
) -> Result<(), sqlx::Error> {
    let moderation_tier_id = if judicial_trigger {
        crate::ref_key_values::lookup_active_id_conn(
            tx,
            crate::ref_keys::GROUP_JUDICIAL_MODERATION_TIER,
            crate::ref_keys::KEY_JUDICIAL_MODERATION_TIER_REGULAR,
        )
        .await?
    } else {
        None
    };
    sqlx::query(
        r#"
        INSERT INTO community_constitution_item (
            community_id, point, item, status, judicial_trigger_chk, moderation_tier_id,
            created_by, modified_by
        )
        VALUES (?, 1, ?, 'active', ?, ?, ?, ?)
        "#,
    )
    .bind(community_entity_id)
    .bind(item)
    .bind(if judicial_trigger { 1i8 } else { 0i8 })
    .bind(moderation_tier_id)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
