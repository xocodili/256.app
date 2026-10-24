//! Nom pack purchase fulfillment (Stripe webhook path).

mod common;

use app::nom_packs::NomPack;
use app::social::noms::purchase::{self, FulfillOutcome, PurchaseRejectReason, PurchaseStatus};
use sqlx::MySqlPool;

async fn seed_human_entity(pool: &MySqlPool, user_id: u64, handle: &str) -> u64 {
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
        INSERT INTO social_human (social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id)
        VALUES (?, 'Nom', 'Buyer', '1990-01-01', ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(loc)
    .bind(join_open)
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
async fn fulfill_credits_wallet_and_is_idempotent() {
    let db = common::TestDb::new().await;
    let user_id = common::api::seed_verified_user(&db.pool, "buyer@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_buyer").await;

    let pack = NomPack::pack_128_aud();
    let pi_id = "pi_test_fulfill_001";

    let purchase_id = purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        pack,
        pi_id,
    )
    .await
    .expect("insert pending");

    let balance1 = match purchase::fulfill_purchase_by_payment_intent(&db.pool, pi_id)
        .await
        .expect("fulfill")
    {
        FulfillOutcome::Credited(b) => b,
        other => panic!("expected credited, got {other:?}"),
    };
    assert_eq!(balance1, 128);

    let balance2 = match purchase::fulfill_purchase_by_payment_intent(&db.pool, pi_id)
        .await
        .expect("fulfill again")
    {
        FulfillOutcome::AlreadyCompleted(b) => b,
        other => panic!("expected already completed, got {other:?}"),
    };
    assert_eq!(balance2, 128);

    let row = purchase::load_purchase_by_id(&db.pool, purchase_id, user_id)
        .await
        .expect("load")
        .expect("row");
    assert_eq!(row.balance_after, Some(128));

    db.cleanup().await;
}

#[tokio::test]
async fn insert_blocked_when_balance_at_or_above_128() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "full@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_full").await;

    sqlx::query("UPDATE social_human SET ai_tokens = 128 WHERE social_entity_id = ?")
        .bind(entity_id)
        .execute(&db.pool)
        .await
        .expect("set balance");

    let err = purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        NomPack::pack_128_aud(),
        "pi_test_blocked",
    )
    .await
    .expect_err("should block");

    assert!(err.to_string().contains("below 128") || err.to_string().contains("128"));

    db.cleanup().await;
}

#[tokio::test]
async fn second_pending_purchase_is_blocked() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "pending@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_pending").await;
    let pack = NomPack::pack_128_aud();

    purchase::insert_pending_purchase(&db.pool, user_id, entity_id, pack, "pi_pending_001")
        .await
        .expect("first pending");

    let err = purchase::insert_pending_purchase(&db.pool, user_id, entity_id, pack, "pi_pending_002")
        .await
        .expect_err("second pending should block");

    assert!(err.to_string().contains("checkout is already in progress"));

    db.cleanup().await;
}

#[tokio::test]
async fn fulfill_rejects_stale_wallet_and_marks_failed() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "stale@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_stale").await;
    let pack = NomPack::pack_128_aud();
    let pi_id = "pi_test_stale_wallet";

    let purchase_id = purchase::insert_pending_purchase(&db.pool, user_id, entity_id, pack, pi_id)
        .await
        .expect("insert pending");

    sqlx::query("UPDATE social_human SET ai_tokens = 128 WHERE social_entity_id = ?")
        .bind(entity_id)
        .execute(&db.pool)
        .await
        .expect("raise balance before fulfill");

    let outcome = purchase::fulfill_purchase_by_payment_intent(&db.pool, pi_id)
        .await
        .expect("fulfill");
    assert_eq!(
        outcome,
        FulfillOutcome::RejectedAfterPayment(PurchaseRejectReason::WalletFull {
            balance: 128,
            max_exclusive: 128,
        })
    );

    let row = purchase::load_purchase_by_id(&db.pool, purchase_id, user_id)
        .await
        .expect("load")
        .expect("row");
    assert_eq!(row.status, PurchaseStatus::Failed);

    db.cleanup().await;
}

