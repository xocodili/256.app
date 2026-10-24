-- Add per-connection quality score so the same participant can be rated differently per relationship edge.
ALTER TABLE social_entity_relationship
    ADD COLUMN rating DECIMAL(3,2) NOT NULL DEFAULT 2.50
        COMMENT 'Internal connection quality rating (0.00..5.00, two decimals max)'
        AFTER status_id,
    ADD CONSTRAINT chk_ser_rating_range CHECK (rating >= 0.00 AND rating <= 5.00);

-- Backfill existing edges to the neutral default requested for new/legacy records.
UPDATE social_entity_relationship
SET rating = 2.50
WHERE rating IS NULL OR rating = 0.00;
