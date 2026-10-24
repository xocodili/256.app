-- One-time upgrade if your database was created from an older initial migration
-- (before created_by / modified_by / sessions.updated_at). Run as MySQL root.
--
-- If a statement fails with "Duplicate column" or "Duplicate key name", skip that statement.

ALTER TABLE users
    ADD COLUMN created_by BIGINT UNSIGNED NULL AFTER updated_at,
    ADD COLUMN modified_by BIGINT UNSIGNED NULL AFTER created_by;

ALTER TABLE users
    ADD CONSTRAINT fk_users_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    ADD CONSTRAINT fk_users_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL;

ALTER TABLE sessions
    ADD COLUMN updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP AFTER created_at,
    ADD COLUMN created_by BIGINT UNSIGNED NULL AFTER updated_at,
    ADD COLUMN modified_by BIGINT UNSIGNED NULL AFTER created_by;

ALTER TABLE sessions
    ADD CONSTRAINT fk_sessions_created_by FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    ADD CONSTRAINT fk_sessions_modified_by FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL;
