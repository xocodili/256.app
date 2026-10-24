-- Juror invite status: FK to ref_key_values (group judicial_case_juror_invite_status) instead of VARCHAR literals.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('judicial_case_juror_invite_status', 'pending', 10, 1, NULL, NULL),
    ('judicial_case_juror_invite_status', 'accepted', 20, 1, NULL, NULL),
    ('judicial_case_juror_invite_status', 'declined', 30, 1, NULL, NULL),
    ('judicial_case_juror_invite_status', 'abstained', 40, 1, NULL, NULL),
    ('judicial_case_juror_invite_status', 'expired', 50, 1, NULL, NULL),
    ('judicial_case_juror_invite_status', 'voided', 60, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

ALTER TABLE judicial_case_juror_invite
    ADD COLUMN invite_status_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group judicial_case_juror_invite_status'
        AFTER invite_status;

UPDATE judicial_case_juror_invite jcji
INNER JOIN ref_key_values rk
    ON rk.`group` = 'judicial_case_juror_invite_status'
   AND rk.`key` = jcji.invite_status
   AND rk.is_active = 1
SET jcji.invite_status_id = rk.id;

ALTER TABLE judicial_case_juror_invite
    DROP INDEX idx_jcji_case_status,
    DROP CHECK chk_jcji_status;

ALTER TABLE judicial_case_juror_invite
    DROP COLUMN invite_status;

ALTER TABLE judicial_case_juror_invite
    MODIFY COLUMN invite_status_id BIGINT UNSIGNED NOT NULL,
    ADD CONSTRAINT fk_jcji_invite_status
        FOREIGN KEY (invite_status_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    ADD KEY idx_jcji_case_status (judicial_case_id, invite_status_id);
