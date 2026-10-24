-- Flag social entities that are adult-only or sensitive in nature.
-- Default remains false for existing entities.

ALTER TABLE social_entity
    ADD COLUMN is_adult_or_sensitive TINYINT(1) NOT NULL DEFAULT 0
        COMMENT '1 when entity is adult-only and/or sensitive in nature' AFTER location_id,
    ADD KEY idx_social_entity_adult_sensitive (is_adult_or_sensitive);
