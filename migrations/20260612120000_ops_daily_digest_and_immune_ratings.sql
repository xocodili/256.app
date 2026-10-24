-- Once-per-period ops digest send log (panoply daily status, etc.).
CREATE TABLE IF NOT EXISTS ops_daily_digest_sent (
    digest_key VARCHAR(64) NOT NULL COMMENT 'e.g. panoply_daily_status',
    period_key VARCHAR(32) NOT NULL COMMENT 'UTC date key YYYY-MM-DD',
    sent_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (digest_key, period_key)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Death-immune product communities and the structural operator were seeded without explicit
-- rating floors; restore neutral standing so failing notices and decay do not misfire.
UPDATE social_entity se
SET se.rating = 2.50
WHERE se.handle IN ('panoply', 'sphinx', '256app', 'operating_charter_256')
  AND se.rating < 2.50;

UPDATE social_entity_relationship ser
INNER JOIN social_entity human_se ON human_se.id = ser.from_social_entity_id
INNER JOIN social_entity comm_se ON comm_se.id = ser.to_social_entity_id
INNER JOIN ref_key_values rk_type
    ON rk_type.id = ser.type_id
   AND rk_type.`group` = 'social_entity_relationship_type'
   AND rk_type.`key` = 'member_of'
SET ser.rating = 2.50
WHERE human_se.handle = 'sphinx'
  AND comm_se.handle = 'panoply'
  AND ser.ended_at IS NULL
  AND ser.rating < 2.50;
