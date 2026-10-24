-- New community memberships start at 0.00 relationship rating (was DEFAULT 2.50).
-- Juror pool logic uses COALESCE(ser.rating, 0.00) for eligibility.

ALTER TABLE social_entity_relationship
    MODIFY COLUMN rating DECIMAL(3,2) NOT NULL DEFAULT 0.00
        COMMENT 'Internal connection quality rating (0.00..5.00, two decimals max)';
