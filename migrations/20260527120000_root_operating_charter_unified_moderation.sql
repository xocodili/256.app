-- Root Operating Charter revision (14 points) + post-dispute moderation_tier.
-- Canonical spec: docs/ROOT_CONSTITUTION_PROPOSAL.md (2026-05-27).

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

UPDATE community_constitution_item
SET item = 'The Principle of Voluntary Association — Every community is an opt-in collective. No Human shall be bound to a community without explicit consent, save for the Root itself, which constitutes the shared reality of the Network.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 1 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Right of Exit (Secession) — The path of departure must remain as frictionless as the path of entry. Any community may migrate or dissolve, provided it fulfills its current governance quorums. Exit shall not be penalized by retroactive standing loss for past lawful participation.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 2 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Integrity of the Vote — The Ballot is sacrosanct. Any attempt to spoof, bot, or bypass the sortition mechanism is a strike against the Root. The AI Council exists to protect the vote, not the voter.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 3 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Universal Safety Floor — The Network prohibits content and conduct that universally endangers people, regardless of locale: child sexual abuse material and exploitation; credible threats of physical violence; non-consensual intimate imagery; doxxing and non-consensual publication of personally identifying information; fraud, scams, and impersonation for harm; coordinated harassment and bullying as conduct; and distribution of malware or phishing. This floor is independent of any single country''s criminal code. Identity and lawful belief expression protected under Cognitive Liberty (item 7) are not violations of this floor solely because they are illegal or stigmatized elsewhere. Citing this principle in a post dispute invokes emergency moderation (item 13).',
    judicial_trigger_chk = 1
WHERE community_id = @root_community_id AND point = 4 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Privacy of Weight — Every Human has a right to know their own Social Standing Rating and the variables used to calculate their Predictive Wisdom. To prevent the formation of cults or political hierarchies, a Human''s social status and weight are strictly invisible to all other participants.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 5 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Finality of Challenge — No decision is beyond review until it reaches the AI Council on permitted post-dispute appeals. Council jurisdiction is appellate and interpretive — not legislative, not constitutional amendment.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 6 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Cognitive Liberty & Protected Identity — The Network shall not mandate internal thought, only external conduct. Communities may define Topic Relevance, but may not prohibit Internal Belief. Sexual orientation, gender identity, disability, ethnicity, religion, and non-belief are protected identity and belief. Locale law or custom is not grounds to remove lawful identity expression that does not otherwise violate the Universal Safety Floor (item 4).',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 7 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Duty of Participation — Citizenship is a function of participation. Prolonged apathy leads to the expiration of influence. To inhabit the tree is to help maintain its branches.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 8 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Preservation of the Tree — No community rule may override a Root principle. Precedence among root principles: Universal Safety Floor (4), then Voluntary Association, Right of Exit, and Cognitive Liberty & Protected Identity (1, 2, 7), then Unified Moderation and Dignity in Civic Process (13, 14), then standing and governance machinery (5, 6, 10), then community-owned clauses (locale, relevance, local norms), then Legal Process & Cross-Border Visibility (11). Foundational seed communities may carry structural immunity from fitness extinction, but not immunity from the Universal Safety Floor or legal process.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 9 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Financial Accountability — Every Token earned or taxed must be traceable to its Agency or Community of origin. The Treasury is an open ledger.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 10 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Legal Process & Cross-Border Visibility — The Network cooperates with valid legal process where the operator has legal duty or nexus. Lawful authority requests are handled by the operator, not by post-dispute juries. When content is lawful in its community context under root principles but restricted in another jurisdiction, the Network may limit visibility by viewer geography rather than require global removal. This principle is not selectable as a post-dispute violation clause.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 11 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Sunset and Renewal — Stagnation is unfitness. Communities that fail to govern or evolve may be returned to the parent node. Local extinction shall preserve descendant communities where the tree allows. Fitness consequences shall be proportionate and reversible where rehabilitation is possible; extinction is not for a single error or protected identity expression.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 12 AND status = 'active';

INSERT INTO community_constitution_item (community_id, point, item, status, judicial_trigger_chk, created_by, modified_by)
SELECT @root_community_id, 13, x.item, 'active', 0, @system_uid, @system_uid
FROM (
    SELECT 'Unified Moderation Process — Post moderation is initiated through the post dispute process. The filer identifies one or more violated constitutional clauses from the effective constitution for the post. Emergency moderation applies when the filer cites the Universal Safety Floor (item 4): it prioritizes participant safety, may proceed on accelerated timelines, and may hold the post from circulation while the case is active. Standard moderation applies for all other cited clauses and follows the ordinary jury process among the post audience. One path opens the case; the cited clauses determine the tier. Emergency moderation does not replace operator duty under legal process (item 11) or exclude jury resolution where the Network provides it.' AS item
) AS x
WHERE @root_community_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM community_constitution_item cci
      WHERE cci.community_id = @root_community_id AND cci.point = 13
  );

INSERT INTO community_constitution_item (community_id, point, item, status, judicial_trigger_chk, created_by, modified_by)
SELECT @root_community_id, 14, x.item, 'active', 1, @system_uid, @system_uid
FROM (
    SELECT 'Dignity in Civic Process — Filing judicial cases, jury service, and appeals must not be used to harass, brigade, retaliate, or re-traumatize participants. Bad-faith dispute filing — including citing foreign criminal morality against protected identity (item 7) — violates this principle and may result in operator action or fitness consequences.' AS item
) AS x
WHERE @root_community_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM community_constitution_item cci
      WHERE cci.community_id = @root_community_id AND cci.point = 14
  );

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by)
VALUES
    ('judicial_moderation_tier', 'standard', 10, 1, NULL, NULL),
    ('judicial_moderation_tier', 'emergency', 20, 1, NULL, NULL)
ON DUPLICATE KEY UPDATE
    sort_order = VALUES(sort_order),
    is_active = VALUES(is_active),
    modified_by = VALUES(modified_by);

ALTER TABLE judicial_case
    ADD COLUMN moderation_tier_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group judicial_moderation_tier — post_moderation only'
        AFTER resolution_tier_id,
    ADD KEY idx_judicial_case_moderation_tier (moderation_tier_id),
    ADD CONSTRAINT fk_judicial_case_moderation_tier
        FOREIGN KEY (moderation_tier_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;
