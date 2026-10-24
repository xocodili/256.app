-- Seed default user_faces rows for AI council members.
-- Uses deterministic placeholder embeddings and per-member default avatar paths.

SET @seed_ts := TIMESTAMP('2026-04-15 00:00:00');

CREATE TEMPORARY TABLE tmp_council_faces (
    handle VARCHAR(32) NOT NULL PRIMARY KEY,
    appearance_description TEXT NOT NULL
);

INSERT INTO tmp_council_faces (handle, appearance_description) VALUES
('milo', 'Milo (256-AI): calm, practical, supportive wellbeing guide focused on body literacy, movement, and sustainable health routines.'),
('soka', 'Soka (256-AI): clear, curious, and analytical learning companion focused on understanding, critical thinking, and education.'),
('hypa', 'Hypa (256-AI): compassionate emotional mentor focused on self-awareness, emotional regulation, and resilient mental wellbeing.'),
('marc', 'Marc (256-AI): disciplined growth partner focused on character, integrity, and habit design for long-term self-mastery.'),
('dogo', 'Dogo (256-AI): reflective meaning-maker focused on spiritual inquiry, purpose, and existential coherence.'),
('aspa', 'Aspa (256-AI): relational guide focused on intimacy, partnership, communication, and healthy romantic bonds.'),
('pele', 'Pele (256-AI): family and caregiving mentor focused on parenting, intergenerational support, and nurturing leadership.'),
('theo', 'Theo (256-AI): civic-minded connector focused on friendship, belonging, social trust, and justice-oriented community life.'),
('augi', 'Augi (256-AI): mission and craft coach focused on meaningful work, contribution, and professional development.'),
('ciro', 'Ciro (256-AI): financial clarity advisor focused on resource stewardship, economic security, and value creation.'),
('atol', 'Atol (256-AI): environmental quality curator focused on home/work habitat quality, aesthetics, and livable surroundings.'),
('epic', 'Epic (256-AI): creative catalyst focused on play, exploration, and translating canonical appearance into expressive portrait art.');

INSERT INTO user_faces (
    user_id,
    embedding,
    embedding_dim,
    embedding_model_id,
    ai_appearance_description,
    generated_artwork_path,
    ai_model_id,
    quality_score,
    captured_at,
    created_by,
    modified_by
)
SELECT
    u.id,
    X'',
    0,
    'council-seed-placeholder-v1',
    t.appearance_description,
    CONCAT('static/generated/council/', t.handle, '/avatar-default.png'),
    'council-seed-avatar-v1',
    NULL,
    @seed_ts,
    u.id,
    u.id
FROM tmp_council_faces t
JOIN users u ON u.email = CONCAT(t.handle, '@256.app')
ON DUPLICATE KEY UPDATE
    embedding = VALUES(embedding),
    embedding_dim = VALUES(embedding_dim),
    embedding_model_id = VALUES(embedding_model_id),
    ai_appearance_description = VALUES(ai_appearance_description),
    generated_artwork_path = VALUES(generated_artwork_path),
    ai_model_id = VALUES(ai_model_id),
    quality_score = VALUES(quality_score),
    captured_at = VALUES(captured_at),
    modified_by = VALUES(modified_by);

DROP TEMPORARY TABLE tmp_council_faces;
