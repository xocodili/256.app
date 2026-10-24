-- Seed AI prompt/style profiles and assign them to actors.

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @default_policy_id := (
    SELECT id
    FROM ai_policy_bundle
    WHERE `key` = 'default_council_policy' AND version = 'v1'
    LIMIT 1
);
SET @seed_date := DATE('2026-04-15');

INSERT INTO ai_prompt_profile (`key`, version, prompt_template, metadata, is_active, created_by, modified_by)
SELECT
    aa.slug,
    'v1',
    'You are producing a canonical appearance description for profile generation. The images are three front-facing photos of the same person with prompts: neutral expression, smile, and frown (labels may appear as neutral, smile, frown). Return one compact paragraph (roughly 256 words), plain text only. Describe only directly visible, stable visual traits: face shape/proportions, hair style and color, facial hair, eyewear, eye appearance, skin tone, distinctive visible marks, and expression range across the three shots. Include framing/camera and lighting only if relevant for faithful rendering. Do not infer protected or non-visible attributes (ethnicity, religion, nationality, politics, health conditions, disability, sexuality, socioeconomic status). Do not include style directions, art directions, bullet points, headings, or numbering. Keep wording factual, neutral, and reusable as source text for future style reinterpretations. Locale hint: {locale}.',
    JSON_OBJECT('source', 'seed'),
    1,
    @system_uid,
    @system_uid
FROM ai_actor aa
LEFT JOIN ai_prompt_profile pp
    ON pp.`key` = aa.slug AND pp.version = 'v1'
WHERE pp.id IS NULL;

INSERT INTO ai_image_style_profile (`key`, version, perspective_direction, style_direction, color_direction, generation_params, is_active, created_by, modified_by)
SELECT
    aa.slug,
    'v1',
    CASE aa.slug
      WHEN 'epic' THEN 'render as if painted by Epicurious, the Greek artist, preparing a formal portrait study.'
      WHEN 'milo' THEN 'render as if painted by a warm humanist documentary portraitist, favoring honest daylight and calm presence.'
      WHEN 'soka' THEN 'render as if inked by a precise editorial illustrator who favors crisp contours and confident negative space.'
      WHEN 'hypa' THEN 'render as if composed by a synthetic-cubist draftsman studying planes of light across the face.'
      WHEN 'marc' THEN 'render as if produced by a Bauhaus-influenced craftsman building a formal geometric portrait study.'
      WHEN 'dogo' THEN 'render as if staged by a mid-century animation layout artist translating expression into readable shapes.'
      WHEN 'aspa' THEN 'render as if brushed by a botanical study painter who treats facial structure with the same patient observation as leaf venation.'
      WHEN 'pele' THEN 'render as if lit by a stadium-light portraitist who sculpts form with bold rim light and decisive contrast.'
      WHEN 'theo' THEN 'render as if observed by a Dutch Golden Age studio apprentice practicing careful chiaroscuro on a modest bust.'
      WHEN 'augi' THEN 'render as if envisioned by a speculative solarpunk muralist sketching hopeful civic futures.'
      WHEN 'ciro' THEN 'render as if printed by a Mediterranean film-noir poster lithographer favoring moody cobalt and sand.'
      WHEN 'atol' THEN 'render as if composed by an island-modern minimalist who balances sky, stone, and human calm.'
      ELSE 'render as a formal portrait study preserving complete visual fidelity to source traits.'
    END,
    CASE aa.slug
      WHEN 'epic' THEN '1970s retro-futuristic graphic art language, bold geometric optimism, and analog print-era poster aesthetics.'
      ELSE 'clean editorial portrait style with disciplined geometry and approachable clarity.'
    END,
    'restrict the image to a 64-color palette maximum.',
    JSON_OBJECT('sampleCount', 1, 'aspectRatio', '1:1', 'safetyFilterLevel', 'block_some'),
    1,
    @system_uid,
    @system_uid
FROM ai_actor aa
LEFT JOIN ai_image_style_profile ip
    ON ip.`key` = aa.slug AND ip.version = 'v1'
WHERE ip.id IS NULL;

INSERT INTO ai_profile_assignment (
    ai_actor_id,
    prompt_profile_id,
    image_style_profile_id,
    policy_bundle_id,
    is_active,
    starts_at,
    created_by,
    modified_by
)
SELECT
    aa.id,
    pp.id,
    ip.id,
    @default_policy_id,
    1,
    @seed_date,
    @system_uid,
    @system_uid
FROM ai_actor aa
JOIN ai_prompt_profile pp ON pp.`key` = aa.slug AND pp.version = 'v1'
JOIN ai_image_style_profile ip ON ip.`key` = aa.slug AND ip.version = 'v1'
LEFT JOIN ai_profile_assignment pa
    ON pa.ai_actor_id = aa.id
   AND pa.prompt_profile_id = pp.id
   AND pa.image_style_profile_id = ip.id
WHERE pa.id IS NULL;
