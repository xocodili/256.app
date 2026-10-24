-- Seed public posts: ensure @public scope + poster entity audience (idempotent).
-- Also insert a linkless "Seed public post %" row for seed users who never got one because
-- `20260426124500_seed_mock_user_posts_public_and_private.sql` requires a verified `content_domain`
-- to create `content_link` and therefore skipped the public post entirely.

SET @status_verified := (
    SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1
);

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
    se_h.id,
    NULL,
    NULL,
    NULL,
    @status_verified,
    0.00,
    CONCAT(
        'Seed public post ',
        u.id,
        ': Lorem ipsum public note (no link; ensures @public audience when no content_domain was seeded).'
    ),
    CONCAT(
        '<p>Seed public post ',
        u.id,
        ': Lorem ipsum public note (no link; ensures @public audience when no content_domain was seeded).</p>'
    ),
    CONCAT(
        '<p>Seed public post ',
        u.id,
        ': Lorem ipsum public note (no link; ensures @public audience when no content_domain was seeded).</p>'
    ),
    UTC_TIMESTAMP(6),
    u.id,
    u.id,
    NULL,
    (
        SELECT loc.country_code
        FROM locations loc
        WHERE loc.id = se_h.location_id
        LIMIT 1
    )
FROM users u
INNER JOIN social_entity se_h ON se_h.created_by = u.id
INNER JOIN social_human sh ON sh.social_entity_id = se_h.id
WHERE u.email LIKE 'seed.%@256.app'
  AND NOT EXISTS (
        SELECT 1
        FROM post p2
        WHERE p2.created_by = u.id
          AND p2.body_plain LIKE 'Seed public post %'
    );

INSERT IGNORE INTO post_audience_scope (post_id, scope_key, created_by)
SELECT
    p.id,
    'public',
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed public post %'
  AND NOT EXISTS (
        SELECT 1
        FROM post_audience_scope pas
        WHERE pas.post_id = p.id
          AND pas.scope_key = 'public'
    );

INSERT IGNORE INTO post_audience_entity (post_id, audience_entity_id, created_by)
SELECT
    p.id,
    p.entity_id,
    p.created_by
FROM post p
WHERE p.body_plain LIKE 'Seed public post %'
  AND NOT EXISTS (
        SELECT 1
        FROM post_audience_entity pae
        WHERE pae.post_id = p.id
          AND pae.audience_entity_id = p.entity_id
    );
