//! Idempotent ledger for godmother outreach moments.

use sqlx::{MySql, MySqlPool, Transaction};

use super::moment::GodmotherMoment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimOutcome {
    Claimed { ledger_id: u64 },
    AlreadyDelivered,
}

pub async fn moment_already_delivered(
    pool: &MySqlPool,
    human_entity_id: u64,
    moment: GodmotherMoment,
) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM godmother_outreach
            WHERE human_entity_id = ?
              AND moment_key = ?
              AND post_id IS NOT NULL
        )
        "#,
    )
    .bind(human_entity_id)
    .bind(moment.key())
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

/// Backfill welcome ledger from legacy private posts (pre-ledger deploy).
pub async fn backfill_welcome_from_legacy_post(
    pool: &MySqlPool,
    human_entity_id: u64,
    godmother_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let post_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT p.id
        FROM post p
        INNER JOIN post_audience_entity pae ON pae.post_id = p.id
        INNER JOIN post_audience_scope pas ON pas.post_id = p.id AND pas.scope_key = 'private'
        WHERE p.entity_id = ?
          AND pae.audience_entity_id = ?
        ORDER BY p.id ASC
        LIMIT 1
        "#,
    )
    .bind(godmother_entity_id)
    .bind(human_entity_id)
    .fetch_optional(pool)
    .await?;

    let Some(post_id) = post_id else {
        return Ok(false);
    };

    sqlx::query(
        r#"
        INSERT IGNORE INTO godmother_outreach (
            human_entity_id, godmother_entity_id, moment_key, post_id, created_at
        ) VALUES (?, ?, ?, ?, UTC_TIMESTAMP(6))
        "#,
    )
    .bind(human_entity_id)
    .bind(godmother_entity_id)
    .bind(GodmotherMoment::Welcome.key())
    .bind(post_id)
    .execute(pool)
    .await?;

    Ok(true)
}

pub async fn try_claim(
    pool: &MySqlPool,
    human_entity_id: u64,
    godmother_entity_id: u64,
    moment: GodmotherMoment,
) -> Result<ClaimOutcome, sqlx::Error> {
    if moment == GodmotherMoment::Welcome {
        if backfill_welcome_from_legacy_post(pool, human_entity_id, godmother_entity_id).await? {
            return Ok(ClaimOutcome::AlreadyDelivered);
        }
    }

    if moment_already_delivered(pool, human_entity_id, moment).await? {
        return Ok(ClaimOutcome::AlreadyDelivered);
    }

    let res = sqlx::query(
        r#"
        INSERT IGNORE INTO godmother_outreach (
            human_entity_id, godmother_entity_id, moment_key, post_id, created_at
        ) VALUES (?, ?, ?, NULL, UTC_TIMESTAMP(6))
        "#,
    )
    .bind(human_entity_id)
    .bind(godmother_entity_id)
    .bind(moment.key())
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        let unpublished: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM godmother_outreach
                WHERE human_entity_id = ?
                  AND moment_key = ?
                  AND post_id IS NULL
            )
            "#,
        )
        .bind(human_entity_id)
        .bind(moment.key())
        .fetch_one(pool)
        .await?;
        if unpublished {
            return Ok(ClaimOutcome::Claimed { ledger_id: 0 });
        }
        return Ok(ClaimOutcome::AlreadyDelivered);
    }

    Ok(ClaimOutcome::Claimed {
        ledger_id: res.last_insert_id(),
    })
}

pub async fn delete_unpublished_claim(
    pool: &MySqlPool,
    human_entity_id: u64,
    moment: GodmotherMoment,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM godmother_outreach
        WHERE human_entity_id = ?
          AND moment_key = ?
          AND post_id IS NULL
        "#,
    )
    .bind(human_entity_id)
    .bind(moment.key())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_post_id_tx(
    tx: &mut Transaction<'_, MySql>,
    human_entity_id: u64,
    moment: GodmotherMoment,
    post_id: u64,
) -> Result<(), sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE godmother_outreach
        SET post_id = ?
        WHERE human_entity_id = ?
          AND moment_key = ?
          AND post_id IS NULL
        "#,
    )
    .bind(post_id)
    .bind(human_entity_id)
    .bind(moment.key())
    .execute(&mut **tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

pub async fn delete_unpublished_claim_tx(
    tx: &mut Transaction<'_, MySql>,
    human_entity_id: u64,
    moment: GodmotherMoment,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM godmother_outreach
        WHERE human_entity_id = ?
          AND moment_key = ?
          AND post_id IS NULL
        "#,
    )
    .bind(human_entity_id)
    .bind(moment.key())
    .execute(&mut **tx)
    .await?;
    Ok(())
}
