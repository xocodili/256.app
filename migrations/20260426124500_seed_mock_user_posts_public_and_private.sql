-- Seed deterministic mock posts for each seeded user:
-- 1) one public community-oriented post with a trusted-domain link
-- 2) one private friends/family post
--
-- Idempotent: guarded by deterministic `body_plain` patterns per user.

SET @status_verified := (
    SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1
);
SET @rel_member_of := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'member_of'
    LIMIT 1
);
SET @scope_public := 'public';
SET @scope_private := 'private';
SET @scope_friends := 'friends';
SET @scope_family := 'family';

CREATE TEMPORARY TABLE tmp_seed_users (
    user_id BIGINT UNSIGNED NOT NULL,
    human_entity_id BIGINT UNSIGNED NOT NULL,
    community_entity_id BIGINT UNSIGNED NULL,
    community_handle VARCHAR(64) NULL,
    country_code CHAR(2) NULL,
    PRIMARY KEY (user_id),
    UNIQUE KEY uq_tmp_seed_users_human_entity (human_entity_id)
) ENGINE=InnoDB;

INSERT INTO tmp_seed_users (
    user_id,
    human_entity_id,
    community_entity_id,
    community_handle,
    country_code
)
SELECT
    u.id AS user_id,
    se_h.id AS human_entity_id,
    se_c.id AS community_entity_id,
    se_c.handle AS community_handle,
    loc.country_code AS country_code
FROM users u
INNER JOIN social_entity se_h
    ON se_h.created_by = u.id
LEFT JOIN locations loc ON loc.id = se_h.location_id
LEFT JOIN social_entity_relationship ser_m
    ON ser_m.from_social_entity_id = se_h.id
   AND ser_m.type_id = @rel_member_of
   AND ser_m.ended_at IS NULL
LEFT JOIN social_entity se_c ON se_c.id = ser_m.to_social_entity_id
WHERE u.email LIKE 'seed.%@256.app';

CREATE TEMPORARY TABLE tmp_seed_domains (
    rn INT UNSIGNED NOT NULL,
    domain_id BIGINT UNSIGNED NOT NULL,
    domain VARCHAR(255) NOT NULL,
    PRIMARY KEY (rn),
    UNIQUE KEY uq_tmp_seed_domains_domain (domain)
) ENGINE=InnoDB;

SET @seed_domain_rn := 0;
INSERT INTO tmp_seed_domains (rn, domain_id, domain)
SELECT
    (@seed_domain_rn := @seed_domain_rn + 1) AS rn,
    cd.id,
    cd.domain
FROM content_domain cd
INNER JOIN ref_key_values rk_status
    ON rk_status.id = cd.status_id
   AND rk_status.`group` = 'status'
WHERE rk_status.`key` = 'verified'
ORDER BY cd.domain ASC;

SET @seed_domain_count := (SELECT COUNT(*) FROM tmp_seed_domains);

CREATE TEMPORARY TABLE tmp_seed_public_plan (
    user_id BIGINT UNSIGNED NOT NULL,
    human_entity_id BIGINT UNSIGNED NOT NULL,
    community_entity_id BIGINT UNSIGNED NULL,
    community_handle VARCHAR(64) NULL,
    domain_id BIGINT UNSIGNED NULL,
    domain VARCHAR(255) NULL,
    url_canonical VARCHAR(2048) NOT NULL,
    url_original TEXT NOT NULL,
    url_canonical_sha BINARY(32) NOT NULL,
    body_plain VARCHAR(4000) NOT NULL,
    body_markup VARCHAR(4000) NOT NULL,
    body_html_sanitized VARCHAR(4000) NOT NULL,
    country_code CHAR(2) NULL,
    PRIMARY KEY (user_id),
    UNIQUE KEY uq_tmp_seed_public_sha (url_canonical_sha)
) ENGINE=InnoDB;

INSERT INTO tmp_seed_public_plan (
    user_id,
    human_entity_id,
    community_entity_id,
    community_handle,
    domain_id,
    domain,
    url_canonical,
    url_original,
    url_canonical_sha,
    body_plain,
    body_markup,
    body_html_sanitized,
    country_code
)
SELECT
    su.user_id,
    su.human_entity_id,
    su.community_entity_id,
    su.community_handle,
    sd.domain_id,
    sd.domain,
    CONCAT(
        'https://',
        sd.domain,
        '/community/',
        COALESCE(su.community_handle, 'general'),
        '/seed-',
        su.user_id
    ) AS url_canonical,
    CONCAT(
        'https://',
        sd.domain,
        '/community/',
        COALESCE(su.community_handle, 'general'),
        '/seed-',
        su.user_id
    ) AS url_original,
    UNHEX(
        SHA2(
            CONCAT(
                'https://',
                sd.domain,
                '/community/',
                COALESCE(su.community_handle, 'general'),
                '/seed-',
                su.user_id
            ),
            256
        )
    ) AS url_canonical_sha,
    CONCAT(
        'Seed public post ',
        su.user_id,
        ': Lorem ipsum update for ',
        COALESCE(su.community_handle, 'the community'),
        ' featuring practical resources at https://',
        sd.domain,
        '.'
    ) AS body_plain,
    CONCAT(
        '<p>Seed public post ',
        su.user_id,
        ': Lorem ipsum update for ',
        COALESCE(su.community_handle, 'the community'),
        ' featuring practical resources at <a href="https://',
        sd.domain,
        '/community/',
        COALESCE(su.community_handle, 'general'),
        '/seed-',
        su.user_id,
        '">https://',
        sd.domain,
        '</a>.</p>'
    ) AS body_markup,
    CONCAT(
        '<p>Seed public post ',
        su.user_id,
        ': Lorem ipsum update for ',
        COALESCE(su.community_handle, 'the community'),
        ' featuring practical resources at <a href="https://',
        sd.domain,
        '/community/',
        COALESCE(su.community_handle, 'general'),
        '/seed-',
        su.user_id,
        '" rel="noopener noreferrer" target="_blank">https://',
        sd.domain,
        '</a>.</p>'
    ) AS body_html_sanitized,
    su.country_code
