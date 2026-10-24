-- Rating schedule revision (δ = 0.035) + human login-inactivity phased notifications/refund ledger.

ALTER TABLE judicial_case
    ADD COLUMN community_jury_resolved_bonus_applied_at DATETIME(6) NULL
        COMMENT 'When +3×δ community bonus applied for successful jury resolution'
        AFTER community_rating_feedback_applied_at;

ALTER TABLE users
    ADD COLUMN inactivity_warning_notified_at TIMESTAMP(6) NULL
        COMMENT '21 idle days: one week before decrement phase' AFTER inactivity_pruning_notified_at,
    ADD COLUMN inactivity_decrement_started_notified_at TIMESTAMP(6) NULL
        COMMENT '28 idle days: daily decrement phase began' AFTER inactivity_warning_notified_at,
    ADD COLUMN inactivity_acceleration_3x_notified_at TIMESTAMP(6) NULL
        COMMENT 'Decrement day 22: 3× phase began' AFTER inactivity_decrement_started_notified_at,
    ADD COLUMN inactivity_acceleration_6x_notified_at TIMESTAMP(6) NULL
        COMMENT 'Decrement day 43: 6× phase began' AFTER inactivity_acceleration_3x_notified_at,
    ADD COLUMN inactivity_rating_debited DECIMAL(3, 2) NOT NULL DEFAULT 0.00
        COMMENT 'Sum of login-inactivity decrements this spell; refunded on sign-in'
        AFTER inactivity_acceleration_6x_notified_at;
