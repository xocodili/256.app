-- Extend judicial tables for voting and verdict resolution.
-- 1) Add vote_direction to juror invites so each juror's vote is recorded.
-- 2) Add jury_size to judicial_case, frozen at filing time from ceil(sqrt(eligible_pool)).

ALTER TABLE judicial_case_juror_invite
    ADD COLUMN vote_direction VARCHAR(8) NULL
        COMMENT 'for|against; NULL until juror casts vote'
        AFTER invite_status;

ALTER TABLE judicial_case_juror_invite
    ADD CONSTRAINT chk_jcji_vote_direction
        CHECK (vote_direction IS NULL OR vote_direction IN ('for', 'against'));

ALTER TABLE judicial_case
    ADD COLUMN jury_size TINYINT UNSIGNED NOT NULL DEFAULT 0
        COMMENT 'Target jury count, frozen at filing from ceil(sqrt(eligible_pool))'
        AFTER resolution_tier_key;
