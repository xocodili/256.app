-- Godmother private outreach ledger (welcome, global failing, global recovery) + recovery email dedupe.

ALTER TABLE social_entity
    ADD COLUMN global_recovery_notified_at TIMESTAMP(6) NULL
        COMMENT 'UTC when owner was first emailed for global rating recovery (>= failing threshold after failing notify)'
        AFTER rating_failing_notified_at;

CREATE TABLE godmother_outreach (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    human_entity_id BIGINT UNSIGNED NOT NULL,
    godmother_entity_id BIGINT UNSIGNED NOT NULL,
    moment_key VARCHAR(64) NOT NULL,
    post_id BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT (UTC_TIMESTAMP(6)),
    UNIQUE KEY uq_godmother_outreach (human_entity_id, moment_key),
    KEY idx_godmother_outreach_godmother (godmother_entity_id),
    CONSTRAINT fk_go_human FOREIGN KEY (human_entity_id) REFERENCES social_entity (id),
    CONSTRAINT fk_go_godmother FOREIGN KEY (godmother_entity_id) REFERENCES social_entity (id),
    CONSTRAINT fk_go_post FOREIGN KEY (post_id) REFERENCES post (id)
);

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'godmother_outreach', 51, 1, NULL, NULL);
