-- Canonical ref key for "Reviewed" join policy (was request_to_join).
UPDATE ref_key_values
SET `key` = 'reviewed'
WHERE `group` = 'social_join_policy' AND `key` = 'request_to_join';

-- Root charter community takes no new members via join policy.
UPDATE social_community sc
INNER JOIN social_entity se ON se.id = sc.social_entity_id
SET sc.join_policy_id = (
    SELECT id
    FROM ref_key_values
    WHERE `group` = 'social_join_policy' AND `key` = 'closed' AND is_active = 1
    LIMIT 1
)
WHERE se.handle = 'operating_charter_256';
