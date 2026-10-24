-- Constitution item configurability:
-- - moderation_chk: whether an item can be selected as a complaint trigger.
-- - multi-select join table for moderation requests -> violated clauses.

ALTER TABLE community_constitution_item
    ADD COLUMN moderation_chk TINYINT(1) NOT NULL DEFAULT 0
        COMMENT '1 when this constitutional item is eligible as a moderation complaint trigger'
        AFTER status,
    ADD KEY idx_community_constitution_moderation_chk (community_id, moderation_chk),
    ADD CONSTRAINT chk_community_constitution_moderation_chk
        CHECK (moderation_chk IN (0, 1));

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

-- Root principles that should currently trigger moderation.
UPDATE community_constitution_item
SET moderation_chk = 1
WHERE community_id = @root_community_id
  AND point IN (4, 11);

CREATE TABLE IF NOT EXISTS ai_moderation_decision_clause (
    moderation_log_id BIGINT UNSIGNED NOT NULL,
    constitution_item_id BIGINT UNSIGNED NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (moderation_log_id, constitution_item_id),
    KEY idx_ai_mod_decision_clause_item (constitution_item_id),
    CONSTRAINT fk_ai_mod_decision_clause_log
        FOREIGN KEY (moderation_log_id)
        REFERENCES ai_moderation_decision_log (id)
        ON DELETE CASCADE,
    CONSTRAINT fk_ai_mod_decision_clause_constitution
        FOREIGN KEY (constitution_item_id)
        REFERENCES community_constitution_item (id)
        ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
