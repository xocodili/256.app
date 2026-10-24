-- Per-councillor council reply worker (fan-out from council_reply_tick coordinator).

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'council_reply_member', 54, 1, NULL, NULL);

-- In-flight monolithic reply coordinators/workers block the queue for ~30 min; drop and re-enqueue via fan-out.
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.`key` = 'council_reply_tick'
WHERE jq.status IN ('pending', 'leased');
