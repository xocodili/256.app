-- Generic ops job queue: ref-backed job types, SHA-256 canonical payload dedupe,
-- active rows (pending | leased), success archive with 30-day purge from conductor.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('ops_job_type', 'noop', 10, 1, NULL, NULL);

CREATE TABLE job_queue (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NULL COMMENT 'FK users.id — owner for cancel; NULL for system jobs',
    job_type_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group ops_job_type',
    payload JSON NOT NULL,
    payload_hash CHAR(64) NOT NULL COMMENT 'lower-hex SHA-256 of canonical sorted JSON',
    correlation_id CHAR(36) NULL,
    status VARCHAR(16) NOT NULL COMMENT 'pending | leased',
    priority SMALLINT NOT NULL DEFAULT 100,
    next_run_at DATETIME(6) NOT NULL,
    lease_until DATETIME(6) NULL,
    attempt_count TINYINT UNSIGNED NOT NULL DEFAULT 0,
    max_attempts TINYINT UNSIGNED NOT NULL DEFAULT 3,
    last_error_code VARCHAR(64) NULL,
    last_error_message VARCHAR(512) NULL,
    last_attempt_at DATETIME(6) NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY uq_job_queue_type_hash (job_type_id, payload_hash),
    KEY idx_job_queue_eligible (status, next_run_at, priority, id),
    KEY idx_job_queue_user (user_id),
    CONSTRAINT fk_job_queue_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT fk_job_queue_job_type FOREIGN KEY (job_type_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT chk_job_queue_status CHECK (status IN ('pending', 'leased'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE job_queue_archive (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    source_job_id BIGINT UNSIGNED NOT NULL COMMENT 'job_queue.id at completion',
    user_id BIGINT UNSIGNED NULL,
    job_type_id BIGINT UNSIGNED NOT NULL,
    payload JSON NOT NULL,
    payload_hash CHAR(64) NOT NULL,
    correlation_id CHAR(36) NULL,
    attempt_count TINYINT UNSIGNED NOT NULL,
    completed_at DATETIME(6) NOT NULL,
    archived_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    KEY idx_job_queue_archive_archived (archived_at),
    KEY idx_job_queue_archive_source (source_job_id),
    KEY idx_job_queue_archive_type (job_type_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