#[tokio::test]
async fn pack_civic_fulfills_from_zero_balance() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "civic@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_civic").await;

    let pack = NomPack::pack_civic_aud();
    let pi_id = "pi_test_civic_001";

    purchase::insert_pending_purchase(&db.pool, user_id, entity_id, pack, pi_id)
        .await
        .expect("insert pending");

    let balance = match purchase::fulfill_purchase_by_payment_intent(&db.pool, pi_id)
        .await
        .expect("fulfill")
    {
        FulfillOutcome::Credited(b) => b,
        other => panic!("expected credited, got {other:?}"),
    };
    assert_eq!(balance, 32);

    db.cleanup().await;
}

#[tokio::test]
async fn pack_civic_available_at_balance_32() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "civic_full@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_civic_full").await;

    sqlx::query("UPDATE social_human SET ai_tokens = 32 WHERE social_entity_id = ?")
        .bind(entity_id)
        .execute(&db.pool)
        .await
        .expect("set balance");

    purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        NomPack::pack_civic_aud(),
        "pi_civic_at_32",
    )
    .await
    .expect("civic available at balance 32");

    db.cleanup().await;
}

#[tokio::test]
async fn cancel_pending_purchase_frees_checkout() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "cancel@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_cancel").await;
    let pack = NomPack::pack_32_aud();

    let purchase_id = purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        pack,
        "pi_cancel_test",
    )
    .await
    .expect("insert pending");

    let pi = purchase::cancel_pending_purchase_by_id(&db.pool, purchase_id, user_id)
        .await
        .expect("cancel")
        .expect("pi id");

    assert_eq!(pi, "pi_cancel_test");

    let row = purchase::load_purchase_by_id(&db.pool, purchase_id, user_id)
        .await
        .expect("load")
        .expect("row");
    assert_eq!(row.status, PurchaseStatus::Failed);

    purchase::insert_pending_purchase(&db.pool, user_id, entity_id, pack, "pi_cancel_test_2")
        .await
        .expect("second pending after cancel");

    db.cleanup().await;
}

#[tokio::test]
async fn pack_128_blocked_when_balance_at_or_above_128() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "steward@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_steward").await;

    sqlx::query("UPDATE social_human SET ai_tokens = 128 WHERE social_entity_id = ?")
        .bind(entity_id)
        .execute(&db.pool)
        .await
        .expect("set balance");

    let err = purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        NomPack::pack_128_aud(),
        "pi_steward_blocked",
    )
    .await
    .expect_err("steward pack blocked when balance >= 128");

    assert!(err.to_string().contains("below 128") || err.to_string().contains("128"));

    sqlx::query("UPDATE social_human SET ai_tokens = 127 WHERE social_entity_id = ?")
        .bind(entity_id)
        .execute(&db.pool)
        .await
        .expect("127 balance");

    let pi_id = "pi_steward_ok";
    purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        NomPack::pack_128_aud(),
        pi_id,
    )
    .await
    .expect("steward at 127");

    let balance = match purchase::fulfill_purchase_by_payment_intent(&db.pool, pi_id)
        .await
        .expect("fulfill")
    {
        FulfillOutcome::Credited(b) => b,
        other => panic!("expected credited, got {other:?}"),
    };
    assert_eq!(balance, 255);

    db.cleanup().await;
}

#[tokio::test]
async fn pack_64_available_at_balance_64() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "standard@example.com", "password12345").await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_standard").await;

    sqlx::query("UPDATE social_human SET ai_tokens = 64 WHERE social_entity_id = ?")
        .bind(entity_id)
        .execute(&db.pool)
        .await
        .expect("set balance");

    purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        NomPack::pack_64_aud(),
        "pi_standard_at_64",
    )
    .await
    .expect("standard pack available at balance 64");

    db.cleanup().await;
}

#[tokio::test]
async fn pack_64_blocked_when_wallet_cap_exceeded() {
    let db = common::TestDb::new().await;
    let user_id =
        common::api::seed_verified_user(&db.pool, "standard_cap@example.com", "password12345")
            .await;
    let entity_id = seed_human_entity(&db.pool, user_id, "nom_standard_cap").await;

    sqlx::query("UPDATE social_human SET ai_tokens = 193 WHERE social_entity_id = ?")
        .bind(entity_id)
        .execute(&db.pool)
        .await
        .expect("set balance");

    let err = purchase::insert_pending_purchase(
        &db.pool,
        user_id,
        entity_id,
        NomPack::pack_64_aud(),
        "pi_standard_blocked_cap",
    )
    .await
    .expect_err("standard pack blocked when balance + 64 > 256");

    assert!(err.to_string().contains("256") || err.to_string().contains("cap"));

    db.cleanup().await;
}
