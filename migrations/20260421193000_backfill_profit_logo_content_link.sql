-- Backfill legacy `social_profit.external_logo_url` into canonical link tables and FK.
-- Safe to run multiple times: inserts are deduped, updates only fill NULL FK values.
-- Also safe on clean installs where the legacy column does not exist.

DROP PROCEDURE IF EXISTS backfill_profit_logo_content_link;

CREATE PROCEDURE backfill_profit_logo_content_link()
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns c
        WHERE c.table_schema = DATABASE()
          AND c.table_name = 'social_profit'
          AND c.column_name = 'external_logo_url'
    ) THEN
        CREATE TEMPORARY TABLE tmp_profit_logo_backfill (
            social_entity_id BIGINT UNSIGNED NOT NULL,
            url_canonical VARCHAR(2048) NOT NULL,
            registrable_domain VARCHAR(255) NOT NULL,
            PRIMARY KEY (social_entity_id),
            KEY idx_tmp_profit_logo_backfill_url (url_canonical(512)),
            KEY idx_tmp_profit_logo_backfill_domain (registrable_domain)
        ) ENGINE=Memory;

        INSERT INTO tmp_profit_logo_backfill (social_entity_id, url_canonical, registrable_domain)
        SELECT
            sp.social_entity_id,
            TRIM(sp.external_logo_url) AS url_canonical,
            LOWER(
                SUBSTRING_INDEX(
                    SUBSTRING_INDEX(
                        SUBSTRING_INDEX(TRIM(sp.external_logo_url), '/', 3),
                        '/',
                        -1
                    ),
                    ':',
                    1
                )
            ) AS registrable_domain
        FROM social_profit sp
        WHERE sp.logo_content_link_id IS NULL
          AND sp.external_logo_url IS NOT NULL
          AND TRIM(sp.external_logo_url) <> ''
          AND TRIM(sp.external_logo_url) REGEXP '^https://[^[:space:]]+$';

        INSERT INTO content_domain (
            domain, status_id, rating, first_seen_at, last_seen_at, created_by, modified_by
        )
        SELECT
            t.registrable_domain,
            rk_pending.id,
            0.00,
            UTC_TIMESTAMP(6),
            UTC_TIMESTAMP(6),
            NULL,
            NULL
        FROM (
            SELECT DISTINCT registrable_domain
            FROM tmp_profit_logo_backfill
        ) t
        INNER JOIN ref_key_values rk_pending
            ON rk_pending.`group` = 'status'
           AND rk_pending.`key` = 'pending_verification'
           AND rk_pending.is_active = 1
        LEFT JOIN content_domain cd
            ON cd.domain = t.registrable_domain
        WHERE cd.id IS NULL;

        INSERT INTO content_link (
            domain_id, url_original, url_canonical, url_canonical_sha,
            status_id, rating, fetch_status, fetch_attempts, next_recrawl_at,
            created_by, modified_by
        )
        SELECT
            cd.id AS domain_id,
            t.url_canonical AS url_original,
            t.url_canonical AS url_canonical,
            UNHEX(SHA2(t.url_canonical, 256)) AS url_canonical_sha,
            rk_pending.id AS status_id,
            0.00,
            'pending',
            0,
            UTC_TIMESTAMP(6) + INTERVAL 6 HOUR,
            NULL,
            NULL
        FROM (
            SELECT DISTINCT url_canonical, registrable_domain
            FROM tmp_profit_logo_backfill
        ) t
        INNER JOIN content_domain cd
            ON cd.domain = t.registrable_domain
        INNER JOIN ref_key_values rk_pending
            ON rk_pending.`group` = 'status'
           AND rk_pending.`key` = 'pending_verification'
           AND rk_pending.is_active = 1
        LEFT JOIN content_link cl
            ON cl.url_canonical_sha = UNHEX(SHA2(t.url_canonical, 256))
        WHERE cl.id IS NULL;

        UPDATE social_profit sp
        INNER JOIN tmp_profit_logo_backfill t
            ON t.social_entity_id = sp.social_entity_id
        INNER JOIN content_link cl
            ON cl.url_canonical_sha = UNHEX(SHA2(t.url_canonical, 256))
        SET sp.logo_content_link_id = cl.id
        WHERE sp.logo_content_link_id IS NULL;

        DROP TEMPORARY TABLE IF EXISTS tmp_profit_logo_backfill;
    END IF;
END;

CALL backfill_profit_logo_content_link();
DROP PROCEDURE IF EXISTS backfill_profit_logo_content_link;
