//! UTC-day platform metrics rollup for the Panoply operator daily status email.

use chrono::{NaiveDate, NaiveDateTime};
use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::public_profile;

const PANOPLY_DAILY_DIGEST_KEY: &str = "panoply_daily_status";

const ENTITY_TYPES: &[(&str, &str)] = &[
    ("human", ref_keys::KEY_ENTITY_TYPE_HUMAN),
    ("community", ref_keys::KEY_ENTITY_TYPE_COMMUNITY),
    ("profit", ref_keys::KEY_ENTITY_TYPE_PROFIT),
    ("non_profit", ref_keys::KEY_ENTITY_TYPE_NON_PROFIT),
    ("gathering", ref_keys::KEY_ENTITY_TYPE_GATHERING),
    ("digital", ref_keys::KEY_ENTITY_TYPE_DIGITAL),
];

#[derive(Debug, Clone, Default)]
pub struct EntityTypeDayCounts {
    pub new_today: u32,
    pub retired_today: u32,
    pub active_verified: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PanoplyDailyMetrics {
    pub report_date: String,
    pub human: EntityTypeDayCounts,
    pub community: EntityTypeDayCounts,
    pub profit: EntityTypeDayCounts,
    pub non_profit: EntityTypeDayCounts,
    pub gathering: EntityTypeDayCounts,
    pub digital: EntityTypeDayCounts,
    pub active_verified_humans_total: u32,
    pub judiciary_opened: u32,
    pub judiciary_resolved: u32,
    pub judiciary_unresolved: u32,
    pub posts_new: u32,
    pub posts_resolved: u32,
    pub posts_active_disputed: u32,
    pub operator_actions: u32,
    pub contained_emergencies: u32,
    pub sanitation_events: u32,
    pub dau: u32,
}

/// Report window: full previous UTC calendar day relative to `period_key` (`YYYY-MM-DD`).
#[must_use]
pub fn report_window_for_period_key(period_key: &str) -> Option<(NaiveDateTime, NaiveDateTime, String)> {
    let day = NaiveDate::parse_from_str(period_key.trim(), "%Y-%m-%d").ok()?;
    let report_day = day.pred_opt()?;
    let start = report_day.and_hms_opt(0, 0, 0)?;
    let end = day.and_hms_opt(0, 0, 0)?;
    Some((start, end, report_day.format("%Y-%m-%d").to_string()))
}

pub async fn collect_panoply_daily_metrics(
    pool: &MySqlPool,
    period_key: &str,
) -> Result<Option<PanoplyDailyMetrics>, sqlx::Error> {
    let Some((start, end, report_label)) = report_window_for_period_key(period_key) else {
        return Ok(None);
    };

    let mut metrics = PanoplyDailyMetrics {
        report_date: report_label,
        ..Default::default()
    };

    let alive = public_profile::sql_alive_entity_predicate("se");

    for (field_key, type_key) in ENTITY_TYPES {
        let counts = entity_type_counts(pool, type_key, start, end, &alive).await?;
        match *field_key {
            "human" => metrics.human = counts,
            "community" => metrics.community = counts,
            "profit" => metrics.profit = counts,
            "non_profit" => metrics.non_profit = counts,
            "gathering" => metrics.gathering = counts,
            "digital" => metrics.digital = counts,
            _ => {}
        }
    }

    metrics.active_verified_humans_total = metrics.human.active_verified;
    metrics.judiciary_opened = count_judicial_opened(pool, start, end).await?;
    metrics.judiciary_resolved = count_judicial_resolved(pool, start, end).await?;
    metrics.judiciary_unresolved = count_judicial_unresolved(pool).await?;
    metrics.posts_new = count_posts_new(pool, start, end).await?;
    metrics.posts_resolved = count_posts_judicial_resolved(pool, start, end).await?;
    metrics.posts_active_disputed = count_posts_active_disputed(pool).await?;
    metrics.operator_actions = count_operator_actions(pool, start, end).await?;
    metrics.contained_emergencies = count_contained_emergencies(pool).await?;
    metrics.sanitation_events = count_sanitation_events(pool, start, end).await?;
    metrics.dau = count_dau(pool, start, end).await?;

    Ok(Some(metrics))
}

async fn entity_type_counts(
    pool: &MySqlPool,
    type_key: &str,
    start: NaiveDateTime,
    end: NaiveDateTime,
    alive: &str,
) -> Result<EntityTypeDayCounts, sqlx::Error> {
    let new_today: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        WHERE se.created_at >= ? AND se.created_at < ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(type_key)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;

    let retired_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = se.entity_status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        WHERE se.updated_at >= ? AND se.updated_at < ?
        "#
    );
    let retired_today: i64 = sqlx::query_scalar(&retired_sql)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(type_key)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
        .bind(ref_keys::KEY_ENTITY_STATUS_DEAD)
        .bind(start)
        .bind(end)
        .fetch_one(pool)
        .await?;

