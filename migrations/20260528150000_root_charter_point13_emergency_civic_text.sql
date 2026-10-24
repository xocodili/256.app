-- Align Root point 13 (and point 9 precedence) with shipped emergency/civic split (2026-05-28).
-- Canonical spec: docs/ROOT_CONSTITUTION_PROPOSAL.md — interim 14-point charter + §4.1–§4.7 sub-clauses.

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

UPDATE community_constitution_item
SET item = 'Civic Moderation Process — Constitutional disagreement is resolved through the post dispute process among the post audience. The filer cites violated clauses from the effective constitution for the post. Emergency safety under §4.1–§4.7 (points 401–407) follows a separate path: the post is suppressed immediately and no jury convenes until the author contests (1 Nom). All other selectable dispute clauses follow the ordinary jury process among the post audience. Neither path replaces operator duty under legal process (item 11). Civic moderation is not an emergency service.'
WHERE community_id = @root_community_id AND point = 13 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Preservation of the Tree — No community rule may override a Root principle. Precedence among root principles: Universal Safety Floor (4), then Voluntary Association, Right of Exit, and Cognitive Liberty & Protected Identity (1, 2, 7), then Civic Moderation, Emergency Safety (§4.1–§4.7), and Dignity in Civic Process (13, 14), then standing and governance machinery (5, 6, 10), then community-owned clauses (locale, relevance, local norms), then Legal Process & Cross-Border Visibility (11). Foundational seed communities may carry structural immunity from fitness extinction, but not immunity from the Universal Safety Floor or legal process.'
WHERE community_id = @root_community_id AND point = 9 AND status = 'active';
