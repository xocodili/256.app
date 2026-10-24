-- Jury abstention support: allow jurors to abstain (up to 3/year without penalty by
-- product policy; the wallet/penalty rule is enforced in application code, not SQL).
-- This migration only widens the invite_status domain to include 'abstained' so the
-- nominate_jurors_for_open_cases worker can free the seat and select a replacement.

ALTER TABLE judicial_case_juror_invite
    DROP CHECK chk_jcji_status;

ALTER TABLE judicial_case_juror_invite
    ADD CONSTRAINT chk_jcji_status CHECK (
        invite_status IN ('pending', 'accepted', 'declined', 'abstained', 'expired', 'voided')
    );

ALTER TABLE judicial_case_juror_invite
    MODIFY COLUMN invite_status VARCHAR(16) NOT NULL DEFAULT 'pending'
        COMMENT 'pending | accepted | declined | abstained | expired | voided';
