//! Tamper-evident audit log for automated sanitation (Root point 15).

use sqlx::MySqlPool;

#[derive(Debug, Clone)]
pub struct SanitationEvent<'a> {
    pub action_class: &'a str,
    pub harm_class: Option<&'a str>,
    pub source: &'a str,
    pub actor_user_id: Option<u64>,
    pub target_user_id: Option<u64>,
    pub target_post_id: Option<u64>,
    pub content_sha256: Option<&'a str>,
    pub registrable_domain: Option<&'a str>,
    pub detail_json: Option<&'a str>,
}

pub async fn log_sanitation_event(pool: &MySqlPool, event: SanitationEvent<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO safety_sanitation_event (
            action_class,
            harm_class,
            source,
            actor_user_id,
            target_user_id,
            target_post_id,
            content_sha256,
            registrable_domain,
            detail_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(event.action_class)
    .bind(event.harm_class)
    .bind(event.source)
    .bind(event.actor_user_id)
    .bind(event.target_user_id)
    .bind(event.target_post_id)
    .bind(event.content_sha256)
    .bind(event.registrable_domain)
    .bind(event.detail_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn log_domain_deny(
    pool: &MySqlPool,
    source: &str,
    registrable_domain: &str,
    actor_user_id: Option<u64>,
    target_post_id: Option<u64>,
) {
    if let Err(err) = log_sanitation_event(
        pool,
        SanitationEvent {
            action_class: "domain_deny",
            harm_class: None,
            source,
            actor_user_id,
            target_user_id: None,
            target_post_id,
            content_sha256: None,
            registrable_domain: Some(registrable_domain),
            detail_json: None,
        },
    )
    .await
    {
        tracing::warn!(
            error = %err,
            domain = registrable_domain,
            source,
            "failed to log safety_sanitation_event for domain deny"
        );
    }
}

pub async fn log_hash_gate_block(
    pool: &MySqlPool,
    source: &str,
    harm_class: &str,
    sha256_hex: &str,
    actor_user_id: Option<u64>,
) {
    if let Err(err) = log_sanitation_event(
        pool,
        SanitationEvent {
            action_class: "hash_gate_block",
            harm_class: Some(harm_class),
            source,
            actor_user_id,
            target_user_id: None,
            target_post_id: None,
            content_sha256: Some(sha256_hex),
            registrable_domain: None,
            detail_json: None,
        },
    )
    .await
    {
        tracing::warn!(
            error = %err,
            source,
            "failed to log safety_sanitation_event for hash gate block"
        );
    }
}

pub async fn log_filing_sanitation_routed(
    pool: &MySqlPool,
    source: &str,
    harm_class: &str,
    actor_user_id: u64,
    target_post_id: u64,
    clause_points: &[u16],
) {
    let detail = serde_json::json!({ "clause_points": clause_points }).to_string();
    if let Err(err) = log_sanitation_event(
        pool,
        SanitationEvent {
            action_class: "filing_routed_sanitation",
            harm_class: Some(harm_class),
            source,
            actor_user_id: Some(actor_user_id),
            target_user_id: None,
            target_post_id: Some(target_post_id),
            content_sha256: None,
            registrable_domain: None,
            detail_json: Some(&detail),
        },
    )
    .await
    {
        tracing::warn!(
            error = %err,
            source,
            post_id = target_post_id,
            "failed to log safety_sanitation_event for filing sanitation route"
        );
    }
}
