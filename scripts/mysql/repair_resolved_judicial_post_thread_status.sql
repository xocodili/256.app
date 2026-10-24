-- Repair: post thread stuck `under_judicial_review` while `judicial_case` is already `resolved`.
--
-- Why: the home feed hides non-author viewers when `post.status_id` -> `status.under_judicial_review`,
-- regardless of `judicial_case.status_id`. The app normally updates `post` when the verdict closes;
-- manual SQL on `judicial_case` alone does not.
--
-- Caution:
--   * Run the PREVIEW first. Wrap changes in START TRANSACTION / COMMIT (or ROLLBACK).
--   * Verdict in code uses *weighted* votes (two-thirds of weight), not raw counts. The bulk
--     UPDATEs below use `jury_votes_for` vs `jury_votes_against` as a **heuristic** only.
--     For a sensitive case, use the single-case block with `AND jc.id = ?` instead.
--   * `modified_by` is cleared to match app behavior after verdict.

SET @rk_status_group := 'status';
SET @rk_jc_status_group := 'judicial_case_status';
SET @rk_jc_trigger_group := 'judicial_case_trigger';

SET @post_verified_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = @rk_status_group AND `key` = 'verified' AND is_active = 1 LIMIT 1
);
SET @post_flagged_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = @rk_status_group AND `key` = 'flagged' AND is_active = 1 LIMIT 1
);
SET @post_under_judicial_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = @rk_status_group AND `key` = 'under_judicial_review' AND is_active = 1 LIMIT 1
);
SET @jc_resolved_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = @rk_jc_status_group AND `key` = 'resolved' AND is_active = 1 LIMIT 1
);
SET @jc_trigger_post_mod_id := (
    SELECT id FROM ref_key_values
    WHERE `group` = @rk_jc_trigger_group AND `key` = 'post_moderation' AND is_active = 1 LIMIT 1
);

-- ---------------------------------------------------------------------------
-- PREVIEW: resolved post-moderation cases whose thread is still under review
-- ---------------------------------------------------------------------------
SELECT
    jc.id AS judicial_case_id,
    jc.subject_post_id,
    COALESCE(pr.root_post_id, pr.id) AS thread_root_id,
    jc.jury_votes_for,
    jc.jury_votes_against,
    rk_post.`key` AS post_status_key
FROM judicial_case jc
INNER JOIN post pr ON pr.id = jc.subject_post_id
INNER JOIN post root ON root.id = COALESCE(pr.root_post_id, pr.id)
INNER JOIN ref_key_values rk_post ON rk_post.id = root.status_id
    AND rk_post.`group` = @rk_status_group
    AND rk_post.is_active = 1
WHERE jc.status_id = @jc_resolved_id
  AND jc.trigger_type_id = @jc_trigger_post_mod_id
  AND jc.subject_post_id IS NOT NULL
  AND rk_post.`key` = 'under_judicial_review';

-- ---------------------------------------------------------------------------
-- OPTIONAL: single case (replace 0 with real `judicial_case.id`)
-- ---------------------------------------------------------------------------
-- SET @case_id := 0;
-- Then add `AND jc.id = @case_id` to the UPDATE WHERE clauses below, or use the dedicated block.

-- ---------------------------------------------------------------------------
-- A) Clears-majority (heuristic): set entire thread -> verified
-- ---------------------------------------------------------------------------
-- START TRANSACTION;

UPDATE post p
INNER JOIN (
    SELECT DISTINCT COALESCE(pr.root_post_id, pr.id) AS root_id
    FROM judicial_case jc
    INNER JOIN post pr ON pr.id = jc.subject_post_id
    WHERE jc.status_id = @jc_resolved_id
      AND jc.trigger_type_id = @jc_trigger_post_mod_id
      AND jc.subject_post_id IS NOT NULL
      AND jc.jury_votes_against > jc.jury_votes_for
) t ON (p.id = t.root_id OR p.root_post_id = t.root_id)
SET p.status_id = @post_verified_id,
    p.modified_by = NULL
WHERE p.status_id = @post_under_judicial_id;

-- COMMIT;

-- ---------------------------------------------------------------------------
-- B) Violates-majority (heuristic): set entire thread -> flagged
-- ---------------------------------------------------------------------------
-- START TRANSACTION;

-- UPDATE post p
-- INNER JOIN (
--     SELECT DISTINCT COALESCE(pr.root_post_id, pr.id) AS root_id
--     FROM judicial_case jc
--     INNER JOIN post pr ON pr.id = jc.subject_post_id
--     WHERE jc.status_id = @jc_resolved_id
--       AND jc.trigger_type_id = @jc_trigger_post_mod_id
--       AND jc.subject_post_id IS NOT NULL
--       AND jc.jury_votes_for > jc.jury_votes_against
-- ) t ON (p.id = t.root_id OR p.root_post_id = t.root_id)
-- SET p.status_id = @post_flagged_id,
--     p.modified_by = NULL
-- WHERE p.status_id = @post_under_judicial_id;

-- COMMIT;

-- ---------------------------------------------------------------------------
-- C) Single case id -> verified (safest when counts are not trustworthy)
-- ---------------------------------------------------------------------------
-- SET @case_id := 123;   -- judicial_case.id
-- START TRANSACTION;
--
-- UPDATE post p
-- INNER JOIN (
--     SELECT COALESCE(pr.root_post_id, pr.id) AS root_id
--     FROM judicial_case jc
--     INNER JOIN post pr ON pr.id = jc.subject_post_id
--     WHERE jc.id = @case_id
--       AND jc.status_id = @jc_resolved_id
--       AND jc.trigger_type_id = @jc_trigger_post_mod_id
--       AND jc.subject_post_id IS NOT NULL
-- ) t ON (p.id = t.root_id OR p.root_post_id = t.root_id)
-- SET p.status_id = @post_verified_id,
--     p.modified_by = NULL
-- WHERE p.status_id = @post_under_judicial_id;
--
-- COMMIT;
