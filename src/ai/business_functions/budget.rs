//! Per-councillor UTC-day budgets: `B = max(1, ceil(√(√N)))`; replies use `max(9, B)`; reply credit amortized over 96 ticks/day.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use super::population::count_active_verified_humans;

const REPLY_TICKS_PER_DAY: u32 = 96;

/// Minimum replies per councillor per UTC day (floor on √√N budget).
pub const REPLY_BUDGET_FLOOR: u64 = 9;

/// Daily action budget from live human population `N`.
#[must_use]
pub fn daily_budget_b(n: u64) -> u64 {
    if n == 0 {
        return 1;
    }
    let root_n = (n as f64).sqrt();
    let root_root = root_n.sqrt();
    root_root.ceil().max(1.0) as u64
}

/// Reply cap per UTC day: `max(REPLY_BUDGET_FLOOR, daily_budget_b(N))`.
#[must_use]
pub fn daily_reply_budget_b(n: u64) -> u64 {
    daily_budget_b(n).max(REPLY_BUDGET_FLOOR)
}

pub async fn daily_budget_b_live(pool: &MySqlPool) -> Result<u64, sqlx::Error> {
    let n = count_active_verified_humans(pool).await?;
    Ok(daily_budget_b(n))
}

pub async fn daily_reply_budget_b_live(pool: &MySqlPool) -> Result<u64, sqlx::Error> {
    let n = count_active_verified_humans(pool).await?;
    Ok(daily_reply_budget_b(n))
}

fn utc_today() -> NaiveDate {
    Utc::now().date_naive()
}

pub async fn ensure_budget_row(
    pool: &MySqlPool,
    ai_actor_id: u64,
    utc_date: NaiveDate,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO council_ai_daily_budget (ai_actor_id, `utc_date`)
        VALUES (?, ?)
        ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)
        "#,
    )
    .bind(ai_actor_id)
    .bind(utc_date)
    .execute(pool)
    .await?;
    Ok(())
}

/// Add fractional reply credit for one 15-minute tick (`+B/96`).
pub async fn accrue_reply_credit(
    pool: &MySqlPool,
    ai_actor_id: u64,
    b: u64,
) -> Result<(), sqlx::Error> {
    let today = utc_today();
    ensure_budget_row(pool, ai_actor_id, today).await?;
    let increment = Decimal::from(b) / Decimal::from(REPLY_TICKS_PER_DAY);
    sqlx::query(
        r#"
        UPDATE council_ai_daily_budget
        SET reply_credit = reply_credit + ?
        WHERE ai_actor_id = ? AND `utc_date` = ?
        "#,
    )
    .bind(increment)
    .bind(ai_actor_id)
    .bind(today)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reply_credit_available(
    pool: &MySqlPool,
    ai_actor_id: u64,
) -> Result<Decimal, sqlx::Error> {
    let today = utc_today();
    ensure_budget_row(pool, ai_actor_id, today).await?;
    let credit: Decimal = sqlx::query_scalar(
        r#"SELECT reply_credit FROM council_ai_daily_budget WHERE ai_actor_id = ? AND `utc_date` = ?"#,
    )
    .bind(ai_actor_id)
    .bind(today)
    .fetch_one(pool)
    .await?;
    Ok(credit)
}

pub async fn spend_reply(
    pool: &MySqlPool,
    tx: &mut Transaction<'_, MySql>,
    ai_actor_id: u64,
    b: u64,
) -> Result<bool, sqlx::Error> {
    let today = utc_today();
    ensure_budget_row(pool, ai_actor_id, today).await?;
    let spent: u32 = sqlx::query_scalar(
        r#"SELECT replies_spent FROM council_ai_daily_budget WHERE ai_actor_id = ? AND `utc_date` = ? FOR UPDATE"#,
    )
    .bind(ai_actor_id)
    .bind(today)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(0);
    if spent >= b as u32 {
        return Ok(false);
    }
    let updated = sqlx::query(
        r#"
        UPDATE council_ai_daily_budget
        SET reply_credit = reply_credit - 1.0000,
            replies_spent = replies_spent + 1
        WHERE ai_actor_id = ? AND `utc_date` = ?
          AND reply_credit >= 1.0000
          AND replies_spent < ?
        "#,
    )
    .bind(ai_actor_id)
    .bind(today)
    .bind(b)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected() > 0)
}

pub async fn spend_post(
    pool: &MySqlPool,
    tx: &mut Transaction<'_, MySql>,
    ai_actor_id: u64,
    b: u64,
) -> Result<bool, sqlx::Error> {
    let today = utc_today();
    ensure_budget_row(pool, ai_actor_id, today).await?;
    let spent: u32 = sqlx::query_scalar(
        r#"SELECT posts_spent FROM council_ai_daily_budget WHERE ai_actor_id = ? AND `utc_date` = ? FOR UPDATE"#,
    )
    .bind(ai_actor_id)
    .bind(today)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(0);
    if spent >= b as u32 {
        return Ok(false);
    }
    let updated = sqlx::query(
        r#"
        UPDATE council_ai_daily_budget
        SET posts_spent = posts_spent + 1
        WHERE ai_actor_id = ? AND `utc_date` = ?
          AND posts_spent < ?
        "#,
    )
    .bind(ai_actor_id)
    .bind(today)
    .bind(b)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected() > 0)
}

pub async fn spend_invite(
    pool: &MySqlPool,
    tx: &mut Transaction<'_, MySql>,
    ai_actor_id: u64,
    b: u64,
) -> Result<bool, sqlx::Error> {
    let today = utc_today();
    ensure_budget_row(pool, ai_actor_id, today).await?;
    let spent: u32 = sqlx::query_scalar(
        r#"SELECT invites_spent FROM council_ai_daily_budget WHERE ai_actor_id = ? AND `utc_date` = ? FOR UPDATE"#,
    )
    .bind(ai_actor_id)
    .bind(today)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(0);
    if spent >= b as u32 {
        return Ok(false);
    }
    let updated = sqlx::query(
        r#"
        UPDATE council_ai_daily_budget
        SET invites_spent = invites_spent + 1
        WHERE ai_actor_id = ? AND `utc_date` = ?
          AND invites_spent < ?
        "#,
    )
    .bind(ai_actor_id)
    .bind(today)
    .bind(b)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::daily_budget_b;

    #[test]
    fn budget_scales_with_sqrt_sqrt_n() {
        assert_eq!(daily_budget_b(0), 1);
        assert_eq!(daily_budget_b(1), 1);
        // ceil(sqrt(sqrt(16))) = ceil(2) = 2
        assert_eq!(daily_budget_b(16), 2);
        // ceil(sqrt(sqrt(100))) = ceil(sqrt(10)) = 4
        assert_eq!(daily_budget_b(100), 4);
        assert_eq!(daily_budget_b(10_000), 10);
    }

    #[test]
    fn reply_budget_floors_at_nine() {
        use super::{daily_reply_budget_b, REPLY_BUDGET_FLOOR};

        assert_eq!(REPLY_BUDGET_FLOOR, 9);
        assert_eq!(daily_reply_budget_b(0), 9);
        assert_eq!(daily_reply_budget_b(100), 9);
        assert_eq!(daily_reply_budget_b(10_000), 10);
    }
}
