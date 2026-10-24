-- Fiat Nom pack purchases (Stripe PaymentIntent idempotency).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('nom_ledger_reason', 'credit_purchase', 15, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE nom_purchase (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'Human wallet row',
    sku VARCHAR(64) NOT NULL,
    noms_granted SMALLINT UNSIGNED NOT NULL,
    amount_cents INT UNSIGNED NOT NULL,
    currency CHAR(3) NOT NULL,
    stripe_payment_intent_id VARCHAR(255) NOT NULL,
    status VARCHAR(16) NOT NULL COMMENT 'pending | completed | failed',
    balance_after SMALLINT UNSIGNED NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_nom_purchase_stripe_pi (stripe_payment_intent_id),
    KEY idx_nom_purchase_user_time (user_id, created_at),
    KEY idx_nom_purchase_entity_time (social_entity_id, created_at),
    CONSTRAINT fk_nom_purchase_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT fk_nom_purchase_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT chk_nom_purchase_noms_positive CHECK (noms_granted > 0),
    CONSTRAINT chk_nom_purchase_amount_positive CHECK (amount_cents > 0),
    CONSTRAINT chk_nom_purchase_balance_after_range CHECK (balance_after IS NULL OR balance_after <= 256)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
