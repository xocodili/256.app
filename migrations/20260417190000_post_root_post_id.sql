-- Dialogue-root pointer for fast thread anchoring in day-sliced feeds.
-- NULL means "this post is itself the root"; non-NULL points at the root post id.

SET @has_root_col := (
    SELECT COUNT(*)
    FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'post'
      AND column_name = 'root_post_id'
);
SET @add_root_col_sql := IF(
    @has_root_col = 0,
    'ALTER TABLE post ADD COLUMN root_post_id BIGINT UNSIGNED NULL COMMENT ''Dialogue root post id; NULL means this post is the root'' AFTER parent_post_id',
    'SELECT 1'
);
PREPARE add_root_col_stmt FROM @add_root_col_sql;
EXECUTE add_root_col_stmt;
DEALLOCATE PREPARE add_root_col_stmt;

-- Helpful for feed lookups that jump from recent replies to older roots.
SET @has_root_idx := (
    SELECT COUNT(*)
    FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'post'
      AND index_name = 'idx_post_root_post_id'
);
SET @add_root_idx_sql := IF(
    @has_root_idx = 0,
    'ALTER TABLE post ADD KEY idx_post_root_post_id (root_post_id)',
    'SELECT 1'
);
PREPARE add_root_idx_stmt FROM @add_root_idx_sql;
EXECUTE add_root_idx_stmt;
DEALLOCATE PREPARE add_root_idx_stmt;

-- Self FK so root pointers remain valid as posts are removed.
SET @has_root_fk := (
    SELECT COUNT(*)
    FROM information_schema.table_constraints
    WHERE table_schema = DATABASE()
      AND table_name = 'post'
      AND constraint_name = 'fk_post_root_post'
      AND constraint_type = 'FOREIGN KEY'
);
SET @add_root_fk_sql := IF(
    @has_root_fk = 0,
    'ALTER TABLE post ADD CONSTRAINT fk_post_root_post FOREIGN KEY (root_post_id) REFERENCES post (id) ON DELETE SET NULL',
    'SELECT 1'
);
PREPARE add_root_fk_stmt FROM @add_root_fk_sql;
EXECUTE add_root_fk_stmt;
DEALLOCATE PREPARE add_root_fk_stmt;
