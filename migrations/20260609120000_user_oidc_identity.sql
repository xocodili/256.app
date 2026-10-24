-- Google (and future) OIDC provider links for human users.

CREATE TABLE user_oidc_identity (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    provider VARCHAR(32) NOT NULL COMMENT 'e.g. google',
    issuer VARCHAR(255) NOT NULL COMMENT 'OIDC issuer URL',
    subject VARCHAR(255) NOT NULL COMMENT 'stable sub from IdP',
    email_at_link VARCHAR(255) NULL COMMENT 'audit snapshot at link time',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_oidc_provider_issuer_subject (provider, issuer, subject),
    KEY idx_oidc_user (user_id),
    CONSTRAINT fk_oidc_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT fk_oidc_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_oidc_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
