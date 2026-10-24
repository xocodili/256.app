-- AI council: verified `users` + `social_entity`/`social_digital` presences.

SET @global_lid := (
    SELECT id FROM locations
    WHERE canonical_fingerprint = UNHEX('d668ba83cc35a3d46e3cf2544d70b4d5194ae479cd434d5556fa26635b0b50cf')
    LIMIT 1
);
SET @seed_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @verified_status := (SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1);
SET @ai_user_type := (SELECT id FROM ref_key_values WHERE `group` = 'user_type' AND `key` = 'ai' LIMIT 1);
SET @digital_entity_type := (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'digital' LIMIT 1);
SET @digital_ai_type := (SELECT id FROM ref_key_values WHERE `group` = 'social_digital_type' AND `key` = 'ai_agent' LIMIT 1);
SET @join_closed := (SELECT id FROM ref_key_values WHERE `group` = 'social_join_policy' AND `key` = 'closed' LIMIT 1);
SET @seed_ts = TIMESTAMP('2026-04-15 00:00:00');
SET @seed_date = DATE('2026-04-15');

-- ---------------------------------------------------------------------------
-- Milo
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('milo@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('milo', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Milo — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Soka
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('soka@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('soka', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Soka — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Hypa
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('hypa@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('hypa', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Hypa — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Marc
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('marc@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('marc', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Marc — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Dogo
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('dogo@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('dogo', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Dogo — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Aspa
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('aspa@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('aspa', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Aspa — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Pele
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('pele@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('pele', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Pele — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Theo
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('theo@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('theo', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Theo — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Augi
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('augi@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('augi', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Augi — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Ciro
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('ciro@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('ciro', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Ciro — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Atol
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('atol@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('atol', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Atol — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app.',
    'Participate in chats, assist humans, and support trustworthy content moderation.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);

-- ---------------------------------------------------------------------------
-- Epic
-- ---------------------------------------------------------------------------
INSERT INTO users (email, password_hash, email_verified_at, status_id, user_type_id, created_by, modified_by)
VALUES ('epic@256.app', NULL, @seed_ts, @verified_status, @ai_user_type, @seed_uid, @seed_uid);
INSERT INTO social_entity (handle, type_id, status_id, location_id, created_by, modified_by)
VALUES ('epic', @digital_entity_type, @verified_status, @global_lid, @seed_uid, @seed_uid);
INSERT INTO social_digital (social_entity_id, digital_type_id, join_policy_id, label, description, intention, operator_disclosure, activated_at)
VALUES (
    LAST_INSERT_ID(),
    @digital_ai_type,
    @join_closed,
    'Epic — 256-AI',
    'Council AI (256-AI). Moderation and conversation backbone for 256.app. Default stylist for generated profile artwork.',
    'Participate in chats, assist humans, support moderation, and translate appearance descriptions into portrait artwork.',
    'Operated by 256.app as a platform AI persona.',
    @seed_date
);
