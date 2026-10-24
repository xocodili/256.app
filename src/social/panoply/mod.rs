//! Panoply — platform safety operator clearance (`member_of` @panoply).

mod actions;
mod audit;
mod conceal;
mod daily_status;
mod filing;
mod queues;

pub use daily_status::{
    collect_panoply_daily_metrics, run_panoply_daily_status_batch, EntityTypeDayCounts,
    PanoplyDailyMetrics,
};

pub use actions::{
    block_domain, eject_human_by_handle, kill_sessions_for_handle, vacate_emergency_case,
    ActionError,
};
pub use audit::{insert_action, recent_actions, ActionType, AuditRow};
pub use conceal::{
    conceal_panoply_from_viewer, filter_concealed_community_handles, is_invalid_community_parent_handle,
    is_panoply_community_handle, sql_exclude_panoply_entity,
};
pub use filing::{file_civic_dispute, file_emergency_safety, load_post_target, FilingError, OpsPostTarget};
pub use queues::{ContainedEmergencyRow, list_contained_emergencies};

use sqlx::MySqlPool;

use crate::html_escape::escape_html;
use crate::social::community_graph::is_panoply_operator_for_user;

pub async fn require_operator(pool: &MySqlPool, user_id: u64) -> Result<(), ()> {
    if is_panoply_operator_for_user(pool, user_id).await.unwrap_or(false) {
        Ok(())
    } else {
        Err(())
    }
}

/// Topbar link to `/home/ops` when the viewer has Panoply clearance.
pub async fn topbar_ops_button_html(
    pool: &MySqlPool,
    user_id: u64,
    label: &str,
) -> Result<String, sqlx::Error> {
    if !is_panoply_operator_for_user(pool, user_id).await? {
        return Ok(String::new());
    }
    Ok(format!(
        r#"<a class="app-topbar-run-jobs-btn app-topbar-ops-btn" id="app-topbar-ops-btn" href="/home/ops">{}</a>"#,
        escape_html(label)
    ))
}
