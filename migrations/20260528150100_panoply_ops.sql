-- Panoply: invite-only platform safety operator community + audit log.
-- Owner: operator@example.com (seed user + human @sphinx when absent).

CREATE TABLE IF NOT EXISTS safety_operator_action (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    actor_user_id BIGINT UNSIGNED NOT NULL COMMENT 'FK users.id — Panoply operator',
    action_type VARCHAR(64) NOT NULL COMMENT 'emergency_file, civic_file, vacate_emergency, eject, session_kill, domain_block',
    target_post_id BIGINT UNSIGNED NULL COMMENT 'FK post.id when post-scoped',
    target_user_id BIGINT UNSIGNED NULL COMMENT 'FK users.id when account-scoped',
    target_domain VARCHAR(253) NULL COMMENT 'registrable domain when domain_block',
    judicial_case_id BIGINT UNSIGNED NULL COMMENT 'FK judicial_case.id when case-linked',
    reason_code VARCHAR(64) NULL,
    note TEXT NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    KEY idx_safety_operator_action_actor (actor_user_id, created_at),
    KEY idx_safety_operator_action_post (target_post_id),
    KEY idx_safety_operator_action_case (judicial_case_id),
    CONSTRAINT fk_safety_operator_action_actor FOREIGN KEY (actor_user_id) REFERENCES users (id) ON DELETE RESTRICT,
    CONSTRAINT fk_safety_operator_action_post FOREIGN KEY (target_post_id) REFERENCES post (id) ON DELETE SET NULL,
    CONSTRAINT fk_safety_operator_action_user FOREIGN KEY (target_user_id) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_safety_operator_action_case FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @global_lid := (
    SELECT l.id FROM locations l
    WHERE l.canonical_fingerprint = UNHEX('d668ba83cc35a3d46e3cf2544d70b4d5194ae479cd434d5556fa26635b0b50cf')
    LIMIT 1
);
SET @verified_status := (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1);
SET @verified_user_status := (SELECT id FROM ref_key_values WHERE `group` = 'user_status' AND `key` = 'verified' LIMIT 1);
SET @human_type := (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'human' LIMIT 1);
SET @community_type := (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'community' LIMIT 1);
SET @invite_only := (SELECT id FROM ref_key_values WHERE `group` = 'social_join_policy' AND `key` = 'invite_only' LIMIT 1);
SET @member_of_type := (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_relationship_type' AND `key` = 'member_of' LIMIT 1);
SET @owner_role := (SELECT id FROM ref_key_values WHERE `group` = 'user_social_entity_role' AND `key` = 'owner' LIMIT 1);
SET @human_user_type := (SELECT id FROM ref_key_values WHERE `group` = 'user_type' AND `key` = 'human' LIMIT 1);
SET @basic_tier := (SELECT id FROM ref_key_values WHERE `group` = 'verification_tier' AND `key` = 'basic' LIMIT 1);
SET @entity_verified := (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_status' AND `key` = 'verified' LIMIT 1);
SET @godmother_type := (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_relationship_type' AND `key` = 'godmother' LIMIT 1);
SET @moderator_type := (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_relationship_type' AND `key` = 'moderator' LIMIT 1);

-- operator@example.com user (idempotent; verified — skips onboarding)
INSERT INTO users (
    email, password_hash, email_verified_at, status_id, user_status_id, user_type_id, verification_tier_id, terms_consent_version, created_by, modified_by
)
SELECT
    'operator@example.com', NULL, UTC_TIMESTAMP(6), @verified_status, @verified_user_status, @human_user_type, @basic_tier, 'v2-2026-05-29', @system_uid, @system_uid
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM users WHERE email = 'operator@example.com');

SET @sphinx_uid := (SELECT id FROM users WHERE email = 'operator@example.com' LIMIT 1);

UPDATE users
SET terms_consent_version = 'v2-2026-05-29',
    status_id = @verified_status,
    user_status_id = @verified_user_status,
    email_verified_at = COALESCE(email_verified_at, UTC_TIMESTAMP(6)),
    modified_by = @system_uid
WHERE id = @sphinx_uid;

-- @sphinx human entity (idempotent; owned by operator account)
INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, modified_by)
SELECT 'sphinx', @human_type, @verified_status, @entity_verified, @global_lid, @sphinx_uid, @sphinx_uid
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM social_entity WHERE handle = 'sphinx');

SET @sphinx_human_eid := (SELECT id FROM social_entity WHERE handle = 'sphinx' LIMIT 1);

UPDATE social_entity
SET created_by = @sphinx_uid,
    modified_by = @system_uid
WHERE id = @sphinx_human_eid
  AND @sphinx_uid IS NOT NULL
  AND created_by <> @sphinx_uid;

INSERT INTO social_human (social_entity_id, given_name, family_name, birth_date, join_policy_id)
SELECT @sphinx_human_eid, 'Founder', NULL, DATE('1980-01-01'), @invite_only
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM social_human WHERE social_entity_id = @sphinx_human_eid);

INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by)
SELECT @sphinx_uid, @sphinx_human_eid, @owner_role, 1, UTC_DATE(), @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_uid IS NOT NULL AND @sphinx_human_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM user_social_entity_role
      WHERE user_id = @sphinx_uid AND social_entity_id = @sphinx_human_eid AND role_id = @owner_role
  );

