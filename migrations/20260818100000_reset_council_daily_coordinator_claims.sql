-- One-time ops reset (2026-08-18): burned daily council coordinator claims on beta.
--
-- Root cause: council_post_tick / council_invite_tick claimed ops_daily_digest_sent
-- before loading active members (or claimed on an empty fan-out). The UTC day was
-- marked done with enqueued=0 — no council_share_* chain until the next UTC day.
-- Code fix: claim only after confirming members.len() > 0 (deploy with this migration).
--
-- Safe to re-run: deletes zero rows once claims are cleared and fan-out succeeds.

-- Drop pending daily coordinators so the scheduler can enqueue a fresh run post-deploy.
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
WHERE jq.status IN ('pending', 'leased')
  AND rk.`key` IN ('council_post_tick', 'council_invite_tick');

-- Clear incomplete share/invite worker chains from the failed day (coordinators never fanned out).
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

-- Allow post-deploy coordinators to claim today and fan out to 12 councillors.
DELETE FROM ops_daily_digest_sent
WHERE digest_key IN ('council_post_tick', 'council_invite_tick');
