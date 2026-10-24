-- Additive constitutional items per community (inheritance-ready).
-- Root seed: 12 principles for `operating_charter_256`.

CREATE TABLE IF NOT EXISTS community_constitution_item (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    community_id BIGINT UNSIGNED NOT NULL COMMENT 'FK social_community.social_entity_id',
    point SMALLINT UNSIGNED NOT NULL COMMENT 'Display order within this community constitution',
    item TEXT NOT NULL COMMENT 'Constitution principle/amendment text',
    status VARCHAR(32) NOT NULL DEFAULT 'active' COMMENT 'active|superseded|retired|draft',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by BIGINT UNSIGNED NULL,
    modified_by BIGINT UNSIGNED NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_community_constitution_point (community_id, point),
    KEY idx_community_constitution_status (community_id, status),
    CONSTRAINT chk_community_constitution_status
        CHECK (status IN ('active', 'superseded', 'retired', 'draft')),
    CONSTRAINT fk_community_constitution_community
        FOREIGN KEY (community_id) REFERENCES social_community (social_entity_id) ON DELETE CASCADE,
    CONSTRAINT fk_community_constitution_created_by
        FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE SET NULL,
    CONSTRAINT fk_community_constitution_modified_by
        FOREIGN KEY (modified_by) REFERENCES users (id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SET @system_uid := (SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1);
SET @root_community_id := (
    SELECT se.id
    FROM social_entity se
    INNER JOIN social_community sc ON sc.social_entity_id = se.id
    WHERE se.handle = 'operating_charter_256'
    LIMIT 1
);

INSERT INTO community_constitution_item (community_id, point, item, status, created_by, modified_by)
SELECT @root_community_id, x.point, x.item, 'active', @system_uid, @system_uid
FROM (
    SELECT 1 AS point, 'The Principle of Voluntary Association — Every community is an opt-in collective. No Human shall be bound to a community without explicit consent, save for the Root itself, which constitutes the shared reality of the Network.' AS item
    UNION ALL
    SELECT 2, 'The Right of Exit (Secession) — The path of departure must remain as frictionless as the path of entry. Any community may migrate or dissolve, provided it fulfills its current governance quorums.'
    UNION ALL
    SELECT 3, 'The Integrity of the Vote — The Ballot is sacrosanct. Any attempt to spoof, bot, or bypass the sortition mechanism is a strike against the Root. The AI Council exists to protect the vote, not the voter.'
    UNION ALL
    SELECT 4, 'Prohibition of Malice and Deception — Communication that facilitates physical harm, exploitation, scams, harassment, bullying, or the distribution of illegal material is inherently incompatible with the Network’s existence.'
    UNION ALL
    SELECT 5, 'The Privacy of Weight — Every Human has a right to know their own Social Standing Rating and the variables used to calculate their Predictive Wisdom. To prevent the formation of cults or political hierarchies, a Human''s social status and weight are strictly invisible to all other participants.'
    UNION ALL
    SELECT 6, 'The Finality of Challenge — No decision is beyond review until it reaches the AI Council. Participating in the site requires submission to their ultimate collective decisions. These interventions consume resources; the Right of Challenge must be exercised with financial wisdom.'
    UNION ALL
    SELECT 7, 'Cognitive Liberty — The Network shall not attempt to mandate internal thought, only external conduct. Communities may define Topic Relevance, but they may not prohibit Internal Belief.'
    UNION ALL
    SELECT 8, 'The Duty of Participation — Citizenship is a function of participation. Prolonged apathy leads to the expiration of influence. To inhabit the tree is to help maintain its branches.'
    UNION ALL
    SELECT 9, 'Preservation of the Tree — No community rule may override a Root principle. The Tree’s structural logic (Inheritance) is the final arbiter of community fitness.'
    UNION ALL
    SELECT 10, 'Financial Accountability — Every Token earned or taxed must be traceable to its Agency or Community of origin. The Treasury is an open ledger.'
    UNION ALL
    SELECT 11, 'External Jurisdictional Compliance — A Human''s biological identity is separate from their Network identity. However, the system acknowledges local Law jurisdictions and legal requirements. The Network will cooperate with local authorities as required by the physical geography of the participant.'
    UNION ALL
    SELECT 12, 'The Sunset Clause (Auto-Purge) — Stagnation is death. Communities that fail to govern, fail to reach quorum, or fail to evolve are returned to the Parent node. Entropy is the fuel in the engine of creation.'
) AS x
WHERE @root_community_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1
      FROM community_constitution_item cci
      WHERE cci.community_id = @root_community_id
        AND cci.point = x.point
  );
