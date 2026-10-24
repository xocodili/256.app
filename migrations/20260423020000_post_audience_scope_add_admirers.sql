-- Keep DB check in sync with app/system handles by allowing @admirers.
ALTER TABLE post_audience_scope
    DROP CHECK chk_post_audience_scope_key,
    ADD CONSTRAINT chk_post_audience_scope_key
        CHECK (scope_key IN ('friends', 'family', 'private', 'public', 'admirers'));
