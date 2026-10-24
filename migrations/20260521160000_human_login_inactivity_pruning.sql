-- Human account inactivity: 21 days since last login, then weekly rating penalties (same cadence as communities).

ALTER TABLE users
    ADD COLUMN last_login_at TIMESTAMP(6) NULL
        COMMENT 'Last successful sign-in' AFTER email_verified_at,
    ADD COLUMN inactivity_penalties_applied INT UNSIGNED NOT NULL DEFAULT 0
        COMMENT 'Login-inactivity rating penalties applied this idle spell' AFTER last_login_at,
    ADD COLUMN inactivity_pruning_notified_at TIMESTAMP(6) NULL
        COMMENT 'When the account-inactivity warning email was sent for this idle spell' AFTER inactivity_penalties_applied;

ALTER TABLE human_graph_event
    MODIFY event_type ENUM('death', 'inactivity_penalty') NOT NULL;

-- Seed last login from the most recent session activity.
UPDATE users u
SET last_login_at = (
    SELECT MAX(GREATEST(s.created_at, s.updated_at))
    FROM sessions s
    WHERE s.user_id = u.id
)
WHERE last_login_at IS NULL;

-- Users already in the inactivity window should not get a retroactive blast email.
UPDATE users
SET inactivity_pruning_notified_at = UTC_TIMESTAMP(6)
WHERE inactivity_penalties_applied > 0
   OR TIMESTAMPDIFF(
        DAY,
        COALESCE(last_login_at, created_at),
        UTC_TIMESTAMP(6)
    ) >= 21;
