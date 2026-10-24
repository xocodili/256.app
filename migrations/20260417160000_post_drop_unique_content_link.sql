-- Many posts may reference the same canonical row in content_link (shared URL / deduped metadata).
-- Drop the mistaken one-link-per-post unique index; keep a non-unique index for analytics joins.

-- MySQL will not drop an index that is required by a FK. If the optional FK exists, drop and recreate it.
SET @fk_exists := (
    SELECT COUNT(*)
    FROM information_schema.TABLE_CONSTRAINTS
    WHERE CONSTRAINT_SCHEMA = DATABASE()
      AND TABLE_NAME = 'post'
      AND CONSTRAINT_NAME = 'fk_post_content_link'
      AND CONSTRAINT_TYPE = 'FOREIGN KEY'
);
SET @drop_fk := IF(@fk_exists > 0, 'ALTER TABLE post DROP FOREIGN KEY fk_post_content_link', 'SELECT 1');
PREPARE drop_fk_stmt FROM @drop_fk;
EXECUTE drop_fk_stmt;
DEALLOCATE PREPARE drop_fk_stmt;

SET @uq := (
    SELECT COUNT(*)
    FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'post'
      AND index_name = 'uq_post_content_link'
);

SET @drop_uq := IF(@uq > 0, 'ALTER TABLE post DROP INDEX uq_post_content_link', 'SELECT 1');
PREPARE drop_uq_stmt FROM @drop_uq;
EXECUTE drop_uq_stmt;
DEALLOCATE PREPARE drop_uq_stmt;

SET @has_idx := (
    SELECT COUNT(*)
    FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'post'
      AND index_name = 'idx_post_content_link_id'
);

SET @add_idx := IF(@has_idx = 0, 'ALTER TABLE post ADD INDEX idx_post_content_link_id (content_link_id)', 'SELECT 1');
PREPARE add_idx_stmt FROM @add_idx;
EXECUTE add_idx_stmt;
DEALLOCATE PREPARE add_idx_stmt;

SET @has_content_link := (
    SELECT COUNT(*)
    FROM information_schema.tables
    WHERE table_schema = DATABASE()
      AND table_name = 'content_link'
);
SET @add_fk := IF(
    @fk_exists = 0 AND @has_content_link > 0,
    'ALTER TABLE post ADD CONSTRAINT fk_post_content_link FOREIGN KEY (content_link_id) REFERENCES content_link (id) ON DELETE RESTRICT',
    'SELECT 1'
);
PREPARE add_fk_stmt FROM @add_fk;
EXECUTE add_fk_stmt;
DEALLOCATE PREPARE add_fk_stmt;
