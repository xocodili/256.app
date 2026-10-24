-- AI prompt/style profile registry + actor assignment.

CREATE TABLE ai_prompt_profile (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `key` VARCHAR(128) NOT NULL,
    version VARCHAR(64) NOT NULL DEFAULT 'v1',
    prompt_template TEXT NOT NULL,
    metadata JSON NULL,
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ai_prompt_profile_key_version (`key`, version),
    KEY idx_ai_prompt_profile_active (is_active),
    CONSTRAINT fk_ai_prompt_profile_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_prompt_profile_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_image_style_profile (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `key` VARCHAR(128) NOT NULL,
    version VARCHAR(64) NOT NULL DEFAULT 'v1',
    perspective_direction TEXT NOT NULL,
    style_direction TEXT NOT NULL,
    color_direction TEXT NOT NULL,
    generation_params JSON NULL,
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_ai_image_style_profile_key_version (`key`, version),
    KEY idx_ai_image_style_profile_active (is_active),
    CONSTRAINT fk_ai_image_style_profile_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_image_style_profile_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ai_profile_assignment (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    ai_actor_id BIGINT UNSIGNED NOT NULL,
    prompt_profile_id BIGINT UNSIGNED NOT NULL,
    image_style_profile_id BIGINT UNSIGNED NULL,
    policy_bundle_id BIGINT UNSIGNED NULL,
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    starts_at DATE NULL,
    ends_at DATE NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_ai_profile_assignment_actor (ai_actor_id),
    KEY idx_ai_profile_assignment_prompt (prompt_profile_id),
    KEY idx_ai_profile_assignment_image (image_style_profile_id),
    KEY idx_ai_profile_assignment_policy (policy_bundle_id),
    KEY idx_ai_profile_assignment_active (is_active),
    CONSTRAINT chk_ai_profile_assignment_dates CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at >= starts_at),
    CONSTRAINT fk_ai_profile_assignment_actor FOREIGN KEY (ai_actor_id) REFERENCES ai_actor (id) ON DELETE CASCADE,
    CONSTRAINT fk_ai_profile_assignment_prompt FOREIGN KEY (prompt_profile_id) REFERENCES ai_prompt_profile (id) ON DELETE RESTRICT,
    CONSTRAINT fk_ai_profile_assignment_image FOREIGN KEY (image_style_profile_id) REFERENCES ai_image_style_profile (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_profile_assignment_policy FOREIGN KEY (policy_bundle_id) REFERENCES ai_policy_bundle (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_profile_assignment_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_ai_profile_assignment_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
