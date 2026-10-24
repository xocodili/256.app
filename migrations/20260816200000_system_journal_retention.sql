-- Daily systemd journal retention worker (3-day default via JOURNAL_RETENTION_DAYS).

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'system_journal_retention', 56, 1, NULL, NULL);
