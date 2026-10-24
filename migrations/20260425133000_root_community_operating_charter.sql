-- Single constitutional root for all council communities.
-- New root community: handle `operating_charter_256`, label `256 Operating Charter`.
-- Re-parent the 12 foundational council communities under this root.

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @global_lid := (
    SELECT id FROM locations
    WHERE canonical_fingerprint = UNHEX('d668ba83cc35a3d46e3cf2544d70b4d5194ae479cd434d5556fa26635b0b50cf')
    LIMIT 1
);
SET @verified_status := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'status' AND `key` = 'verified'
    LIMIT 1
);
SET @community_entity_type := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_type' AND `key` = 'community'
    LIMIT 1
);
SET @join_open := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_join_policy' AND `key` = 'open'
    LIMIT 1
);

INSERT INTO social_entity (
    handle,
    type_id,
    status_id,
    location_id,
    is_adult_or_sensitive,
    created_by,
    modified_by
)
SELECT
    'operating_charter_256',
    @community_entity_type,
    @verified_status,
    @global_lid,
    0,
    @system_uid,
    @system_uid
FROM DUAL
WHERE NOT EXISTS (
    SELECT 1
    FROM social_entity se
    WHERE se.handle = 'operating_charter_256'
);

INSERT INTO social_community (
    social_entity_id,
    label,
    description,
    intention,
    rules,
    founded_at,
    join_policy_id,
    parent_community_id
)
SELECT
    se.id,
    '256 Operating Charter',
    'Foundational constitutional root for all 256 communities and moderation policy inheritance.',
    'Provide the single operating charter and constitutional source for all descendant communities on 256.',
    'All content and behavior are governed by the 256 Operating Charter baseline. Descendant communities may add constitutional amendments but cannot negate inherited charter items.',
    DATE('2026-04-25'),
    @join_open,
    NULL
FROM social_entity se
LEFT JOIN social_community sc ON sc.social_entity_id = se.id
WHERE se.handle = 'operating_charter_256'
  AND sc.social_entity_id IS NULL;

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

UPDATE social_community sc
INNER JOIN social_entity se ON se.id = sc.social_entity_id
SET sc.parent_community_id = @root_community_id
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
  AND @root_community_id IS NOT NULL;
