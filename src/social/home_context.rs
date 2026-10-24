//! Labels for signed-in shell chrome (e.g. top bar “256 – …”).

use sqlx::MySqlPool;

#[derive(Debug, sqlx::FromRow)]
struct PrimaryRow {
    handle: String,
    given_name: Option<String>,
    family_name: Option<String>,
}

fn format_context(handle: &str, given: Option<&str>, family: Option<&str>) -> String {
    let g = given.map(str::trim).filter(|s| !s.is_empty());
    let f = family.map(str::trim).filter(|s| !s.is_empty());
    match (g, f) {
        (Some(g), Some(f)) => format!("{g} {f}"),
        (Some(g), None) => g.to_string(),
        (None, Some(f)) => f.to_string(),
        (None, None) => handle.to_string(),
    }
}

/// Earliest `social_entity` created by this user; human names become “Given Family”, else `handle`.
pub async fn primary_entity_context_label(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query_as::<_, PrimaryRow>(
        r#"
        SELECT se.handle, sh.given_name, sh.family_name
        FROM social_entity se
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.created_by = ?
        ORDER BY se.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| format_context(&r.handle, r.given_name.as_deref(), r.family_name.as_deref())))
}
