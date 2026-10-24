-- Link a moderation appeal (`judicial_case_trigger.moderation_appeal`) to the resolved post dispute it revisits.
-- One appeal row per prior case (enforced by UNIQUE on appealed_from when set).

ALTER TABLE judicial_case
    ADD COLUMN appealed_from_judicial_case_id BIGINT UNSIGNED NULL
        COMMENT 'FK judicial_case.id — prior resolved post_moderation case when trigger is moderation_appeal'
        AFTER subject_post_id,
    ADD KEY idx_judicial_case_appealed_from (appealed_from_judicial_case_id),
    ADD CONSTRAINT fk_judicial_case_appealed_from
        FOREIGN KEY (appealed_from_judicial_case_id) REFERENCES judicial_case (id) ON DELETE RESTRICT,
    ADD CONSTRAINT uq_judicial_case_appealed_from UNIQUE (appealed_from_judicial_case_id);