FROM tmp_seed_users su
LEFT JOIN tmp_seed_domains sd
    ON sd.rn = ((su.user_id - 1) % NULLIF(@seed_domain_count, 0)) + 1;

INSERT INTO content_link (
    domain_id,
    url_original,
    url_canonical,
    url_canonical_sha,
    status_id,
    rating,
    fetch_status,
    fetch_attempts,
    fetched_at,
    last_success_at,
    created_by,
    modified_by
)
SELECT
    p.domain_id,
    p.url_original,
    p.url_canonical,
    p.url_canonical_sha,
    @status_verified,
    0.00,
    'ok',
    1,
    UTC_TIMESTAMP(6),
    UTC_TIMESTAMP(6),
    p.user_id,
    p.user_id
FROM tmp_seed_public_plan p
LEFT JOIN content_link cl ON cl.url_canonical_sha = p.url_canonical_sha
WHERE p.domain_id IS NOT NULL
  AND cl.id IS NULL;

INSERT INTO post (
    entity_id,
    content_link_id,
    parent_post_id,
    root_post_id,
    status_id,
    rating,
    body_plain,
    body_markup,
    body_html_sanitized,
    published_at,
    created_by,
    modified_by,
    domain_id,
    country_code
)
SELECT
    p.human_entity_id,
    cl.id,
    NULL,
    NULL,
    @status_verified,
    0.00,
    p.body_plain,
    p.body_markup,
    p.body_html_sanitized,
    UTC_TIMESTAMP(6),
    p.user_id,
    p.user_id,
    p.domain_id,
    p.country_code
FROM tmp_seed_public_plan p
INNER JOIN content_link cl ON cl.url_canonical_sha = p.url_canonical_sha
LEFT JOIN post existing
    ON existing.created_by = p.user_id
   AND existing.body_plain = p.body_plain
WHERE existing.id IS NULL;

INSERT IGNORE INTO post_audience_scope (post_id, scope_key, created_by)
SELECT
    p.id,
    @scope_public,
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed public post %';

INSERT IGNORE INTO post_audience_entity (post_id, audience_entity_id, created_by)
SELECT
    p.id,
    p.entity_id,
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed public post %';

INSERT IGNORE INTO post_audience_entity (post_id, audience_entity_id, created_by)
SELECT
    p.id,
    plan.community_entity_id,
    p.created_by
FROM post p
INNER JOIN tmp_seed_public_plan plan
    ON plan.user_id = p.created_by
   AND plan.body_plain = p.body_plain
WHERE p.body_plain LIKE 'Seed public post %'
  AND plan.community_entity_id IS NOT NULL;

INSERT INTO post (
    entity_id,
    content_link_id,
    parent_post_id,
    root_post_id,
    status_id,
    rating,
    body_plain,
    body_markup,
    body_html_sanitized,
    published_at,
    created_by,
    modified_by,
    domain_id,
    country_code
)
SELECT
    su.human_entity_id,
    NULL,
    NULL,
    NULL,
    @status_verified,
    0.00,
    CONCAT(
        'Seed private post ',
        su.user_id,
        ': Lorem ipsum note for friends and family only.'
    ) AS body_plain,
    CONCAT(
        '<p>Seed private post ',
        su.user_id,
        ': Lorem ipsum note for friends and family only.</p>'
    ) AS body_markup,
    CONCAT(
        '<p>Seed private post ',
        su.user_id,
        ': Lorem ipsum note for friends and family only.</p>'
    ) AS body_html_sanitized,
    UTC_TIMESTAMP(6),
    su.user_id,
    su.user_id,
    NULL,
    su.country_code
FROM tmp_seed_users su
LEFT JOIN post existing
    ON existing.created_by = su.user_id
   AND existing.body_plain = CONCAT(
        'Seed private post ',
        su.user_id,
        ': Lorem ipsum note for friends and family only.'
    )
WHERE existing.id IS NULL;

INSERT IGNORE INTO post_audience_scope (post_id, scope_key, created_by)
SELECT
    p.id,
    @scope_private,
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed private post %';

INSERT IGNORE INTO post_audience_scope (post_id, scope_key, created_by)
SELECT
    p.id,
    @scope_friends,
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed private post %';

INSERT IGNORE INTO post_audience_scope (post_id, scope_key, created_by)
SELECT
    p.id,
    @scope_family,
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed private post %';

INSERT IGNORE INTO post_audience_entity (post_id, audience_entity_id, created_by)
SELECT
    p.id,
    p.entity_id,
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed private post %';

DROP TEMPORARY TABLE IF EXISTS tmp_seed_public_plan;
DROP TEMPORARY TABLE IF EXISTS tmp_seed_domains;
DROP TEMPORARY TABLE IF EXISTS tmp_seed_users;