    let active_sql = if type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
        format!(
            r#"
            SELECT COUNT(DISTINCT se.id)
            FROM social_entity se
            INNER JOIN ref_key_values rk_type
                ON rk_type.id = se.type_id
               AND rk_type.`group` = ?
               AND rk_type.`key` = ?
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = se.entity_status_id
               AND rk_st.`group` = ?
               AND rk_st.`key` = ?
            INNER JOIN social_human sh ON sh.social_entity_id = se.id
            INNER JOIN user_social_entity_role ur
                ON ur.social_entity_id = se.id
               AND ur.is_primary = 1
               AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
            INNER JOIN users u ON u.id = ur.user_id
            WHERE sh.died_at IS NULL
              AND se.rating > 0.00
              AND u.email_verified_at IS NOT NULL
              AND {alive}
            "#,
            alive = alive
        )
    } else {
        format!(
            r#"
            SELECT COUNT(*)
            FROM social_entity se
            INNER JOIN ref_key_values rk_type
                ON rk_type.id = se.type_id
               AND rk_type.`group` = ?
               AND rk_type.`key` = ?
            INNER JOIN ref_key_values rk_st
                ON rk_st.id = se.entity_status_id
               AND rk_st.`group` = ?
               AND rk_st.`key` = ?
            WHERE se.rating > 0.00
              AND {alive}
            "#,
            alive = alive
        )
    };

    let mut q = sqlx::query_scalar::<_, i64>(&active_sql);
    q = q
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(type_key)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
        .bind(ref_keys::KEY_VERIFIED);
    let active_verified: i64 = q.fetch_one(pool).await?;

    Ok(EntityTypeDayCounts {
        new_today: new_today.max(0) as u32,
        retired_today: retired_today.max(0) as u32,
        active_verified: active_verified.max(0) as u32,
    })
}

