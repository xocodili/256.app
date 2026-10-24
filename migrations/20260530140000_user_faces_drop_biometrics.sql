-- Biometric-liability removal (litigation risk):
--   * Stop storing face embeddings (biometric templates) and the AI-generated appearance description.
--   * Replace the embedding-based onboarding completion signal with an explicit `face_captured_at`.
--
-- Raw capture images were already transient (request-only, never persisted). This migration removes
-- the remaining stored biometric/appearance columns so they can never come back, and introduces a
-- dedicated completion timestamp so the onboarding / composite-verification gates do not depend on
-- any biometric data.

ALTER TABLE user_faces
    ADD COLUMN face_captured_at TIMESTAMP NULL DEFAULT NULL
        COMMENT 'Set when a face capture completes successfully; onboarding completion signal (no biometric data)'
        AFTER quality_score;

-- Treat all existing rows (council seeds, already-onboarded users) as captured.
UPDATE user_faces
SET face_captured_at = COALESCE(captured_at, UTC_TIMESTAMP())
WHERE face_captured_at IS NULL;

ALTER TABLE user_faces
    DROP COLUMN embedding,
    DROP COLUMN embedding_dim,
    DROP COLUMN embedding_model_id,
    DROP COLUMN ai_appearance_description;
