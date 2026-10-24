-- Daily user engagement digest job (every 3 idle days, days 3–18; stops at 21d inactivity mail).

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)

VALUES ('ops_job_type', 'user_engagement_digest', 57, 1, NULL, NULL);



ALTER TABLE users

    ADD COLUMN engagement_digest_sent_at TIMESTAMP(6) NULL

        COMMENT 'Last away-digest email sent; reset eligibility on login via last_login_at comparison'

        AFTER last_login_at;

