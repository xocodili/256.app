-- Add `godmother` relationship type for onboarding mentor selection.
INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
SELECT 'social_entity_relationship_type', 'godmother', 80, 1, NULL, NULL
FROM DUAL
WHERE NOT EXISTS (
    SELECT 1
    FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type'
      AND `key` = 'godmother'
);
