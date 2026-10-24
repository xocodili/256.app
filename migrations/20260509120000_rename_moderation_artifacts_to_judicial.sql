-- Greenfield scrub: product naming moves to "judicial"; ref_key `judicial_case_trigger` keys
-- `post_moderation` and `moderation_appeal` remain as stored trigger enumerations (unchanged).

-- Post lifecycle status: under_moderation → under_judicial_review
UPDATE ref_key_values
SET `key` = 'under_judicial_review'
WHERE `group` = 'status' AND `key` = 'under_moderation';

-- Constitution: complaint-eligible flag column rename
ALTER TABLE community_constitution_item
    DROP CHECK chk_community_constitution_moderation_chk;
ALTER TABLE community_constitution_item
    DROP INDEX idx_community_constitution_moderation_chk;
ALTER TABLE community_constitution_item
    CHANGE COLUMN moderation_chk judicial_trigger_chk TINYINT(1) NOT NULL DEFAULT 0
        COMMENT '1 when this constitutional item is eligible as a judicial case complaint clause',
    ADD KEY idx_community_constitution_judicial_trigger (community_id, judicial_trigger_chk),
    ADD CONSTRAINT chk_community_constitution_judicial_trigger_chk CHECK (judicial_trigger_chk IN (0, 1));

-- AI scope / decision logging tables (internal names; no product "moderation" token)
DROP VIEW IF EXISTS vw_ai_council_integrity;

ALTER TABLE ai_moderation_decision_clause
    DROP FOREIGN KEY fk_ai_mod_decision_clause_log;
ALTER TABLE ai_moderation_decision_clause
    DROP FOREIGN KEY fk_ai_mod_decision_clause_constitution;

ALTER TABLE ai_moderation_decision_log
    DROP FOREIGN KEY fk_ai_mod_log_scope;

RENAME TABLE ai_moderation_scope TO ai_judicial_scope;
RENAME TABLE ai_moderation_decision_log TO ai_judicial_decision_log;
RENAME TABLE ai_moderation_decision_clause TO ai_judicial_decision_clause;

ALTER TABLE ai_judicial_decision_log
    CHANGE COLUMN moderation_scope_id judicial_scope_id BIGINT UNSIGNED NULL COMMENT 'optional FK ai_judicial_scope.id',
    DROP INDEX idx_ai_mod_log_scope,
    ADD KEY idx_ai_judicial_log_scope (judicial_scope_id);

ALTER TABLE ai_judicial_decision_log
    ADD CONSTRAINT fk_ai_judicial_log_scope FOREIGN KEY (judicial_scope_id) REFERENCES ai_judicial_scope (id) ON DELETE SET NULL;

ALTER TABLE ai_judicial_decision_clause
    CHANGE COLUMN moderation_log_id judicial_decision_log_id BIGINT UNSIGNED NOT NULL;

ALTER TABLE ai_judicial_decision_clause
    ADD CONSTRAINT fk_ai_judicial_decision_clause_log FOREIGN KEY (judicial_decision_log_id) REFERENCES ai_judicial_decision_log (id) ON DELETE CASCADE,
    ADD CONSTRAINT fk_ai_judicial_decision_clause_constitution FOREIGN KEY (constitution_item_id) REFERENCES community_constitution_item (id) ON DELETE RESTRICT;

CREATE VIEW vw_ai_council_integrity AS
SELECT
    (SELECT COUNT(*) FROM ai_actor WHERE is_active = 1) AS ai_actor_count,
    (SELECT COUNT(*) FROM ai_council_membership WHERE is_active = 1) AS council_membership_count,
    (SELECT COUNT(*) FROM ai_domain_ownership WHERE is_active = 1) AS domain_ownership_count,
    (SELECT COUNT(*) FROM ai_profile_assignment WHERE is_active = 1) AS profile_assignment_count,
    (SELECT COUNT(*) FROM ai_judicial_scope WHERE is_active = 1) AS judicial_scope_count,
    (
        (SELECT COUNT(*) FROM ai_actor WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_council_membership WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_domain_ownership WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_profile_assignment WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_judicial_scope WHERE is_active = 1) >= 12
    ) AS ok;
