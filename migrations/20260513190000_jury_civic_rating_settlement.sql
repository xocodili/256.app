-- Jury civic rating settlement: persist which rating surface each invite used,
-- and record when +/-0.075 adjustments were applied for a case (resolved or voided).

ALTER TABLE judicial_case_juror_invite
    ADD COLUMN rating_basis VARCHAR(16) NOT NULL DEFAULT 'global'
        COMMENT 'community = member_of relationship rating; global = social_entity.rating'
        AFTER juror_rating_at_invite,
    ADD COLUMN rating_community_entity_id BIGINT UNSIGNED NULL
        COMMENT 'When rating_basis=community, social_entity.id of the community (member_of target)'
        AFTER rating_basis;

ALTER TABLE judicial_case_juror_invite
    ADD CONSTRAINT chk_jcji_rating_basis CHECK (rating_basis IN ('global', 'community'));

ALTER TABLE judicial_case_juror_invite
    ADD CONSTRAINT fk_jcji_rating_community_entity
        FOREIGN KEY (rating_community_entity_id) REFERENCES social_entity (id) ON DELETE SET NULL;

ALTER TABLE judicial_case
    ADD COLUMN jury_civic_rating_applied_at DATETIME(6) NULL
        COMMENT 'When apply_jury_civic_rating_settlement completed for this case'
        AFTER closed_at;
