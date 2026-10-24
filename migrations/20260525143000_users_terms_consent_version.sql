-- Onboarding Root Charter / terms acceptance version (non-null = consent recorded for that version).
ALTER TABLE users
    ADD COLUMN terms_consent_version VARCHAR(32) NULL DEFAULT NULL
        COMMENT 'Root Charter / onboarding terms version accepted'
        AFTER email_verified_at;
