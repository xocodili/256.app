-- Judicial trigger and proposal metadata for community profile amendments
-- (About, Our Intention, Join Policy — location is not amendable).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('judicial_case_trigger', 'community_profile_amendment', 35, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE IF NOT EXISTS judicial_case_community_profile_amendment (
    judicial_case_id BIGINT UNSIGNED NOT NULL COMMENT 'FK judicial_case.id',
    change_description TINYINT NOT NULL DEFAULT 0,
    change_intention TINYINT NOT NULL DEFAULT 0,
    change_join_policy TINYINT NOT NULL DEFAULT 0,
    current_description_plain TEXT NOT NULL,
    current_intention_plain TEXT NOT NULL,
    current_join_policy_id BIGINT UNSIGNED NULL,
    proposed_description_plain TEXT NOT NULL,
    proposed_description_markup TEXT NOT NULL,
    proposed_description_html_sanitized TEXT NOT NULL,
    proposed_intention_plain TEXT NOT NULL,
    proposed_intention_markup TEXT NOT NULL,
    proposed_intention_html_sanitized TEXT NOT NULL,
    proposed_join_policy_id BIGINT UNSIGNED NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (judicial_case_id),
    CONSTRAINT chk_jccpa_at_least_one_change
        CHECK (change_description = 1 OR change_intention = 1 OR change_join_policy = 1),
    CONSTRAINT fk_jccpa_case
        FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jccpa_current_join_policy
        FOREIGN KEY (current_join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_jccpa_proposed_join_policy
        FOREIGN KEY (proposed_join_policy_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
