-- Add qualitative rating to social entities.
-- Scale: 0.00 to 5.00 (max two decimals), for internal scoring logic (not user-facing yet).

ALTER TABLE social_entity
    ADD COLUMN rating DECIMAL(3,2) NOT NULL DEFAULT 0.00
        COMMENT 'Internal qualitative rating (0.00..5.00, two decimals max)'
        AFTER location_id,
    ADD CONSTRAINT chk_social_entity_rating_range CHECK (rating >= 0.00 AND rating <= 5.00);

-- Baseline all existing entities to neutral floor.
UPDATE social_entity
SET rating = 0.00;

-- Human entities default to 2.50.
UPDATE social_entity se
INNER JOIN ref_key_values rk_type
    ON rk_type.id = se.type_id
   AND rk_type.`group` = 'social_entity_type'
   AND rk_type.`key` = 'human'
SET se.rating = 2.50;

-- Council AI members default to 5.00.
UPDATE social_entity se
INNER JOIN ai_actor aa ON aa.social_entity_id = se.id
INNER JOIN ai_council_membership acm
    ON acm.ai_actor_id = aa.id
   AND acm.is_active = 1
SET se.rating = 5.00;
