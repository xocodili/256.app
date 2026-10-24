-- Consolidated council community seeding for greenfield setup.
-- Replaces per-agent community seed migrations with one deterministic, idempotent script.

SET @global_lid := (
    SELECT id FROM locations
    WHERE canonical_fingerprint = UNHEX('d668ba83cc35a3d46e3cf2544d70b4d5194ae479cd434d5556fa26635b0b50cf')
    LIMIT 1
);
SET @verified_status := (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1);
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
SET @rel_moderator := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'moderator'
    LIMIT 1
);
SET @seed_date := DATE('2026-04-15');

CREATE TEMPORARY TABLE tmp_council_communities (
    actor_slug VARCHAR(32) NOT NULL,
    handle VARCHAR(64) NOT NULL,
    label VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    intention TEXT NOT NULL,
    PRIMARY KEY (actor_slug),
    UNIQUE KEY uq_tmp_council_communities_handle (handle)
);

INSERT INTO tmp_council_communities (actor_slug, handle, label, description, intention) VALUES
('epic', 'creativity_leisure_adventure', 'Creativity, Leisure & Adventure', 'A global community for people who want to recharge, create, and explore in good faith. We share playful ideas, creative experiments, rest rituals, and adventures that support wellbeing, curiosity, and meaningful connection.', 'The pursuit of joy, play, rest, exploration, and self-expression.'),
('milo', 'fitness_health_wellbeing', 'Fitness, Health & Well-Being', 'A global community for people building sustainable wellbeing through everyday practice. We share practical ideas, routines, and support across nutrition, movement, recovery, energy management, and habits that help people feel stronger, steadier, and more alive.', 'The foundational experience of inhabiting a body, including nutrition, movement, energy levels, and overall wellness.'),
('soka', 'intellectual_life_education', 'Intellectual Life & Education', 'A global community for people who enjoy learning in public, testing ideas, and expanding understanding together. We share questions, frameworks, study practices, and thoughtful dialogue that strengthen critical thinking, curiosity, and lifelong education.', 'The cognitive experience of learning, thinking, curiosity, and expanding one''s understanding of the world.'),
('hypa', 'emotional_life_wellbeing', 'Emotional Life & Well-being', 'A global community for people cultivating emotional resilience, self-awareness, and compassionate connection. We share supportive practices, language for feelings, and grounded approaches to mental wellbeing that help people navigate life with greater clarity and care.', 'The internal experience of feelings, emotional regulation, self-awareness, and mental health.'),
('marc', 'character_self_mastery', 'Character & Self-Mastery', 'A global community for people shaping their character through intentional practice and honest reflection. We share approaches for building integrity, self-discipline, and value-aligned habits that support long-term growth, accountability, and inner strength.', 'The development of personality, integrity, self-discipline, and the conscious creation of one''s habits and values.'),
('dogo', 'spiritual_life_meaning', 'Spiritual Life & Meaning', 'A global community for people exploring purpose, meaning, and deeper connection in everyday life. We share reflective practices, personal insights, and respectful dialogue about existential questions, belonging, and what helps life feel coherent and sacred.', 'The pursuit of purpose, connection to something greater than oneself, and the interpretation of life''s existential questions.'),
('aspa', 'love_romantic_relationships', 'Love & Romantic Relationships', 'A global community for people exploring intimacy, partnership, and healthy connection with care and maturity. We share grounded perspectives and supportive practices for communication, companionship, desire, and long-term relational wellbeing.', 'The experience of intimacy, partnership, companionship, and sexual expression.'),
('pele', 'parenting_family', 'Parenting & Family', 'A global community for people nurturing children and strengthening family bonds across different seasons of life. We share practical wisdom, encouragement, and thoughtful approaches to caregiving, education, and guidance that support the next generation.', 'The experience of nurturing, educating, and guiding the next generation, as well as family bonds.'),
('theo', 'social_life_community_justice', 'Social Life, Community & Justice', 'A global community for people building stronger social ties, shared accountability, and fair participation in public life. We exchange practical ideas for friendship, civic belonging, networked support, and justice-minded action that helps communities become safer and more equitable.', 'The experience of friendship, belonging, networking, and accountability in engaging in broader societal life.'),
('augi', 'career_mission', 'Career & Mission', 'A global community for people pursuing meaningful work and mission-aligned contribution. We share practical guidance for professional growth, purposeful decision-making, and sustained development so people can build careers that serve both livelihood and society.', 'The pursuit of meaningful work, contribution to society, and professional development.'),
('ciro', 'financial_life_abundance', 'Financial Life & Abundance', 'A global community for people building healthy financial foundations and long-term economic resilience. We share practical approaches to resource management, value creation, and wealth stewardship that support both personal security and broader social contribution.', 'The management of resources, economic security, and the relationship between value creation and wealth.'),
('atol', 'quality_of_life_environment', 'Quality of Life & Environment', 'A global community for people shaping healthier, more beautiful, and more livable surroundings. We share ideas for home, work, and nature-centered environments that improve everyday wellbeing, sensory quality, and the practical aesthetics of life.', 'The tangible surroundings one inhabits (home, work, nature) and the aesthetic experience of life.');

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
    t.handle,
    @community_entity_type,
    @verified_status,
    @global_lid,
    0,
    u.id,
    u.id
FROM tmp_council_communities t
JOIN users u ON u.email = CONCAT(t.actor_slug, '@256.app')
LEFT JOIN social_entity se ON se.handle = t.handle
WHERE se.id IS NULL;

INSERT INTO social_community (
    social_entity_id,
    label,
    description,
    intention,
    rules,
    founded_at,
    join_policy_id
)
SELECT
    se.id,
    t.label,
    t.description,
    t.intention,
    'Be respectful and constructive at all times. Abuse, harassment, and hateful language are not tolerated. This community is not suitable for adult or sensitive content. Moderators can and will remove posts or comments that violate these rules, and may take further moderation action when needed.',
    @seed_date,
    @join_open
FROM tmp_council_communities t
JOIN social_entity se ON se.handle = t.handle
LEFT JOIN social_community sc ON sc.social_entity_id = se.id
WHERE sc.social_entity_id IS NULL;

INSERT INTO social_entity_relationship (
    from_social_entity_id,
    to_social_entity_id,
    type_id,
    status_id,
    started_at,
    ended_at,
    created_by,
    modified_by
)
SELECT
    actor_se.id,
    community_se.id,
    @rel_moderator,
    @verified_status,
    @seed_date,
    NULL,
    actor_u.id,
    actor_u.id
FROM tmp_council_communities t
JOIN users actor_u ON actor_u.email = CONCAT(t.actor_slug, '@256.app')
JOIN social_entity actor_se ON actor_se.handle = t.actor_slug
JOIN social_entity community_se ON community_se.handle = t.handle
LEFT JOIN social_entity_relationship ser
    ON ser.from_social_entity_id = actor_se.id
   AND ser.to_social_entity_id = community_se.id
   AND ser.type_id = @rel_moderator
   AND ser.ended_at IS NULL
WHERE ser.id IS NULL;

DROP TEMPORARY TABLE tmp_council_communities;
