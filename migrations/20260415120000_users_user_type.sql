-- Distinguish human accounts from platform AI personas (`users.user_type_id` → ref_key_values group `user_type`).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
('user_type', 'human', 10, 1, NULL, NULL),
('user_type', 'ai', 20, 1, NULL, NULL);

ALTER TABLE users
    ADD COLUMN user_type_id BIGINT UNSIGNED NULL COMMENT 'FK ref_key_values.id, group user_type' AFTER status_id,
    ADD KEY idx_users_user_type_id (user_type_id),
    ADD CONSTRAINT fk_users_user_type_id FOREIGN KEY (user_type_id) REFERENCES ref_key_values (id) ON DELETE SET NULL;

-- Default existing rows to human until explicitly set (e.g. council seeds use `ai`).
UPDATE users u
INNER JOIN ref_key_values rk
    ON rk.`group` = 'user_type' AND rk.`key` = 'human'
SET u.user_type_id = rk.id
WHERE u.user_type_id IS NULL;
