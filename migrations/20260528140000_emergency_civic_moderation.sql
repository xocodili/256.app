-- Greenfield emergency safety vs civic dispute moderation.
-- §4 preamble (point 4) + selectable safety sub-clauses (401–407).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('status', 'suppressed', 45, 1, NULL, NULL),
    ('judicial_case_trigger', 'emergency_safety', 12, 1, NULL, NULL),
    ('judicial_case_trigger', 'civic_dispute', 14, 1, NULL, NULL),
    ('judicial_case_status', 'contained', 15, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

UPDATE ref_key_values
SET is_active = 0
WHERE `group` = 'judicial_case_trigger'
  AND `key` = 'post_moderation';

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

SET @tier_serious_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'judicial_moderation_tier' AND `key` = 'serious' AND is_active = 1
    LIMIT 1
);

SET @tier_regular_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'judicial_moderation_tier' AND `key` = 'regular' AND is_active = 1
    LIMIT 1
);

-- Point 4: preamble only (not selectable for filing).
UPDATE community_constitution_item
SET item = 'Universal Safety Floor — The Network prohibits the conduct listed in §4.1–§4.7 below. These prohibitions apply network-wide regardless of locale. Lawful protected identity and belief (item 7) are not violations here solely because they are illegal or stigmatized elsewhere. Select a specific §4.x clause when filing an emergency safety report.',
    judicial_trigger_chk = 0,
    moderation_tier_id = NULL
WHERE community_id = @root_community_id AND point = 4 AND status = 'active';

INSERT INTO community_constitution_item (
    community_id, point, item, status, judicial_trigger_chk, moderation_tier_id, created_by, modified_by
)
SELECT @root_community_id, x.point, x.item, 'active', 1, @tier_serious_id, @system_uid, @system_uid
FROM (
    SELECT 401 AS point, '§4.1 — Child sexual abuse material and exploitation of minors.' AS item
    UNION ALL SELECT 402, '§4.2 — Credible threats of physical violence.'
    UNION ALL SELECT 403, '§4.3 — Non-consensual intimate imagery.'
    UNION ALL SELECT 404, '§4.4 — Doxxing and non-consensual publication of personally identifying information.'
    UNION ALL SELECT 405, '§4.5 — Fraud, scams, and impersonation intended to harm.'
    UNION ALL SELECT 406, '§4.6 — Coordinated harassment and bullying as conduct.'
    UNION ALL SELECT 407, '§4.7 — Distribution of malware or phishing intended to harm.'
) AS x
WHERE @root_community_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM community_constitution_item cci
      WHERE cci.community_id = @root_community_id AND cci.point = x.point
  );

ALTER TABLE judicial_case
    ADD COLUMN contested_at TIMESTAMP(6) NULL COMMENT 'Author opened contest (UTC)' AFTER closed_at,
    ADD COLUMN contested_by BIGINT UNSIGNED NULL COMMENT 'FK users.id — contesting author' AFTER contested_at,
    ADD COLUMN standing_applied_at TIMESTAMP(6) NULL COMMENT 'Loathed-equivalent standing applied (uncontested T+3)' AFTER contested_by,
    ADD KEY idx_judicial_case_contested (contested_at),
    ADD CONSTRAINT fk_judicial_case_contested_by FOREIGN KEY (contested_by) REFERENCES users (id) ON DELETE SET NULL;

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES ('ops_job_type', 'emergency_standing_settlement', 46, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);
