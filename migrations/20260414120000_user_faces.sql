-- One row per successful face capture (512x512 pipeline): client embedding + vision text + generated art path.
-- Greenfield-safe forward create (no destructive DROP here).

CREATE TABLE user_faces (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    embedding BLOB NOT NULL COMMENT 'Packed little-endian float32[embedding_dim]',
    embedding_dim SMALLINT UNSIGNED NOT NULL,
    embedding_model_id VARCHAR(64) NOT NULL,
    ai_appearance_description TEXT NOT NULL,
    generated_artwork_path VARCHAR(512) NULL COMMENT 'Relative filesystem path to 512x512 generated artwork',
    ai_model_id VARCHAR(64) NOT NULL,
    quality_score FLOAT NULL,
    captured_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_user_faces_user_id (user_id),
    KEY idx_user_faces_user_id (user_id),
    KEY idx_user_faces_created_by (created_by),
    KEY idx_user_faces_modified_by (modified_by),
    CONSTRAINT fk_user_faces_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT fk_user_faces_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_user_faces_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
