//! Nom accrual: daily login (+1) and jury duty (+3).

mod common;

use app::auth::session;
use app::social::noms::{self, AccrualOutcome};
use chrono::{Duration, Utc};
use sqlx::MySqlPool;

async fn seed_human_wallet(pool: &MySqlPool, user_id: u64, handle: &str, balance: u16) -> u64 {
    let loc = common::api::seed_location(pool).await;
    let type_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'human' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("human type");
    let status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("verified status");
    let entity_status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_status' AND `key` = 'verified' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("entity verified");
    let join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_join_policy' AND `key` = 'open' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("join open");

    let entity_id = sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, rating)
        VALUES (?, ?, ?, ?, ?, ?, 2.50)
        "#,
    )
    .bind(handle)
    .bind(type_id)
    .bind(status_id)
    .bind(entity_status_id)
    .bind(loc)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert human")
    .last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO social_human (social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id, ai_tokens)
        VALUES (?, 'Accrual', 'Tester', '1990-01-01', ?, ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(loc)
    .bind(join_open)
    .bind(balance)
    .execute(pool)
    .await
    .expect("insert social_human");

    let owner_role: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'user_social_entity_role' AND `key` = 'owner' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("owner role");
    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at)
        VALUES (?, ?, ?, 1, UTC_TIMESTAMP())
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .bind(owner_role)
    .execute(pool)
    .await
    .expect("owner role row");

    entity_id
}

#[tokio::test]
async fn daily_login_credits_once_per_utc_day() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "daily@example.com", "password12345").await;
    let entity_id = seed_human_wallet(&db.pool, user_id, "daily_login", 32).await;

    session::record_user_login(&db.pool, user_id)
        .await
        .expect("first login");

    let balance: u16 = sqlx::query_scalar(
        "SELECT ai_tokens FROM social_human WHERE social_entity_id = ?",
    )
    .bind(entity_id)
    .fetch_one(&db.pool)
    .await
    .expect("balance");
    assert_eq!(balance, 33);

    session::record_user_login(&db.pool, user_id)
        .await
        .expect("second login same day");

    let balance2: u16 = sqlx::query_scalar(
        "SELECT ai_tokens FROM social_human WHERE social_entity_id = ?",
    )
    .bind(entity_id)
    .fetch_one(&db.pool)
    .await
    .expect("balance again");
    assert_eq!(balance2, 33, "second login same UTC day should not double-credit");

    let yesterday = Utc::now().date_naive() - Duration::days(1);
    sqlx::query("UPDATE users SET nom_daily_credit_utc_date = ? WHERE id = ?")
        .bind(yesterday)
        .bind(user_id)
        .execute(&db.pool)
        .await
        .expect("backdate credit date");

    session::record_user_login(&db.pool, user_id)
        .await
        .expect("login after backdate");

    let balance3: u16 = sqlx::query_scalar(
        "SELECT ai_tokens FROM social_human WHERE social_entity_id = ?",
    )
    .bind(entity_id)
    .fetch_one(&db.pool)
    .await
    .expect("balance after new day");
    assert_eq!(balance3, 34);

    db.cleanup().await;
}

#[tokio::test]
async fn jury_duty_credit_is_idempotent_per_invite() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "juror@example.com", "password12345").await;
    let entity_id = seed_human_wallet(&db.pool, user_id, "jury_nom", 10).await;

    let target_entity_id = seed_human_wallet(&db.pool, user_id, "jury_target", 0).await;

    let trigger_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'judicial_case_trigger' AND `key` = 'civic_dispute' LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .expect("trigger");
    let status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'judicial_case_status' AND `key` = 'resolved' LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .expect("resolved");

    let case_id: u64 = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id, target_social_entity_id,
            trigger_type_id, status_id, created_by
        )
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(target_entity_id)
    .bind(entity_id)
    .bind(trigger_id)
    .bind(status_id)
    .bind(user_id)
    .execute(&db.pool)
    .await
    .expect("case")
    .last_insert_id();

    let invite_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'judicial_case_juror_invite_status' AND `key` = 'accepted' LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .expect("accepted");
    let vote_for: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'judicial_case_juror_invite_vote_direction' AND `key` = 'for' LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .expect("for");
    let rating_basis: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'judicial_case_juror_invite_rating_basis' AND `key` = 'global' LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .expect("global basis");

    let invite_id = sqlx::query(
        r#"
        INSERT INTO judicial_case_juror_invite (
            judicial_case_id, juror_social_entity_id, juror_user_id,
            juror_rating_at_invite, invite_status_id, vote_direction_id, rating_basis_id,
            selected_at, responded_at, created_by
        )
        VALUES (?, ?, ?, 2.50, ?, ?, ?, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6), ?)
        "#,
    )
    .bind(case_id)
    .bind(entity_id)
    .bind(user_id)
    .bind(invite_status)
    .bind(vote_for)
    .bind(rating_basis)
    .bind(user_id)
    .execute(&db.pool)
    .await
    .expect("invite")
    .last_insert_id();

    let mut tx = db.pool.begin().await.expect("tx");
    let first = noms::try_credit_jury_duty_tx(&mut tx, invite_id, entity_id, case_id, Some(user_id))
        .await
        .expect("first credit");
    tx.commit().await.expect("commit");
    assert_eq!(
        first,
        AccrualOutcome::Credited { balance: 13 },
        "10 + 3 jury duty"
    );

    let mut tx = db.pool.begin().await.expect("tx");
    let second = noms::try_credit_jury_duty_tx(&mut tx, invite_id, entity_id, case_id, Some(user_id))
        .await
        .expect("second credit");
    tx.commit().await.expect("commit");
    assert_eq!(second, AccrualOutcome::AlreadyClaimed);

    db.cleanup().await;
}
