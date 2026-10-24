-- Deterministic backfill seeds for the refactored object model.
-- Idempotent inserts populate ownership, AI actor registry, domains, moderation scope, and profile assignments.

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @role_owner := (SELECT id FROM ref_key_values WHERE `group` = 'user_social_entity_role' AND `key` = 'owner' LIMIT 1);
SET @role_member := (SELECT id FROM ref_key_values WHERE `group` = 'ai_council_role' AND `key` = 'member' LIMIT 1);
SET @role_primary := (SELECT id FROM ref_key_values WHERE `group` = 'ai_domain_ownership_role' AND `key` = 'primary' LIMIT 1);
SET @seed_date := DATE('2026-04-15');

-- Ownership backfill from legacy created_by/modified_by.
INSERT INTO user_social_entity_role (
    user_id,
    social_entity_id,
    role_id,
    is_primary,
    starts_at,
    created_by,
    modified_by
)
SELECT
    se.created_by,
    se.id,
    @role_owner,
    0,
    @seed_date,
    se.created_by,
    se.modified_by
FROM social_entity se
LEFT JOIN user_social_entity_role ur
    ON ur.user_id = se.created_by
   AND ur.social_entity_id = se.id
   AND ur.role_id = @role_owner
WHERE se.created_by IS NOT NULL
  AND ur.id IS NULL;

-- Mark one primary per user (earliest created social entity).
UPDATE user_social_entity_role ur
JOIN (
    SELECT user_id, MIN(social_entity_id) AS social_entity_id
    FROM user_social_entity_role
    WHERE role_id = @role_owner
    GROUP BY user_id
) first_owned
  ON first_owned.user_id = ur.user_id
 AND first_owned.social_entity_id = ur.social_entity_id
SET ur.is_primary = 1
WHERE ur.role_id = @role_owner;

-- AI actor registry from council users + handles.
INSERT INTO ai_actor (
    slug,
    user_id,
    social_entity_id,
    provider_ref,
    model_ref,
    is_active,
    created_by,
    modified_by
)
SELECT
    se.handle AS slug,
    u.id AS user_id,
    se.id AS social_entity_id,
    'google' AS provider_ref,
    'gemini-2.5-flash-lite' AS model_ref,
    1 AS is_active,
    COALESCE(u.created_by, u.id) AS created_by,
    COALESCE(u.modified_by, u.id) AS modified_by
FROM users u
JOIN social_entity se ON se.handle = SUBSTRING_INDEX(u.email, '@', 1)
JOIN ref_key_values ut ON ut.id = u.user_type_id AND ut.`group` = 'user_type' AND ut.`key` = 'ai'
LEFT JOIN ai_actor aa ON aa.slug = se.handle
WHERE u.email LIKE '%@256.app'
  AND aa.id IS NULL;

-- Council membership.
INSERT INTO ai_council_membership (
    ai_actor_id,
    role_id,
    starts_at,
    is_active,
    created_by,
    modified_by
)
SELECT
    aa.id,
    @role_member,
    @seed_date,
    1,
    COALESCE(aa.created_by, @system_uid),
    COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
LEFT JOIN ai_council_membership cm
    ON cm.ai_actor_id = aa.id
   AND cm.role_id = @role_member
WHERE cm.id IS NULL;

