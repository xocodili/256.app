-- Post-moderation cases tied to a feed community: time-based `social_entity.rating` feedback on
-- resolution, plus `no_jury` terminal status when a jury cannot be seated within 21 days.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('judicial_case_status', 'no_jury', 38, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

ALTER TABLE judicial_case
    ADD COLUMN community_rating_feedback_applied_at DATETIME(6) NULL
        COMMENT 'When feed-community timing reward/penalty was applied (verdict or no-jury timeout)'
        AFTER jury_civic_rating_applied_at;
