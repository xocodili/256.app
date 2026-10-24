-- Human → profit/non-profit follow, and human → gathering RSVP response types.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
SELECT 'social_entity_relationship_type', 'follow', 26, 1, NULL, NULL
FROM DUAL
WHERE NOT EXISTS (
    SELECT 1
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type'
      AND `key` = 'follow'
);

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
SELECT 'social_entity_relationship_type', 'event_going', 90, 1, NULL, NULL
FROM DUAL
WHERE NOT EXISTS (
    SELECT 1
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type'
      AND `key` = 'event_going'
);

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
SELECT 'social_entity_relationship_type', 'event_not_going', 91, 1, NULL, NULL
FROM DUAL
WHERE NOT EXISTS (
    SELECT 1
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type'
      AND `key` = 'event_not_going'
);

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
SELECT 'social_entity_relationship_type', 'event_interested', 92, 1, NULL, NULL
FROM DUAL
WHERE NOT EXISTS (
    SELECT 1
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type'
      AND `key` = 'event_interested'
);
