-- Nom accrual economy: daily login + jury duty ledger reasons, dedupe columns.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('nom_ledger_reason', 'credit_daily_login', 15, 1, NULL, NULL),
    ('nom_ledger_reason', 'credit_jury_duty', 20, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

ALTER TABLE users
    ADD COLUMN nom_daily_credit_utc_date DATE NULL
        COMMENT 'UTC date of last daily login Nom accrual (+1; wallet cap 256)'
        AFTER last_login_at;

ALTER TABLE judicial_case_juror_invite
    ADD COLUMN nom_duty_credited_at DATETIME(6) NULL
        COMMENT 'When +3 jury-duty Noms were credited for this invite (once per case)'
        AFTER responded_at;
