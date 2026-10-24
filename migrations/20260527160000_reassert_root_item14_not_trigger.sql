-- Idempotent: point 14 is charter guidance only (not a post-dispute trigger).

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

UPDATE community_constitution_item
SET judicial_trigger_chk = 0,
    moderation_tier_id = NULL,
    modified_by = @system_uid
WHERE community_id = @root_community_id
  AND point = 14
  AND status = 'active'
  AND judicial_trigger_chk <> 0;
