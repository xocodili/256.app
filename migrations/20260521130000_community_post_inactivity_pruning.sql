-- Community post-inactivity penalties and one-time failing-rating email markers.

ALTER TABLE community_graph_event
    MODIFY event_type ENUM(
        'death',
        'reparent',
        'reparent_judicial',
        'inactivity_penalty'
    ) NOT NULL;

ALTER TABLE social_community
    ADD COLUMN last_post_activity_at TIMESTAMP(6) NULL
        COMMENT 'Last post authored by or addressed to this community' AFTER died_at,
    ADD COLUMN inactivity_penalties_applied INT UNSIGNED NOT NULL DEFAULT 0
        COMMENT 'Count of inactivity rating penalties applied' AFTER last_post_activity_at;

ALTER TABLE social_entity
    ADD COLUMN rating_failing_notified_at TIMESTAMP(6) NULL
        COMMENT 'When owners/admins were notified that global rating dropped below 1' AFTER rating;

ALTER TABLE social_entity_relationship
    ADD COLUMN rating_failing_notified_at TIMESTAMP(6) NULL
        COMMENT 'When the member was notified this edge rating dropped below 1' AFTER rating;

-- Seed activity timestamps from existing posts (author or audience).
UPDATE social_community sc
INNER JOIN social_entity se ON se.id = sc.social_entity_id
SET sc.last_post_activity_at = (
    SELECT MAX(ts) FROM (
        SELECT p.created_at AS ts
        FROM post p
        WHERE p.entity_id = sc.social_entity_id
        UNION ALL
        SELECT p2.created_at AS ts
        FROM post p2
        INNER JOIN post_audience_entity pae ON pae.post_id = p2.id
        WHERE pae.audience_entity_id = sc.social_entity_id
    ) activity
)
WHERE sc.last_post_activity_at IS NULL;

-- Do not email everyone already failing before this feature ships.
UPDATE social_entity
SET rating_failing_notified_at = UTC_TIMESTAMP(6)
WHERE rating < 1.00
  AND rating_failing_notified_at IS NULL;

UPDATE social_entity_relationship ser
INNER JOIN ref_key_values rk_type
    ON rk_type.id = ser.type_id
   AND rk_type.`group` = 'social_entity_relationship_type'
   AND rk_type.`key` = 'member_of'
SET ser.rating_failing_notified_at = UTC_TIMESTAMP(6)
WHERE ser.rating < 1.00
  AND ser.ended_at IS NULL
  AND ser.rating_failing_notified_at IS NULL;
