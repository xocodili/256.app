//! Generic job queue: ref-backed `ops_job_type`, canonical JSON + SHA-256 dedupe, conductor with 64s lease.

mod enqueue_policy;
mod journal_retention;
mod ops_period_claim;
mod payload;
pub mod queue;
pub mod scheduler;

pub use ops_period_claim::{claim_ops_period_once, ops_period_already_claimed};

pub use enqueue_policy::is_user_http_enqueue_allowed;
pub use queue::{
    cancel_job, council_pipeline_job_types_ready, enqueue_job, enqueue_job_at, extend_job_lease,
    job_queue_snapshot, resolve_job_type_id, run_conductor_tick,
};
pub use scheduler::{
    enqueue_recurring_system_jobs, scheduler_last_claimed, scheduler_tick_count,
    spawn as spawn_scheduler,
};
