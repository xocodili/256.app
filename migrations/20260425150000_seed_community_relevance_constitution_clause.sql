-- Constitutional additive clause for each of the 12 foundational communities:
-- Posts must stay relevant to the community's core intention.

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

INSERT INTO community_constitution_item (community_id, point, item, status, created_by, modified_by)
SELECT
    se.id AS community_id,
    100 AS point,
    'Posts should be relevant to the core intention of this group.' AS item,
    'active' AS status,
    @system_uid AS created_by,
    @system_uid AS modified_by
FROM social_entity se
INNER JOIN social_community sc ON sc.social_entity_id = se.id
WHERE se.handle IN (
    'creativity_leisure_adventure',
    'fitness_health_wellbeing',
    'intellectual_life_education',
    'emotional_life_wellbeing',
    'character_self_mastery',
    'spiritual_life_meaning',
    'love_romantic_relationships',
    'parenting_family',
    'social_life_community_justice',
    'career_mission',
    'financial_life_abundance',
    'quality_of_life_environment'
)
AND NOT EXISTS (
    SELECT 1
    FROM community_constitution_item cci
    WHERE cci.community_id = se.id
      AND cci.point = 100
);
