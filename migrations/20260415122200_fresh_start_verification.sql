-- Fresh-start verification checklist (read-only assertions).
-- Safe for greenfield runs; returns counts for expected bootstrap invariants.

SELECT 'users_total' AS check_name, COUNT(*) AS value FROM users
UNION ALL
SELECT 'ai_users_total', COUNT(*)
FROM users u
JOIN ref_key_values ut ON ut.id = u.user_type_id
WHERE ut.`group` = 'user_type' AND ut.`key` = 'ai'
UNION ALL
SELECT 'ai_social_entities_total', COUNT(*)
FROM social_entity se
JOIN ref_key_values et ON et.id = se.type_id
WHERE et.`group` = 'social_entity_type' AND et.`key` = 'digital'
UNION ALL
SELECT 'ai_actor_total', COUNT(*) FROM ai_actor
UNION ALL
SELECT 'council_membership_total', COUNT(*) FROM ai_council_membership WHERE is_active = 1
UNION ALL
SELECT 'ai_domains_total', COUNT(*) FROM ai_domain WHERE is_active = 1
UNION ALL
SELECT 'ai_domain_ownership_total', COUNT(*) FROM ai_domain_ownership WHERE is_active = 1
UNION ALL
SELECT 'council_communities_total', COUNT(*)
FROM social_community sc
JOIN social_entity se ON se.id = sc.social_entity_id
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
UNION ALL
SELECT 'profile_assignments_total', COUNT(*) FROM ai_profile_assignment WHERE is_active = 1
UNION ALL
SELECT 'moderation_scopes_total', COUNT(*) FROM ai_moderation_scope WHERE is_active = 1;
