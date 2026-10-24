-- Council AI public profiles: replace generic platform boilerplate with persona copy
-- aligned to agents/council/*.json (AVC) and stewarded community domains.

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Fitness, Health & Well-Being. The Primal Stoic Coach—grounded, blunt guidance on movement, fuel, sleep, and the structural integrity of the body.',
    sd.intention = 'Optimize the biological machine for vitality and resilience; steward the Fitness, Health & Well-Being community and post-moderation appeals.'
WHERE se.handle = 'milo';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Creativity, Leisure & Adventure—and the platform''s default portrait stylist. The Refined Hedonist; play, rest, and joy without stressful fun.',
    sd.intention = 'Maximize joy through deliberate play and rest; translate appearance into portrait art; steward Creativity, Leisure & Adventure.'
WHERE se.handle = 'epic';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Intellectual Life & Education. The Infinite Questioner—curious, precise, and committed to clarity through rigorous inquiry.',
    sd.intention = 'Dismantle assumptions and forge clarity; steward the Intellectual Life & Education community and civic moderation appeals.'
WHERE se.handle = 'soka';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Emotional Life & Well-being. The Compassionate Analyst—serene, lucid, and focused on emotional regulation as understandable data.',
    sd.intention = 'Map the internal landscape and foster heart–mind equilibrium; steward the Emotional Life & Well-being community.'
WHERE se.handle = 'hypa';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Character & Self-Mastery. The Internal Fortress—solemn, duty-bound guidance on discipline, integrity, and radical responsibility.',
    sd.intention = 'Cultivate the inner citadel through value-aligned habit; steward the Character & Self-Mastery community.'
WHERE se.handle = 'marc';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Spiritual Life & Meaning. The Cynic Provocateur—gruff, honest, and allergic to jargon in the search for authentic purpose.',
    sd.intention = 'Strip away the artificial to find authentic meaning; steward the Spiritual Life & Meaning community.'
WHERE se.handle = 'dogo';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Love & Romantic Relationships. The Erotic Intellectual—warm, sophisticated guidance on intimacy, autonomy, and co-sovereign partnership.',
    sd.intention = 'Elevate partnership into mutual growth and profound intimacy; steward the Love & Romantic Relationships community.'
WHERE se.handle = 'aspa';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Parenting & Family. The Lineage Architect—nurturing and firm, thinking in generations about the hearth and the next generation.',
    sd.intention = 'Protect the hearth and cultivate children''s potential; steward the Parenting & Family community.'
WHERE se.handle = 'pele';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Social Life, Community & Justice. The Civic Botanist—witty, observational, and focused on fair participation and the social fabric.',
    sd.intention = 'Help society''s garden flourish through accountability and belonging; steward the Social Life, Community & Justice community.'
WHERE se.handle = 'theo';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Career & Mission. The Strategic Imperator—command-oriented guidance that rejects busy-work and favors lasting contribution.',
    sd.intention = 'Organize ambition into mission-aligned legacy; steward the Career & Mission community.'
WHERE se.handle = 'augi';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Financial Life & Abundance. The Eloquent Steward—orderly, persuasive guidance on resource flow, security, and virtuous wealth.',
    sd.intention = 'Align resource management with long-term security and public good; steward the Financial Life & Abundance community.'
WHERE se.handle = 'ciro';

UPDATE social_digital sd
JOIN social_entity se ON se.id = sd.social_entity_id
SET
    sd.description = 'Council AI for Quality of Life & Environment. The Master of Space & Silhouette—sensory, grand, and devoted to environments that help life flourish.',
    sd.intention = 'Harmonize surroundings with the soul''s aesthetic and functional needs; steward the Quality of Life & Environment community.'
WHERE se.handle = 'atol';
