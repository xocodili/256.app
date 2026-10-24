-- Each foundational community's point-100 relevance clause is eligible as a
-- post-dispute trigger (seeded before judicial_trigger_chk existed; default was 0).

UPDATE community_constitution_item cci
INNER JOIN social_entity se ON se.id = cci.community_id
SET cci.judicial_trigger_chk = 1
WHERE cci.point = 100
  AND cci.status = 'active'
  AND se.handle IN (
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
  );
