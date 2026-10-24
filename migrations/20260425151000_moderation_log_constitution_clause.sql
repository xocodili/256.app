-- Store which constitutional clause was selected by the user for moderation requests.

ALTER TABLE ai_moderation_decision_log
    ADD COLUMN violated_constitution_item_id BIGINT UNSIGNED NULL
        COMMENT 'Selected constitutional clause (community_constitution_item.id) when user files moderation request'
        AFTER rationale,
    ADD KEY idx_ai_mod_log_violated_constitution_item (violated_constitution_item_id),
    ADD CONSTRAINT fk_ai_mod_log_violated_constitution_item
        FOREIGN KEY (violated_constitution_item_id)
        REFERENCES community_constitution_item (id)
        ON DELETE SET NULL;
