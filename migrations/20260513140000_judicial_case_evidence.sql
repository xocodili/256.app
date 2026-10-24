-- Immutable capture of extrinsic URLs for judicial / moderation cases (evidentiary integrity).
-- Worker flow (planned): scrape at filing time → store text/html + screenshot reference → council review uses stored text, not live URLs.

CREATE TABLE judicial_case_evidence (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    judicial_case_id BIGINT UNSIGNED NOT NULL COMMENT 'FK judicial_case.id',
    source_url VARCHAR(2048) NOT NULL,
    captured_html MEDIUMTEXT NULL COMMENT 'Raw HTML snapshot at capture time',
    captured_text MEDIUMTEXT NULL COMMENT 'Normalized plain text / reader-mode body',
    screenshot_object_key VARCHAR(512) NULL COMMENT 'Relative or object-store key for PNG capture',
    content_sha256 BINARY(32) NULL COMMENT 'Fingerprint of canonical text used for audit',
    captured_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_jce_case (judicial_case_id),
    KEY idx_jce_captured (captured_at),
    CONSTRAINT fk_jce_case FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jce_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_jce_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
