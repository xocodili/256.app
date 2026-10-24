-- 256.app database user (run as MySQL root).
--
-- Replace YOUR_DB_PASSWORD below with the same password you put in `.env` as DATABASE_URL
-- (do not commit real passwords in this file if you change it).
--
-- From host (replace container name):
--   Get-Content scripts\mysql\create_webadm_user.sql | docker exec -i <mysql_container> mysql -u root -p

CREATE DATABASE IF NOT EXISTS twofivesix
  CHARACTER SET utf8mb4
  COLLATE utf8mb4_unicode_ci;

CREATE USER IF NOT EXISTS 'webadm'@'%' IDENTIFIED BY 'YOUR_DB_PASSWORD';

GRANT ALL PRIVILEGES ON twofivesix.* TO 'webadm'@'%';
FLUSH PRIVILEGES;
