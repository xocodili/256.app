-- Profit, non-profit, and gathering entities default to neutral 2.50 (same as humans/communities).

UPDATE social_entity se
INNER JOIN ref_key_values rk_type
    ON rk_type.id = se.type_id
   AND rk_type.`group` = 'social_entity_type'
   AND rk_type.`key` IN ('profit', 'non_profit', 'gathering')
   AND rk_type.is_active = 1
LEFT JOIN ref_key_values rk_dead
    ON rk_dead.id = se.entity_status_id
   AND rk_dead.`group` = 'social_entity_status'
   AND rk_dead.`key` = 'dead'
   AND rk_dead.is_active = 1
SET se.rating = 2.50
WHERE se.rating = 0.00
  AND rk_dead.id IS NULL;
