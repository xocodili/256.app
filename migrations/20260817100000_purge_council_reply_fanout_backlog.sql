-- One-time ops purge (2026-08-17): duplicate council reply fan-out backlog on beta.
--
-- Root cause: queue dedup is live-row only — after council_reply_tick archives, the
-- scheduler re-enqueued it every tick and fanned out 12 council_reply_member jobs
-- per second for the same 15-minute period. Deploy ships ops_daily_digest_sent
-- period claims in handlers; this migration clears poisoned queue rows and claim
-- rows so the next scheduler cycle starts clean.
--
-- Safe to re-run (deletes zero rows once backlog is gone).

-- Duplicate reply workers + coordinators (pending or stuck leased).
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
WHERE jq.status IN ('pending', 'leased')
  AND rk.`key` IN ('council_reply_member', 'council_reply_tick');

-- Same duplicate-fan-out pattern for daily share/invite coordinators (pending/leased workers).
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
WHERE jq.status IN ('pending', 'leased')
  AND rk.`key` IN (
    'council_share_shortlist',
    'council_share_read',
    'council_share_publish',
    'council_invite_article_pick',
    'council_invite_person_discover',
    'council_invite_contact_resolve',
    'council_invite_compose_send'
  );

-- Reset period claims so post-deploy handlers can claim once per period/day.
DELETE FROM ops_daily_digest_sent
WHERE digest_key IN (
    'council_reply_tick',
    'council_reply_member',
    'council_post_tick',
    'council_invite_tick'
);
