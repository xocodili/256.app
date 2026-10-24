-- Per-post origin country (ISO 3166-1 alpha-2): captured at save time.
-- Nullable for legacy rows or unknown origin.

SET @has_post_country_col := (
    SELECT COUNT(*)
    FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'post'
      AND column_name = 'country_code'
);
SET @add_post_country_col_sql := IF(
    @has_post_country_col = 0,
    'ALTER TABLE post ADD COLUMN country_code CHAR(2) NULL COMMENT ''Post origin country (ISO 3166-1 alpha-2); NULL when unavailable'' AFTER domain_id',
    'SELECT 1'
);
PREPARE add_post_country_col_stmt FROM @add_post_country_col_sql;
EXECUTE add_post_country_col_stmt;
DEALLOCATE PREPARE add_post_country_col_stmt;

SET @has_post_country_idx := (
    SELECT COUNT(*)
    FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'post'
      AND index_name = 'idx_post_country_code'
);
SET @add_post_country_idx_sql := IF(
    @has_post_country_idx = 0,
    'ALTER TABLE post ADD KEY idx_post_country_code (country_code)',
    'SELECT 1'
);
PREPARE add_post_country_idx_stmt FROM @add_post_country_idx_sql;
EXECUTE add_post_country_idx_stmt;
DEALLOCATE PREPARE add_post_country_idx_stmt;
