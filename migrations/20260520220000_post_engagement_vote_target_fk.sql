-- FK for per-post engagement votes (root + replies).

ALTER TABLE post_engagement_vote
    ADD CONSTRAINT fk_pev_target_post
        FOREIGN KEY (target_post_id) REFERENCES post (id) ON DELETE CASCADE;
