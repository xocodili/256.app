-- Seed permanently blocked domain: 4chan (registrable domain form).
SET @blocked_status := (
    SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'blocked' LIMIT 1
);
SET @seed_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

INSERT INTO content_domain (
    domain,
    status_id,
    rating,
    first_seen_at,
    last_seen_at,
    safety_checked_at,
    safety_state,
    safety_note,
    created_by,
    modified_by
)
SELECT
    '4chan.org',
    @blocked_status,
    0.00,
    UTC_TIMESTAMP(6),
    UTC_TIMESTAMP(6),
    UTC_TIMESTAMP(6),
    'unsafe',
    'Seeded as permanently blocked domain.',
    @seed_uid,
    @seed_uid
FROM DUAL
WHERE NOT EXISTS (
    SELECT 1 FROM content_domain WHERE domain = '4chan.org'
);

UPDATE content_domain
SET
    status_id = @blocked_status,
    safety_state = 'unsafe',
    safety_checked_at = UTC_TIMESTAMP(6),
    safety_note = 'Seeded as permanently blocked domain.',
    modified_by = COALESCE(@seed_uid, modified_by)
WHERE domain = '4chan.org';
