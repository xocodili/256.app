-- Correct panoply seed operator email (was missing .com TLD).
SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

UPDATE users
SET email = 'operator@example.com',
    modified_by = @system_uid
WHERE email = 'operator@example'
  AND NOT EXISTS (SELECT 1 FROM (SELECT id FROM users WHERE email = 'operator@example.com') AS existing_operator);
