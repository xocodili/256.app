//! Who the mail is **from** in product terms — almost always a [`MailInitiator::SocialEntity`].
//!
//! HTTP handlers, graph jobs, and AI workers resolve an initiator once, then pass it to
//! category send functions (recruitment, engagement, judiciary).

use sqlx::MySqlPool;

use crate::error::{AppError, AppResult};
use crate::ref_keys;

/// Product actor attributed on outbound mail.
#[derive(Debug, Clone)]
pub enum MailInitiator {
    /// Human, profit/non-profit, gathering, community, or digital (AI) entity.
    SocialEntity { entity_id: u64 },
    /// Platform transactional mail with no social-graph sender (verify, reset).
    System,
}

/// Resolved display identity for templates (`@handle`, optional brand label).
#[derive(Debug, Clone)]
pub struct ResolvedInitiator {
    pub entity_id: u64,
    pub handle: String,
    /// Primary line for copy — `@handle` or `Label (@handle)` for brands.
    pub display_line: String,
    pub type_key: String,
}

impl ResolvedInitiator {
    #[must_use]
    pub fn at_handle(&self) -> String {
        format!("@{}", self.handle)
    }
}

pub async fn resolve_social_entity(
    pool: &MySqlPool,
    entity_id: u64,
) -> AppResult<ResolvedInitiator> {
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT se.handle, t.`key`, COALESCE(
            sc.label, sp.label, snp.label, sg.label, sd.label, se.handle
        ) AS display_label
        FROM social_entity se
        INNER JOIN ref_key_values t
            ON t.id = se.type_id
           AND t.`group` = ?
           AND t.is_active = 1
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN social_profit sp ON sp.social_entity_id = se.id
        LEFT JOIN social_non_profit snp ON snp.social_entity_id = se.id
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        LEFT JOIN social_digital sd ON sd.social_entity_id = se.id
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(entity_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;

    let Some((handle, type_key, label)) = row else {
        return Err(AppError::NotFound);
    };

    let display_line = match type_key.as_str() {
        ref_keys::KEY_ENTITY_TYPE_HUMAN | ref_keys::KEY_ENTITY_TYPE_DIGITAL => {
            format!("@{handle}")
        }
        _ => {
            let label = label.unwrap_or_else(|| handle.clone());
            if label.eq_ignore_ascii_case(&handle) {
                format!("@{handle}")
            } else {
                format!("{label} (@{handle})")
            }
        }
    };

    Ok(ResolvedInitiator {
        entity_id,
        handle,
        display_line,
        type_key,
    })
}
