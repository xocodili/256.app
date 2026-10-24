-- Signup Nom grant ledger reason (16 Noms on new human profile creation).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('nom_ledger_reason', 'credit_signup', 5, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);
