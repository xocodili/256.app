-- Per-councillor council post worker (fan-out from daily council_post_tick coordinator).

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'council_post_member', 54, 1, NULL, NULL);
