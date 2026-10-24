-- User-created communities and other brand entities were inserted at pending_verification;
-- public discovery (`/public/{handle}`) requires verified. Promote alive brand rows.

SET @verified_status := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'status' AND `key` = 'verified' AND is_active = 1
    LIMIT 1
);
SET @verified_entity_status := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_status' AND `key` = 'verified' AND is_active = 1
    LIMIT 1
);

UPDATE social_entity se
INNER JOIN ref_key_values rk_type
    ON rk_type.id = se.type_id
   AND rk_type.`group` = 'social_entity_type'
   AND rk_type.`key` IN ('community', 'non_profit', 'profit', 'gathering')
   AND rk_type.is_active = 1
INNER JOIN ref_key_values rk_cur
    ON rk_cur.id = COALESCE(se.entity_status_id, se.status_id)
   AND rk_cur.`key` = 'pending_verification'
LEFT JOIN ref_key_values rk_dead
    ON rk_dead.id = COALESCE(se.entity_status_id, se.status_id)
   AND rk_dead.`group` IN ('social_entity_status', 'status')
   AND rk_dead.`key` = 'dead'
   AND rk_dead.is_active = 1
SET se.status_id = @verified_status,
    se.entity_status_id = @verified_entity_status
WHERE rk_dead.id IS NULL;
