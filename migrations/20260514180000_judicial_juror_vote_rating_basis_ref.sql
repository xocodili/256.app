-- Juror invite vote direction and rating basis → ref_key_values (FK ids).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('judicial_case_juror_invite_vote_direction', 'for', 10, 1, NULL, NULL),
    ('judicial_case_juror_invite_vote_direction', 'against', 20, 1, NULL, NULL),
    ('judicial_case_juror_invite_rating_basis', 'global', 10, 1, NULL, NULL),
    ('judicial_case_juror_invite_rating_basis', 'community', 20, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

ALTER TABLE judicial_case_juror_invite
    ADD COLUMN vote_direction_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group judicial_case_juror_invite_vote_direction; NULL until vote cast'
        AFTER vote_direction,
    ADD COLUMN rating_basis_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group judicial_case_juror_invite_rating_basis'
        AFTER rating_basis;

UPDATE judicial_case_juror_invite jcji
INNER JOIN ref_key_values rk
    ON rk.`group` = 'judicial_case_juror_invite_rating_basis'
   AND rk.`key` = jcji.rating_basis
   AND rk.is_active = 1
SET jcji.rating_basis_id = rk.id;

UPDATE judicial_case_juror_invite jcji
INNER JOIN ref_key_values rk
    ON rk.`group` = 'judicial_case_juror_invite_vote_direction'
   AND rk.`key` = jcji.vote_direction
   AND rk.is_active = 1
SET jcji.vote_direction_id = rk.id
WHERE jcji.vote_direction IS NOT NULL;

ALTER TABLE judicial_case_juror_invite
    DROP CHECK chk_jcji_vote_direction,
    DROP CHECK chk_jcji_rating_basis;

ALTER TABLE judicial_case_juror_invite
    DROP COLUMN vote_direction,
    DROP COLUMN rating_basis;

ALTER TABLE judicial_case_juror_invite
    MODIFY COLUMN rating_basis_id BIGINT UNSIGNED NOT NULL,
    ADD CONSTRAINT fk_jcji_vote_direction
        FOREIGN KEY (vote_direction_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    ADD CONSTRAINT fk_jcji_rating_basis
        FOREIGN KEY (rating_basis_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;
