-- Denormalize `content_domain.id` onto `post` for link validity checks without joining through `content_link`.
-- Populated when a post is tied to a known domain (from `content_link.domain_id` at write time, or backfilled here).

ALTER TABLE post
    ADD COLUMN domain_id BIGINT UNSIGNED NULL COMMENT 'FK content_domain.id; denormalized when available'
    AFTER content_link_id;

SET @has_content_link := (
    SELECT COUNT(*)
    FROM information_schema.tables
    WHERE table_schema = DATABASE()
      AND table_name = 'content_link'
);
SET @post_domain_backfill_sql := IF(
    @has_content_link = 1,
    'UPDATE post p INNER JOIN content_link cl ON cl.id = p.content_link_id SET p.domain_id = cl.domain_id WHERE p.domain_id IS NULL',
    'SELECT 1'
);
PREPARE post_domain_backfill_stmt FROM @post_domain_backfill_sql;
EXECUTE post_domain_backfill_stmt;
DEALLOCATE PREPARE post_domain_backfill_stmt;

SET @has_content_domain := (
    SELECT COUNT(*)
    FROM information_schema.tables
    WHERE table_schema = DATABASE()
      AND table_name = 'content_domain'
);
SET @post_domain_fk_sql := IF(
    @has_content_domain = 1,
    'ALTER TABLE post ADD CONSTRAINT fk_post_domain FOREIGN KEY (domain_id) REFERENCES content_domain (id) ON DELETE RESTRICT, ADD KEY idx_post_domain_id (domain_id)',
    'ALTER TABLE post ADD KEY idx_post_domain_id (domain_id)'
);
PREPARE post_domain_fk_stmt FROM @post_domain_fk_sql;
EXECUTE post_domain_fk_stmt;
DEALLOCATE PREPARE post_domain_fk_stmt;