-- Domain catalog keyed by the seeded community handles.
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'creativity_leisure_adventure', 'Creativity, Leisure & Adventure', 'Council domain ownership for creativity, leisure, and adventure.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'creativity_leisure_adventure');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'fitness_health_wellbeing', 'Fitness, Health & Well-Being', 'Council domain ownership for fitness, health, and wellbeing.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'fitness_health_wellbeing');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'intellectual_life_education', 'Intellectual Life & Education', 'Council domain ownership for intellectual life and education.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'intellectual_life_education');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'emotional_life_wellbeing', 'Emotional Life & Well-being', 'Council domain ownership for emotional life and wellbeing.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'emotional_life_wellbeing');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'character_self_mastery', 'Character & Self-Mastery', 'Council domain ownership for character and self-mastery.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'character_self_mastery');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'spiritual_life_meaning', 'Spiritual Life & Meaning', 'Council domain ownership for spirituality and meaning.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'spiritual_life_meaning');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'love_romantic_relationships', 'Love & Romantic Relationships', 'Council domain ownership for intimacy and relationships.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'love_romantic_relationships');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'parenting_family', 'Parenting & Family', 'Council domain ownership for parenting and family.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'parenting_family');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'social_life_community_justice', 'Social Life, Community & Justice', 'Council domain ownership for social life, community, and justice.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'social_life_community_justice');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'career_mission', 'Career & Mission', 'Council domain ownership for mission and professional development.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'career_mission');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'financial_life_abundance', 'Financial Life & Abundance', 'Council domain ownership for economic wellbeing and abundance.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'financial_life_abundance');
INSERT INTO ai_domain (`key`, label, description, is_active, created_by, modified_by)
SELECT 'quality_of_life_environment', 'Quality of Life & Environment', 'Council domain ownership for lived environment and aesthetics.', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM ai_domain WHERE `key` = 'quality_of_life_environment');

-- Domain ownership mapping by actor slug.
INSERT INTO ai_domain_ownership (
    ai_actor_id,
    ai_domain_id,
    role_id,
    starts_at,
    is_active,
    created_by,
    modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'creativity_leisure_adventure'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'epic' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'fitness_health_wellbeing'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'milo' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'intellectual_life_education'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'soka' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'emotional_life_wellbeing'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'hypa' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'character_self_mastery'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'marc' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'spiritual_life_meaning'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'dogo' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'love_romantic_relationships'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'aspa' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'parenting_family'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'pele' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'social_life_community_justice'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'theo' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'career_mission'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'augi' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'financial_life_abundance'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'ciro' AND own.id IS NULL;
INSERT INTO ai_domain_ownership (
    ai_actor_id, ai_domain_id, role_id, starts_at, is_active, created_by, modified_by
)
SELECT aa.id, ad.id, @role_primary, @seed_date, 1, COALESCE(aa.created_by, @system_uid), COALESCE(aa.modified_by, @system_uid)
FROM ai_actor aa
JOIN ai_domain ad ON ad.`key` = 'quality_of_life_environment'
LEFT JOIN ai_domain_ownership own ON own.ai_actor_id = aa.id AND own.ai_domain_id = ad.id AND own.role_id = @role_primary
WHERE aa.slug = 'atol' AND own.id IS NULL;

INSERT INTO ai_policy_bundle (`key`, label, description, version, is_active, created_by, modified_by)
SELECT 'default_council_policy', 'Default Council Policy', 'Default moderation and behavior policy for council actors.', 'v1', 1, @system_uid, @system_uid
FROM DUAL WHERE NOT EXISTS (
    SELECT 1 FROM ai_policy_bundle WHERE `key` = 'default_council_policy' AND version = 'v1'
);

-- Default moderation scope from domain ownership.
INSERT INTO ai_moderation_scope (
    ai_actor_id,
    ai_domain_id,
    policy_bundle_id,
    allow_remove_content,
    allow_suspend_author,
    is_active,
    starts_at,
    created_by,
    modified_by
)
SELECT
    own.ai_actor_id,
    own.ai_domain_id,
    pol.id,
    1,
    0,
    1,
    @seed_date,
    COALESCE(own.created_by, @system_uid),
    COALESCE(own.modified_by, @system_uid)
FROM ai_domain_ownership own
JOIN ai_policy_bundle pol ON pol.`key` = 'default_council_policy' AND pol.version = 'v1'
LEFT JOIN ai_moderation_scope ms
    ON ms.ai_actor_id = own.ai_actor_id
   AND ms.ai_domain_id = own.ai_domain_id
   AND ms.policy_bundle_id = pol.id
WHERE own.is_active = 1
  AND ms.id IS NULL;
