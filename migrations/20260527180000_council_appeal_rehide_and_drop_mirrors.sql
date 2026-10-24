-- Council appeal pending: re-hide subject posts from the aggregate feed (author-only).
-- Verdict copy is rendered on feed cards (@256 notice); drop thread mirror rows.

SET @under_review_status_id := (
    SELECT id
    FROM ref_key_values
    WHERE `group` = 'status'
      AND `key` = 'under_judicial_review'
      AND is_active = 1
    LIMIT 1
);

UPDATE post p
INNER JOIN judicial_case jc
    ON jc.subject_post_id = COALESCE(p.root_post_id, p.id)
INNER JOIN ref_key_values rk_st
    ON rk_st.id = jc.status_id
   AND rk_st.`group` = 'judicial_case_status'
   AND rk_st.`key` = 'escalated_to_ai'
   AND rk_st.is_active = 1
INNER JOIN ref_key_values rk_tr
    ON rk_tr.id = jc.trigger_type_id
   AND rk_tr.`group` = 'judicial_case_trigger'
   AND rk_tr.`key` = 'case_appeal'
   AND rk_tr.is_active = 1
SET p.status_id = @under_review_status_id
WHERE @under_review_status_id IS NOT NULL
  AND jc.closed_at IS NULL;

DELETE FROM post WHERE is_moderation_mirror = 1;
