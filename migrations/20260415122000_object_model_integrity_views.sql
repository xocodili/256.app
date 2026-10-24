-- Migration-level integrity views for the refactored object model.

DROP VIEW IF EXISTS vw_ai_council_integrity;
CREATE VIEW vw_ai_council_integrity AS
SELECT
    (SELECT COUNT(*) FROM ai_actor WHERE is_active = 1) AS ai_actor_count,
    (SELECT COUNT(*) FROM ai_council_membership WHERE is_active = 1) AS council_membership_count,
    (SELECT COUNT(*) FROM ai_domain_ownership WHERE is_active = 1) AS domain_ownership_count,
    (SELECT COUNT(*) FROM ai_profile_assignment WHERE is_active = 1) AS profile_assignment_count,
    (SELECT COUNT(*) FROM ai_moderation_scope WHERE is_active = 1) AS moderation_scope_count,
    (
        (SELECT COUNT(*) FROM ai_actor WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_council_membership WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_domain_ownership WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_profile_assignment WHERE is_active = 1) >= 12
        AND (SELECT COUNT(*) FROM ai_moderation_scope WHERE is_active = 1) >= 12
    ) AS ok;

DROP VIEW IF EXISTS vw_social_entity_safety_compat;
CREATE VIEW vw_social_entity_safety_compat AS
SELECT
    se.id AS social_entity_id,
    se.is_adult_or_sensitive,
    esc.is_adult,
    esc.is_sensitive,
    (CASE WHEN se.is_adult_or_sensitive = 1 THEN 1 ELSE 0 END) AS legacy_flag_normalized,
    (CASE WHEN esc.is_adult = 1 OR esc.is_sensitive = 1 THEN 1 ELSE 0 END) AS structured_flag_normalized
FROM social_entity se
LEFT JOIN social_entity_safety_classification esc
    ON esc.social_entity_id = se.id;
