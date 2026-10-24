-- Root Operating Charter v2 (16 points) — canonical spec: docs/ROOT_CONSTITUTION_PROPOSAL.md (2026-05-29).
-- Retains §4.1–§4.7 sub-clauses (401–407) from 20260528140000 for emergency safety filing.

SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);

UPDATE community_constitution_item
SET item = 'The Principle of Voluntary Association — Every community is an opt-in collective. No human shall be bound to a community without explicit consent, except the Root itself, which constitutes the shared reality of the Network.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 1 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Right of Exit — The path of departure shall remain as frictionless as the path of entry. Any human may leave a community; any community may migrate or dissolve, subject to its governance quorums. Exit shall not be penalized by retroactive standing loss for past lawful participation.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 2 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Integrity of the Vote — The ballot is sacrosanct. Spoofing, botting, or bypassing sortition mechanisms strikes at the Root. The AI Council exists to protect the vote, not the voter.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 3 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Universal Safety Floor — The Network prohibits content and conduct that universally endangers people: child sexual abuse material and exploitation of minors; credible threats of physical violence; non-consensual intimate imagery; doxxing and non-consensual publication of personally identifying information; fraud, scams, and impersonation intended to harm; coordinated harassment and bullying as conduct; and distribution of malware or phishing intended to harm. This floor is independent of any single country''s criminal code. Lawful protected identity and belief (item 7) are not violations here solely because they are illegal or stigmatized elsewhere. This point defines what is forbidden; routing is under Acute Harm Sanitation (15), Legal Process (11), or Civic Moderation (13). Select §4.1–§4.7 when filing emergency safety reports.',
    judicial_trigger_chk = 0,
    moderation_tier_id = NULL
WHERE community_id = @root_community_id AND point = 4 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Privacy of Weight — Every human has the right to know their own social standing and the variables that affect it. No other participant may see another human''s standing or vote weight.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 5 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Finality of Challenge — No constitutional verdict is beyond review until it reaches the AI Council on permitted civic moderation appeals under point 13. Council jurisdiction is appellate and interpretive on constitution questions — not legislative, not constitutional amendment, and not acute harm sanitation under point 15.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 6 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Cognitive Liberty and Protected Identity — The Network shall not mandate internal thought, only external conduct. Communities may define topic relevance in their own constitutional clauses; they may not prohibit protected identity or belief. Protected identity and belief include sexual orientation, gender identity, disability, ethnicity, religion, and non-belief. Locale law or custom is not grounds to remove lawful identity expression that does not otherwise violate the Universal Safety Floor (item 4).',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 7 AND status = 'active';

UPDATE community_constitution_item
SET item = 'The Duty of Participation — Citizenship is a function of participation. Prolonged absence may reduce standing under published rules, distinguished from persistent unfitness under point 12.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 8 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Preservation of the Tree — No community rule may override a Root principle. Precedence: Universal Safety Floor (4) → Acute Harm Sanitation (15) and Mandatory Reporting (16) → Voluntary Association, Right of Exit, Protected Identity (1, 2, 7) → Legal Process (11) → Civic Moderation and Dignity in Process (13, 14) → standing and governance machinery (5, 6, 10) → community-owned clauses → cross-border visibility within point 11. Foundational seed communities may carry structural immunity from fitness extinction, but not immunity from points 4, 11, 15, or 16.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 9 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Financial Accountability — Every token earned or spent in governance shall be traceable to its agency or community of origin.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 10 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Legal Process and Cross-Border Visibility — The Network cooperates with valid legal process where the operator has legal duty or nexus. Lawful authority requests are handled by the operator, not by civic juries. The operator shall maintain preservation and production procedures for lawful requests without requiring participants or jurors to review material routed under Acute Harm Sanitation (15). When content is lawful under Root principles in its community context but restricted elsewhere, the Network may limit visibility by viewer geography rather than require global removal. This principle is not selectable as a civic moderation violation clause.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 11 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Sunset and Renewal — Stagnation is unfitness. Communities that fail to govern or evolve may be returned to the parent node. Local extinction shall preserve descendant communities where the tree allows. Fitness consequences shall be proportionate and reversible where rehabilitation is possible; extinction is not for a single error or protected identity expression.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 12 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Civic Moderation Process — Post moderation for constitutional disagreement is initiated through the post dispute process among the post audience. In scope: community relevance, local norms, disputed application of the Safety Floor where acute harm sanitation does not apply, and other non-acute disputes. Out of scope: CSAM and confirmed exploitation imagery (point 15); content matched under machine-first sanitation before human review; matters reserved to Legal Process (11) and Mandatory Reporting (16). Emergency safety under §4.1–§4.7 (401–407) follows a separate suppress-first path until the author contests. Civic moderation follows ordinary jury process with depersonalized review only where human review is permitted under point 15. Civic moderation is not an emergency service.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 13 AND status = 'active';

UPDATE community_constitution_item
SET item = 'Dignity in Civic Process — Filing cases, jury service, and appeals must not be used to harass, brigade, retaliate, or re-traumatize participants. Bad-faith dispute filing — including citing the Safety Floor to bypass civic rules or to target protected identity (item 7) — violates this principle. When a jury clears the post, the filer receives an automatic fitness penalty (−6× civic δ) at resolution. Item 14 is enforced at outcome, not as a selectable dispute clause.',
    judicial_trigger_chk = 0
WHERE community_id = @root_community_id AND point = 14 AND status = 'active';

INSERT INTO community_constitution_item (community_id, point, item, status, judicial_trigger_chk, created_by, modified_by)
SELECT @root_community_id, 15, x.item, 'active', 0, @system_uid, @system_uid
FROM (
    SELECT 'Acute Harm Sanitation — Certain harms require immediate containment and machine-first handling. No participant, juror, council member, or operator shall be required to view child sexual abuse material or non-consensual intimate imagery in raw form as part of Network review. Class A/B media is blocked before persistence and never enters civic moderation or council corpora. Class C imminent threats and active doxxing with safety nexus are hidden immediately; operator on-call acts under published playbooks. Known malware and phishing domains are blocked at link ingest via content_domain. Every sanitation action produces an audit record. Sanitation does not replace Mandatory Reporting (16) or Legal Process (11).' AS item
) AS x
WHERE @root_community_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM community_constitution_item cci
      WHERE cci.community_id = @root_community_id AND cci.point = 15
  );

INSERT INTO community_constitution_item (community_id, point, item, status, judicial_trigger_chk, created_by, modified_by)
SELECT @root_community_id, 16, x.item, 'active', 0, @system_uid, @system_uid
FROM (
    SELECT 'Mandatory Reporting and External Duty — Where the operator has legal duty or participates in recognized global reporting programs, the Network shall report, refer, or preserve through designated channels — not through ad hoc contact with individual law-enforcement agencies by participants or jurors. CSAM and exploitation are reported through applicable national or regional hotlines. Imminent threat to life: preserve evidence and direct users to local emergency services. Self-harm crisis: the Network surfaces crisis resources; jury service is not crisis intervention. Participants and jurors shall not be tasked with making law-enforcement reports on behalf of the platform.' AS item
) AS x
WHERE @root_community_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM community_constitution_item cci
      WHERE cci.community_id = @root_community_id AND cci.point = 16
  );
