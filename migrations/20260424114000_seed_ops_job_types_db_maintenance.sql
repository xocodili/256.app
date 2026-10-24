-- Queue-backed DB maintenance jobs.
INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('ops_job_type', 'scope_recalc_drain', 20, 1, NULL, NULL),
('ops_job_type', 'scope_rematerialize_all', 30, 1, NULL, NULL);
