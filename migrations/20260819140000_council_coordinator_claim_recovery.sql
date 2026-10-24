-- Recovery: phantom ops_daily_digest_sent rows blocked coordinator enqueue all day.
-- Safe to re-run. Clears daily fan-out claims only (handlers re-claim idempotently).

DELETE FROM ops_daily_digest_sent
WHERE digest_key IN (
    'council_post_tick',
    'council_invite_tick',
    'panoply_daily_status'
);
