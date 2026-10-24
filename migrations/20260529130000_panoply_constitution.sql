-- Panoply community charter: demarcation of operator mandate, scope, and conduct.
-- Descriptive only (judicial_trigger_chk = 0) — not wired to filing or automated enforcement in v1.

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @panoply_community_id := (
    SELECT se.id
    FROM social_entity se
    WHERE se.handle = 'panoply'
    LIMIT 1
);

INSERT INTO community_constitution_item (community_id, point, item, status, judicial_trigger_chk, created_by, modified_by)
SELECT @panoply_community_id, x.point, x.item, 'active', 0, @system_uid, @system_uid
FROM (
    SELECT 1 AS point,
        'Mandate — Panoply exists to protect the habitat of 256.app: contain acute harm, preserve audit trails, and exercise platform safety powers when community-scoped tools are insufficient or too slow. Panoply is not a public community and not a general governance layer for ordinary disputes.' AS item
    UNION ALL
    SELECT 2,
        'Two planes — Operators remain ordinary participants everywhere except the ops dashboard (`/home/ops`). Clearance does not bypass community membership rules, feed visibility, or jury eligibility in ordinary use. Ops powers activate only inside the dashboard and its audited actions.' AS item
    UNION ALL
    SELECT 3,
        'Subordination to Root — The Root Operating Charter (`operating_charter_256`) governs post disputes, emergency safety (§4.1–§4.7), and civic moderation among audiences. This charter governs operator conduct and demarcation only. Post filings cite Root clauses, not Panoply clauses.' AS item
    UNION ALL
    SELECT 4,
        'Clearance model — Membership is invite-only. Clearance grants access to the ops dashboard and platform fitness death immunity while an active verified `member_of` edge exists. Clearance is not transferable, hereditary, or discoverable outside Panoply.' AS item
    UNION ALL
    SELECT 5,
        'Concealment — `@panoply` and operator roster membership are hidden from search, public profiles, and non-operator profile views. Outside Panoply, operators present as regular citizens. Inside Panoply, operators may recognize one another.' AS item
    UNION ALL
    SELECT 6,
        'Audit obligation — Every dashboard action requires a written ops note and is recorded in the platform safety audit log. Undocumented or casual use of ops tools violates this charter.' AS item
    UNION ALL
    SELECT 7,
        'Proportionality — Operators use the least restrictive effective tool first: domain block, session termination, emergency safety filing, civic dispute filing, and eject only when lesser measures are insufficient or inappropriate. Good faith judgment applies; this charter demarcates duty, not automatic sanctions.' AS item
    UNION ALL
    SELECT 8,
        'Platform fitness immunity — Active Panoply membership grants immunity from rating-zero death and login-inactivity death only. It does not grant immunity from platform safety eject, legal-process obligations, or ordinary community standing consequences in the graph.' AS item
    UNION ALL
    SELECT 9,
        'Relationship to legal process — Operators cooperate with valid legal process, preserve evidence where required, and do not use ops tools to obstruct lawful investigation. Legal-process duties in Root item 11 remain operative.' AS item
    UNION ALL
    SELECT 10,
        'Operator trust (v1) — Panoply-on-Panoply enforcement is deferred. The roster is trusted; revocation and operator eject are product decisions reserved for a later phase. This charter states demarcation now, not automated discipline among operators.' AS item
) AS x
WHERE @panoply_community_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1
      FROM community_constitution_item cci
      WHERE cci.community_id = @panoply_community_id
        AND cci.point = x.point
  );