async fn count_judicial_opened(
    pool: &MySqlPool,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM judicial_case WHERE opened_at >= ? AND opened_at < ?"#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_judicial_resolved(
    pool: &MySqlPool,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case jc
        INNER JOIN ref_key_values rk
            ON rk.id = jc.status_id
           AND rk.`group` = ?
           AND rk.`key` IN (?, ?, ?)
        WHERE jc.closed_at >= ? AND jc.closed_at < ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
    .bind(ref_keys::KEY_JUDICIAL_CASE_NO_JURY)
    .bind(ref_keys::KEY_JUDICIAL_CASE_VOIDED)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_judicial_unresolved(pool: &MySqlPool) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case jc
        INNER JOIN ref_key_values rk
            ON rk.id = jc.status_id
           AND rk.`group` = ?
           AND rk.`key` IN (?, ?, ?, ?)
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
    .bind(ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_posts_new(
    pool: &MySqlPool,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post p
        WHERE p.root_post_id IS NULL
          AND COALESCE(p.published_at, p.created_at) >= ?
          AND COALESCE(p.published_at, p.created_at) < ?
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_posts_judicial_resolved(
    pool: &MySqlPool,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT COALESCE(p.root_post_id, p.id))
        FROM judicial_case jc
        INNER JOIN post p ON p.id = jc.subject_post_id
        INNER JOIN ref_key_values rk
            ON rk.id = jc.status_id
           AND rk.`group` = ?
           AND rk.`key` IN (?, ?, ?)
        WHERE jc.closed_at >= ? AND jc.closed_at < ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
    .bind(ref_keys::KEY_JUDICIAL_CASE_NO_JURY)
    .bind(ref_keys::KEY_JUDICIAL_CASE_VOIDED)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_posts_active_disputed(pool: &MySqlPool) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post p
        INNER JOIN ref_key_values rk
            ON rk.id = p.status_id
           AND rk.`group` = ?
           AND rk.`key` IN (?, ?, ?)
        WHERE p.root_post_id IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_POST_SUPPRESSED)
    .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
    .bind(ref_keys::KEY_FLAGGED)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_operator_actions(
    pool: &MySqlPool,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM safety_operator_action WHERE created_at >= ? AND created_at < ?"#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_contained_emergencies(pool: &MySqlPool) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_trig
            ON rk_trig.id = jc.trigger_type_id
           AND rk_trig.`group` = 'judicial_case_trigger'
           AND rk_trig.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        "#,
    )
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_sanitation_events(
    pool: &MySqlPool,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM safety_sanitation_event WHERE created_at >= ? AND created_at < ?"#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

async fn count_dau(
    pool: &MySqlPool,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<u32, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT u.id)
        FROM users u
        WHERE u.last_login_at >= ? AND u.last_login_at < ?
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;
    Ok(n.max(0) as u32)
}

pub async fn list_operator_emails(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT DISTINCT u.email
        FROM users u
        INNER JOIN user_social_entity_role ur
            ON ur.user_id = u.id
           AND ur.is_primary = 1
           AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
        INNER JOIN social_entity se_human ON se_human.id = ur.social_entity_id
        INNER JOIN social_entity_relationship ser
            ON ser.from_social_entity_id = se_human.id
           AND ser.ended_at IS NULL
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = ser.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        INNER JOIN social_entity se_pan ON se_pan.id = ser.to_social_entity_id
        WHERE se_pan.handle = ?
          AND u.email_verified_at IS NOT NULL
          AND TRIM(u.email) <> ''
        ORDER BY u.email ASC
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::PANOPLY_COMMUNITY_HANDLE)
    .fetch_all(pool)
    .await
}

/// Claim send rights for this digest + UTC period (idempotent across scheduler re-enqueues).
async fn claim_daily_digest_send(
    pool: &MySqlPool,
    digest_key: &str,
    period_key: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO ops_daily_digest_sent (digest_key, period_key)
        VALUES (?, ?)
        "#,
    )
    .bind(digest_key)
    .bind(period_key)
    .execute(pool)
    .await;
    match result {
        Ok(_) => Ok(true),
        Err(e) if crate::social::db::is_unique_violation(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

pub async fn run_panoply_daily_status_batch(
    pool: &MySqlPool,
    config: &crate::config::Config,
    period_key: &str,
) -> Result<u64, sqlx::Error> {
    let Some(_smtp) = config.smtp.as_ref() else {
        return Ok(0);
    };
    if !config.ops_digest_live {
        tracing::debug!(
            period_key = %period_key,
            "panoply daily status skipped (OPS_DIGEST_LIVE=0)"
        );
        return Ok(0);
    }
    if !claim_daily_digest_send(pool, PANOPLY_DAILY_DIGEST_KEY, period_key).await? {
        return Ok(0);
    }
    let Some(metrics) = collect_panoply_daily_metrics(pool, period_key).await? else {
        return Ok(0);
    };
    let emails = list_operator_emails(pool).await?;
    if emails.is_empty() {
        return Ok(0);
    }
    let mut sent = 0u64;
    for email in emails {
        crate::email::notify::send_panoply_daily_status(config, &email, &metrics).await;
        sent += 1;
    }
    Ok(sent)
}
