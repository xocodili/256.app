-- New `member_of` edges use neutral standing (2.50): eligible for community jury pools at
-- weight 1.0 without requiring earned reputation first.

ALTER TABLE social_entity_relationship
    MODIFY COLUMN rating DECIMAL(3,2) NOT NULL DEFAULT 2.50
        COMMENT 'Internal connection quality rating (0.00..5.00, two decimals max)';

UPDATE social_entity_relationship ser
INNER JOIN ref_key_values rk_type
    ON rk_type.id = ser.type_id
   AND rk_type.`group` = 'social_entity_relationship_type'
   AND rk_type.`key` = 'member_of'
SET ser.rating = 2.50
WHERE ser.ended_at IS NULL
  AND ser.rating = 0.00;
