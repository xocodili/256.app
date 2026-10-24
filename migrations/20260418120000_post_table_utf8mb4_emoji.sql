-- Post body fields must store full Unicode (emoji). If the table inherited
-- utf8mb3 (MySQL's legacy `utf8`), inserts fail with ER_CANNOT_CONVERT_STRING (3988).
ALTER TABLE post
    CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
