//! Hide @panoply and operator membership from non-operators (search + profiles).

use sqlx::MySqlPool;

use crate::ref_keys;

use super::is_panoply_operator_for_user;

#[must_use]
pub fn is_panoply_community_handle(handle: &str) -> bool {
    handle
        .trim()
        .trim_start_matches('@')
        .eq_ignore_ascii_case(ref_keys::PANOPLY_COMMUNITY_HANDLE)
}

/// `@panoply` is ops clearance only — never a valid parent in the community tree.
#[must_use]
pub fn is_invalid_community_parent_handle(handle: &str) -> bool {
    is_panoply_community_handle(handle)
}

/// Non-operators must not discover @panoply or operator roster via ordinary UI.
pub async fn conceal_panoply_from_viewer(
    pool: &MySqlPool,
    viewer_user_id: u64,
) -> Result<bool, sqlx::Error> {
    Ok(!is_panoply_operator_for_user(pool, viewer_user_id).await?)
}

#[must_use]
pub fn sql_exclude_panoply_entity(se_alias: &str) -> String {
    format!(
        "AND {alias}.handle <> '{handle}'",
        alias = se_alias,
        handle = ref_keys::PANOPLY_COMMUNITY_HANDLE,
    )
}

#[must_use]
pub fn filter_concealed_community_handles(
    handles: Vec<String>,
    conceal: bool,
) -> Vec<String> {
    if !conceal {
        return handles;
    }
    handles
        .into_iter()
        .filter(|h| !is_panoply_community_handle(h))
        .collect()
}
