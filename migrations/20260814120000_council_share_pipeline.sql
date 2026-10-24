-- Council share pipeline: job types, feed registry, link extract status.

INSERT IGNORE INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('ops_job_type', 'council_share_shortlist', 54, 1, NULL, NULL),
    ('ops_job_type', 'council_share_read', 54, 1, NULL, NULL),
    ('ops_job_type', 'council_share_publish', 54, 1, NULL, NULL),
    ('ops_job_type', 'council_portfolio_feed_poll', 54, 1, NULL, NULL);

CREATE TABLE content_domain_feed (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    content_domain_id BIGINT UNSIGNED NOT NULL,
    feed_url VARCHAR(2048) NOT NULL,
    feed_type ENUM('rss', 'atom') NOT NULL DEFAULT 'rss',
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    last_polled_at DATETIME(6) NULL,
    last_error TEXT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_domain_feed_url (content_domain_id, feed_url(512)),
    KEY idx_domain_feed_active (content_domain_id, is_active),
    CONSTRAINT fk_domain_feed_domain FOREIGN KEY (content_domain_id) REFERENCES content_domain (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

ALTER TABLE content_link
    ADD COLUMN extract_status VARCHAR(32) NULL
        COMMENT 'ok|paywalled|extract_failed|pending'
        AFTER fetch_status;

-- Seed RSS feeds for a subset of Epic portfolio hosts (expand via ops as needed).
INSERT INTO content_domain_feed (content_domain_id, feed_url, feed_type, is_active)
SELECT cd.id, feeds.feed_url, feeds.feed_type, 1
FROM content_domain cd
INNER JOIN (
    SELECT 'theguardian.com' AS domain, 'https://www.theguardian.com/world/rss' AS feed_url, 'rss' AS feed_type
    UNION ALL SELECT 'bbc.com', 'https://feeds.bbci.co.uk/news/rss.xml', 'rss'
    UNION ALL SELECT 'bbc.co.uk', 'https://feeds.bbci.co.uk/news/rss.xml', 'rss'
    UNION ALL SELECT 'reuters.com', 'https://www.reutersagency.com/feed/', 'rss'
) feeds ON feeds.domain = cd.domain
WHERE NOT EXISTS (
    SELECT 1 FROM content_domain_feed cdf
    WHERE cdf.content_domain_id = cd.id AND cdf.feed_url = feeds.feed_url
);
