//! Run against your dev DB: `LIVE_DB=1 cargo test --test live_db_home_smoke -- --nocapture`
//! Uses `DATABASE_URL` from `.env` (not TestDb).

use app::social::{connections, feed, human_onboarding, judicial, user_faces};

#[tokio::test]
async fn home_page_query_stack_smoke() {
    if std::env::var("LIVE_DB").ok().as_deref() != Some("1") {
        eprintln!("skip live_db_home_smoke (set LIVE_DB=1)");
        return;
    }
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = app::db::connect_and_migrate(&url).await.expect("connect");

    let user_id: u64 = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email_verified_at IS NOT NULL ORDER BY id LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .expect("verified user");

    eprintln!("smoke user_id={user_id}");

    human_onboarding::user_has_human_profile(&pool, user_id)
        .await
        .expect("user_has_human_profile");
    let _: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM users u
        INNER JOIN ref_key_values rk ON rk.id = COALESCE(u.user_status_id, u.status_id)
        WHERE u.id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .expect("user status");
    connections::list_for_user(&pool, user_id)
        .await
        .expect("list_for_user");
    user_faces::artwork_path_for_user(&pool, user_id)
        .await
        .expect("artwork_path");
    judicial::find_pending_human_jury_invite_for_user(&pool, user_id)
        .await
        .expect("juror invite");
    feed::fetch_page_rows(&pool, user_id, 0, 13, None, false)
        .await
        .expect("fetch_page_rows");

    eprintln!("home query stack ok");
}
