-- One-time ops purge (2026-08-15): drain poisoned council AI job backlog on beta.
-- Caused by dedupe miss + short JOB_QUEUE_LEASE_SECS → thousands of pending
-- council_share_shortlist rows and stale daily coordinators blocking the conductor.
-- Safe to re-run (deletes zero rows once backlog is gone). Scheduler re-enqueues
-- fresh daily coordinators on the next tick after deploy.

-- Council share + invite pipeline workers (pending or stuck leased).
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
WHERE jq.status IN ('pending', 'leased')
  AND rk.`key` IN (
    'council_post_member',
    'council_share_shortlist',
    'council_share_read',
    'council_share_publish',
    'council_invite_article_pick',
    'council_invite_person_discover',
    'council_invite_contact_resolve',
    'council_invite_compose_send'
  );

-- Stale daily coordinators (utc_date before today UTC).
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
WHERE jq.status = 'pending'
  AND rk.`key` IN (
    'council_post_tick',
    'council_invite_tick',
    'panoply_daily_status',
    'user_engagement_digest'
  )
  AND JSON_UNQUOTE(JSON_EXTRACT(jq.payload, '$.utc_date')) < DATE_FORMAT(UTC_TIMESTAMP(), '%Y-%m-%d');

-- Duplicate coordinators for today (keep one pending row per job type; drop extras).
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.is_active = 1
INNER JOIN (
    SELECT jq2.id
    FROM job_queue jq2
    INNER JOIN ref_key_values rk2
        ON rk2.id = jq2.job_type_id
       AND rk2.`group` = 'ops_job_type'
       AND rk2.is_active = 1
    INNER JOIN (
        SELECT rk3.`key` AS type_key, MIN(jq3.id) AS keep_id
        FROM job_queue jq3
        INNER JOIN ref_key_values rk3
            ON rk3.id = jq3.job_type_id
           AND rk3.`group` = 'ops_job_type'
           AND rk3.is_active = 1
        WHERE jq3.status = 'pending'
          AND rk3.`key` IN (
              'council_post_tick',
              'council_invite_tick',
              'panoply_daily_status',
              'user_engagement_digest'
          )
          AND JSON_UNQUOTE(JSON_EXTRACT(jq3.payload, '$.utc_date')) = DATE_FORMAT(UTC_TIMESTAMP(), '%Y-%m-%d')
        GROUP BY rk3.`key`
    ) keepers ON rk2.`key` = keepers.type_key
    WHERE jq2.status = 'pending'
      AND jq2.id <> keepers.keep_id
      AND rk2.`key` IN (
          'council_post_tick',
          'council_invite_tick',
          'panoply_daily_status',
          'user_engagement_digest'
      )
      AND JSON_UNQUOTE(JSON_EXTRACT(jq2.payload, '$.utc_date')) = DATE_FORMAT(UTC_TIMESTAMP(), '%Y-%m-%d')
) dupes ON dupes.id = jq.id;

-- Expired invite pipeline scratch (optional hygiene; TTL rows only).
DELETE FROM council_invite_pipeline_state
WHERE expires_at < UTC_TIMESTAMP(6);
