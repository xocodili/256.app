-- Remove legacy raw logo URL storage once `logo_content_link_id` backfill is in place.
-- Guarded so fresh installs (without the legacy column) do not fail.

DROP PROCEDURE IF EXISTS drop_social_profit_external_logo_url;

CREATE PROCEDURE drop_social_profit_external_logo_url()
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns c
        WHERE c.table_schema = DATABASE()
          AND c.table_name = 'social_profit'
          AND c.column_name = 'external_logo_url'
    ) THEN
        ALTER TABLE social_profit
            DROP COLUMN external_logo_url;
    END IF;
END;

CALL drop_social_profit_external_logo_url();
DROP PROCEDURE IF EXISTS drop_social_profit_external_logo_url;
