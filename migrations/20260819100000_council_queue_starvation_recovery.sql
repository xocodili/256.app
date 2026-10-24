-- Recovery from feed-poll re-enqueue starvation (2026-08-19).
--
-- Feed poll archived every ~2 min and was re-enqueued every scheduler tick at
-- priority 81, filling the batch-8 conductor plan before share (83) / panoply (90).
-- Code fix: period claim + skip enqueue when period satisfied; share priority 76.
--
-- Safe to re-run: idempotent deletes.

DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
WHERE jq.status IN ('pending', 'leased')
  AND rk.`key` = 'council_portfolio_feed_poll';

DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
WHERE jq.status IN ('pending', 'leased')
  AND rk.`key` IN (
    'council_share_shortlist',
    'council_share_read',
    'council_share_publish'
  );

DELETE FROM ops_daily_digest_sent
WHERE digest_key IN (
    'council_post_tick',
    'council_invite_tick',
    'council_portfolio_feed_poll',
    'panoply_daily_status'
);

UPDATE job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
   AND rk.`key` IN (
       'council_share_shortlist',
       'council_share_read',
       'council_share_publish'
   )
SET jq.priority = 76
WHERE jq.status = 'pending';
