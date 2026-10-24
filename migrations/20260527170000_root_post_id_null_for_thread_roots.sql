-- Root thread posts keep root_post_id NULL (see 20260417190000_post_root_post_id.sql).
-- create_post had been setting root_post_id = id, which hid new posts from feed queries
-- that filter on root_post_id IS NULL.

UPDATE post
SET root_post_id = NULL
WHERE parent_post_id IS NULL
  AND root_post_id = id;
