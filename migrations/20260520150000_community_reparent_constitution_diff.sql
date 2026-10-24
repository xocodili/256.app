-- Store constitution before/after diff for community re-parent jury review.

ALTER TABLE judicial_case_community_reparent
    ADD COLUMN constitution_diff_json JSON NULL
        COMMENT 'Added/removed effective-clause entries after proposed parent change'
        AFTER proposed_parent_community_id;
