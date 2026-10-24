-- Split event tagline from long-form copy (aligned with profit/non-profit `intention` / `description`).
ALTER TABLE social_gathering
  ADD COLUMN intention TEXT NULL COMMENT 'Short tagline / what to expect (distinct from description)' AFTER description;
