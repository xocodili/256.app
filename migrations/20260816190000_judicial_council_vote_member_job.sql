-- Per-councillor AI council appeal vote worker (fan-out from judicial_council_appeal coordinator).

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'judicial_council_vote_member', 55, 1, NULL, NULL);

-- In-flight monolithic appeal coordinators can block the queue for many minutes; drop and re-enqueue via fan-out.
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk
    ON rk.id = jq.job_type_id
   AND rk.`group` = 'ops_job_type'
   AND rk.`key` = 'judicial_council_appeal'
WHERE jq.status IN ('pending', 'leased');
