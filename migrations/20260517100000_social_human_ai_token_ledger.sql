-- Append-only Nom (ai_tokens) ledger for internal economics analysis.

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('nom_ledger_reason', 'credit_feed_ads', 10, 1, NULL, NULL),
    ('nom_ledger_reason', 'spend_billable_service', 20, 1, NULL, NULL),
    ('nom_ledger_reason', 'admin_adjustment', 30, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

CREATE TABLE social_human_ai_token_ledger (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    social_entity_id BIGINT UNSIGNED NOT NULL COMMENT 'Human wallet row (social_human.social_entity_id)',
    delta SMALLINT NOT NULL COMMENT 'Signed change applied to ai_tokens (+ credit, - spend)',
    balance_after SMALLINT UNSIGNED NOT NULL COMMENT 'Wallet balance immediately after this entry',
    reason_id BIGINT UNSIGNED NOT NULL COMMENT 'FK ref_key_values.id, group nom_ledger_reason',
    service_key VARCHAR(128) NULL COMMENT 'BillableService ledger key when reason is spend_billable_service',
    reference_subject VARCHAR(255) NULL COMMENT 'Opaque subject ref (e.g. judicial_case:8)',
    metadata_json JSON NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    KEY idx_nom_ledger_entity_time (social_entity_id, created_at),
    KEY idx_nom_ledger_reason_time (reason_id, created_at),
    KEY idx_nom_ledger_service (service_key),
    CONSTRAINT fk_nom_ledger_entity FOREIGN KEY (social_entity_id) REFERENCES social_entity (id) ON DELETE CASCADE,
    CONSTRAINT fk_nom_ledger_reason FOREIGN KEY (reason_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT,
    CONSTRAINT fk_nom_ledger_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT chk_nom_ledger_balance_range CHECK (balance_after <= 256)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