-- @panoply community (idempotent; @sphinx is founder)
INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, modified_by)
SELECT 'panoply', @community_type, @verified_status, @entity_verified, @global_lid, @sphinx_uid, @sphinx_uid
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM social_entity WHERE handle = 'panoply');

SET @panoply_eid := (SELECT id FROM social_entity WHERE handle = 'panoply' LIMIT 1);

INSERT INTO social_community (social_entity_id, label, description, intention, rules, founded_at, join_policy_id)
SELECT
    @panoply_eid,
    'Panoply',
    'Platform safety operators — habitat security for 256.app.',
    'Contain acute harm, enforce platform safety, and preserve audit trails without bypassing community membership in ordinary use.',
    'Invite-only. Clearance grants ops dashboard access and platform fitness death immunity only.',
    UTC_DATE(),
    @invite_only
FROM DUAL
WHERE @panoply_eid IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM social_community WHERE social_entity_id = @panoply_eid);

INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by)
SELECT @sphinx_uid, @panoply_eid, @owner_role, 0, UTC_DATE(), @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_uid IS NOT NULL AND @panoply_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM user_social_entity_role
      WHERE user_id = @sphinx_uid AND social_entity_id = @panoply_eid AND role_id = @owner_role
  );

INSERT INTO social_entity_relationship (
    from_social_entity_id, to_social_entity_id, type_id, status_id, created_by, modified_by
)
SELECT @sphinx_human_eid, @panoply_eid, @member_of_type, @verified_status, @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_human_eid IS NOT NULL AND @panoply_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM social_entity_relationship ser
      WHERE ser.from_social_entity_id = @sphinx_human_eid
        AND ser.to_social_entity_id = @panoply_eid
        AND ser.type_id = @member_of_type
        AND ser.ended_at IS NULL
  );

-- Face capture + godmother graph so composite verification matches verified lifecycle.
INSERT INTO user_faces (
    user_id, embedding, embedding_dim, embedding_model_id,
    ai_appearance_description, generated_artwork_path, ai_model_id,
    captured_at, created_by, modified_by
)
SELECT
    @sphinx_uid,
    X'00000000',
    1,
    'panoply-operator-seed-v1',
    'Panoply operator structural profile.',
    NULL,
    'panoply-operator-seed-v1',
    UTC_TIMESTAMP(),
    @sphinx_uid,
    @sphinx_uid
FROM DUAL
WHERE @sphinx_uid IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM user_faces WHERE user_id = @sphinx_uid);

SET @milo_eid := (SELECT id FROM social_entity WHERE handle = 'milo' LIMIT 1);
SET @milo_community_eid := (
    SELECT modr.to_social_entity_id
    FROM social_entity_relationship modr
    WHERE modr.from_social_entity_id = @milo_eid
      AND modr.type_id = @moderator_type
      AND modr.ended_at IS NULL
    ORDER BY modr.id ASC
    LIMIT 1
);

INSERT INTO social_entity_relationship (
    from_social_entity_id, to_social_entity_id, type_id, status_id, created_by, modified_by
)
SELECT @sphinx_human_eid, @milo_eid, @godmother_type, @verified_status, @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_human_eid IS NOT NULL
  AND @milo_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM social_entity_relationship ser
      WHERE ser.from_social_entity_id = @sphinx_human_eid
        AND ser.to_social_entity_id = @milo_eid
        AND ser.type_id = @godmother_type
        AND ser.ended_at IS NULL
  );

INSERT INTO social_entity_relationship (
    from_social_entity_id, to_social_entity_id, type_id, status_id, created_by, modified_by
)
SELECT @sphinx_human_eid, @milo_community_eid, @member_of_type, @verified_status, @system_uid, @system_uid
FROM DUAL
WHERE @sphinx_human_eid IS NOT NULL
  AND @milo_community_eid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM social_entity_relationship ser
      WHERE ser.from_social_entity_id = @sphinx_human_eid
        AND ser.to_social_entity_id = @milo_community_eid
        AND ser.type_id = @member_of_type
        AND ser.ended_at IS NULL
  );
