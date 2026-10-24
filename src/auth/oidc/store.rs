//! OIDC identity persistence (`user_oidc_identity`).

use sqlx::MySqlPool;

pub const PROVIDER_GOOGLE: &str = "google";
pub const GOOGLE_ISSUER: &str = "https://accounts.google.com";

pub async fn find_user_by_provider_subject(
    pool: &MySqlPool,
    provider: &str,
    issuer: &str,
    subject: &str,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT user_id
        FROM user_oidc_identity
        WHERE provider = ? AND issuer = ? AND subject = ?
        LIMIT 1
        "#,
    )
    .bind(provider)
    .bind(issuer)
    .bind(subject)
    .fetch_optional(pool)
    .await
}

pub async fn user_has_provider(
    pool: &MySqlPool,
    user_id: u64,
    provider: &str,
) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM user_oidc_identity
        WHERE user_id = ? AND provider = ?
        "#,
    )
    .bind(user_id)
    .bind(provider)
    .fetch_one(pool)
    .await?;
    Ok(n > 0)
}

pub async fn insert_identity_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_id: u64,
    provider: &str,
    issuer: &str,
    subject: &str,
    email_at_link: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO user_oidc_identity (
            user_id, provider, issuer, subject, email_at_link, created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(provider)
    .bind(issuer)
    .bind(subject)
    .bind(email_at_link)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn insert_identity(
    pool: &MySqlPool,
    user_id: u64,
    provider: &str,
    issuer: &str,
    subject: &str,
    email_at_link: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    insert_identity_tx(
        &mut tx,
        user_id,
        provider,
        issuer,
        subject,
        email_at_link,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
