//! Acute-harm link/domain rating adjustments (idempotent per `judicial_case_id`).

use rust_decimal::Decimal;
use sqlx::{MySql, Transaction};

use super::rating::community_member_delta_to_global_spillover;
use super::types::{
    civic_rating_delta_times, RATING_MULT_CONVERSATION_LOATHED_GLOBAL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcuteHarmDirection {
    Apply,
    Refund,
}

pub async fn apply_or_refund_acute_harm_content_rating_tx(
    tx: &mut Transaction<'_, MySql>,
    judicial_case_id: u64,
    root_post_id: u64,
    direction: AcuteHarmDirection,
) -> Result<bool, sqlx::Error> {
    let dir_key = match direction {
        AcuteHarmDirection::Apply => "apply",
        AcuteHarmDirection::Refund => "refund",
    };

    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM acute_harm_content_rating_event
            WHERE judicial_case_id = ? AND direction = ?
        )
        "#,
    )
    .bind(judicial_case_id)
    .bind(dir_key)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        return Ok(false);
    }

    let row: Option<(Option<u64>, Option<u64>)> = sqlx::query_as(
        r#"
        SELECT p.content_link_id, COALESCE(p.domain_id, cl.domain_id)
        FROM post p
        LEFT JOIN content_link cl ON cl.id = p.content_link_id
        WHERE p.id = ?
        LIMIT 1
        "#,
    )
    .bind(root_post_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((content_link_id, domain_id)) = row else {
        return Ok(false);
    };

    let sign = match direction {
        AcuteHarmDirection::Apply => Decimal::NEGATIVE_ONE,
        AcuteHarmDirection::Refund => Decimal::ONE,
    };
    let link_delta =
        sign * civic_rating_delta_times(RATING_MULT_CONVERSATION_LOATHED_GLOBAL);
    let domain_delta = community_member_delta_to_global_spillover(link_delta);

    if let Some(link_id) = content_link_id {
        sqlx::query(
            r#"
            UPDATE content_link
            SET rating = GREATEST(0.00, LEAST(5.00, rating + ?))
            WHERE id = ?
            "#,
        )
        .bind(link_delta)
        .bind(link_id)
        .execute(&mut **tx)
        .await?;
    }

    if let Some(did) = domain_id {
        sqlx::query(
            r#"
            UPDATE content_domain
            SET rating = GREATEST(0.00, LEAST(5.00, rating + ?))
            WHERE id = ?
            "#,
        )
        .bind(domain_delta)
        .bind(did)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO acute_harm_content_rating_event (
            judicial_case_id, content_link_id, content_domain_id,
            direction, link_delta, domain_delta
        )
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(content_link_id)
    .bind(domain_id)
    .bind(dir_key)
    .bind(link_delta)
    .bind(domain_delta)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}
