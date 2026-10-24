-- Voluntary account deletion records a distinct death_source on social_human.

ALTER TABLE social_human
    MODIFY death_source ENUM('rating_zero', 'manual_ops', 'user_requested') NULL
    COMMENT 'Why the human was removed';
