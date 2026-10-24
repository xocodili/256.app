-- Council AI batch jobs, daily budget ledger, acute-harm content rating audit, link/domain neutral floor.

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
('ops_job_type', 'godmother_welcome', 52, 1, NULL, NULL),
('ops_job_type', 'council_reply_tick', 53, 1, NULL, NULL),
('ops_job_type', 'council_post_tick', 54, 1, NULL, NULL),
('ops_job_type', 'council_invite_tick', 55, 1, NULL, NULL),
('ops_job_type', 'content_domain_lifecycle', 56, 1, NULL, NULL);

CREATE TABLE IF NOT EXISTS council_ai_daily_budget (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    ai_actor_id BIGINT UNSIGNED NOT NULL,
    `utc_date` DATE NOT NULL,
    reply_credit DECIMAL(8,4) NOT NULL DEFAULT 0.0000,
    replies_spent INT UNSIGNED NOT NULL DEFAULT 0,
    posts_spent INT UNSIGNED NOT NULL DEFAULT 0,
    invites_spent INT UNSIGNED NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY uq_council_ai_daily_budget_actor_date (ai_actor_id, `utc_date`),
    CONSTRAINT fk_council_ai_daily_budget_actor
        FOREIGN KEY (ai_actor_id) REFERENCES ai_actor (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS acute_harm_content_rating_event (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    judicial_case_id BIGINT UNSIGNED NOT NULL,
    content_link_id BIGINT UNSIGNED NULL,
    content_domain_id BIGINT UNSIGNED NULL,
    direction ENUM('apply', 'refund') NOT NULL,
    link_delta DECIMAL(6,4) NOT NULL DEFAULT 0.0000,
    domain_delta DECIMAL(6,4) NOT NULL DEFAULT 0.0000,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY uq_acute_harm_content_rating_case_dir (judicial_case_id, direction),
    CONSTRAINT fk_acute_harm_content_rating_case
        FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Neutral floor for outbound link trust (was 0.00 at ingest).
UPDATE content_domain SET rating = 2.50 WHERE rating = 0.00;
UPDATE content_link SET rating = 2.50 WHERE rating = 0.00;
