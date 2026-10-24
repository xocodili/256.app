-- Canonical backend/storage naming uses "followers".
-- Keep compatibility by remapping previously stored "admirers" rows.
UPDATE post_audience_scope
SET scope_key = 'followers'
WHERE scope_key = 'admirers';

ALTER TABLE post_audience_scope
    DROP CHECK chk_post_audience_scope_key,
    ADD CONSTRAINT chk_post_audience_scope_key
        CHECK (scope_key IN ('friends', 'family', 'private', 'public', 'followers'));
