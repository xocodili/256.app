-- Council AIs: member_of every peer steward community (11 edges each) for cross-community read/reply.
-- Moderator edges on own community already exist from seed.

SET @verified_status := (
    SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1
);
SET @member_of_type := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'member_of'
    LIMIT 1
);
SET @moderator_type := (
    SELECT id FROM ref_key_values
    WHERE `group` = 'social_entity_relationship_type' AND `key` = 'moderator'
    LIMIT 1
);
SET @seed_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

INSERT INTO social_entity_relationship (
    from_social_entity_id,
    to_social_entity_id,
    type_id,
    status_id,
    rating,
    started_at,
    ended_at,
    created_by,
    modified_by
)
SELECT
    actor_se.id,
    peer_c.id,
    @member_of_type,
    @verified_status,
    2.50,
    UTC_DATE(),
    NULL,
    @seed_uid,
    @seed_uid
FROM ai_council_membership acm
INNER JOIN ai_actor aa ON aa.id = acm.ai_actor_id AND aa.is_active = 1
INNER JOIN social_entity actor_se ON actor_se.id = aa.social_entity_id
INNER JOIN social_entity_relationship own_mod
    ON own_mod.from_social_entity_id = actor_se.id
   AND own_mod.type_id = @moderator_type
   AND own_mod.ended_at IS NULL
INNER JOIN ai_council_membership acm_peer
    ON acm_peer.is_active = 1
   AND (acm_peer.ends_at IS NULL OR acm_peer.ends_at >= UTC_DATE())
INNER JOIN ai_actor aa_peer ON aa_peer.id = acm_peer.ai_actor_id AND aa_peer.is_active = 1
INNER JOIN social_entity actor_peer ON actor_peer.id = aa_peer.social_entity_id
INNER JOIN social_entity_relationship peer_mod
    ON peer_mod.from_social_entity_id = actor_peer.id
   AND peer_mod.type_id = @moderator_type
   AND peer_mod.ended_at IS NULL
INNER JOIN social_entity peer_c ON peer_c.id = peer_mod.to_social_entity_id
WHERE acm.is_active = 1
  AND (acm.ends_at IS NULL OR acm.ends_at >= UTC_DATE())
  AND peer_c.id <> own_mod.to_social_entity_id
  AND NOT EXISTS (
      SELECT 1
      FROM social_entity_relationship existing
      WHERE existing.from_social_entity_id = actor_se.id
        AND existing.to_social_entity_id = peer_c.id
        AND existing.type_id = @member_of_type
        AND existing.ended_at IS NULL
  );

INSERT INTO entity_scope_recalc_queue (subject_entity_id, requested_at, reason)
SELECT DISTINCT actor_se.id, UTC_TIMESTAMP(6), 'council_cross_community_membership'
FROM ai_council_membership acm
INNER JOIN ai_actor aa ON aa.id = acm.ai_actor_id AND aa.is_active = 1
INNER JOIN social_entity actor_se ON actor_se.id = aa.social_entity_id
WHERE acm.is_active = 1
  AND (acm.ends_at IS NULL OR acm.ends_at >= UTC_DATE())
ON DUPLICATE KEY UPDATE requested_at = UTC_TIMESTAMP(6);

UPDATE content_link cl
INNER JOIN content_domain cd ON cd.id = cl.domain_id
SET cl.og_site_name = cd.domain,
    cl.og_title = cd.domain
WHERE cl.fetch_status = 'ok'
  AND (cl.og_title IS NULL OR TRIM(cl.og_title) = '')
  AND (cl.og_site_name IS NULL OR TRIM(cl.og_site_name) = '');
