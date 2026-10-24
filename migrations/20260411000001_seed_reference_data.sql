-- Seed `ref_key_values` for a greenfield deploy. Stable `key` slugs; human labels in locales/*/ref_values.*.
-- Runs after `20260411000000_schema.sql` (requires `ref_key_values` table).

INSERT INTO ref_key_values (`group`, `key`, sort_order, is_active, created_by, modified_by) VALUES
-- Shared lifecycle: users.status_id, social_entity.status_id, social_entity_relationship.status_id (group `status`)
('status', 'pending_verification', 10, 1, NULL, NULL),
('status', 'verified', 20, 1, NULL, NULL),
('status', 'retired', 30, 1, NULL, NULL),
('status', 'to_be_deleted', 40, 1, NULL, NULL),
('status', 'blocked', 50, 1, NULL, NULL),
('status', 'flagged', 60, 1, NULL, NULL),
-- social_entity_type (gathering → optional row in social_gathering)
('social_entity_type', 'human', 10, 1, NULL, NULL),
('social_entity_type', 'digital', 20, 1, NULL, NULL),
('social_entity_type', 'community', 30, 1, NULL, NULL),
('social_entity_type', 'non_profit', 40, 1, NULL, NULL),
('social_entity_type', 'profit', 50, 1, NULL, NULL),
('social_entity_type', 'gathering', 60, 1, NULL, NULL),
-- social_gathering_recurrence (`social_gathering.recurrence_frequency_id`; or weekly + recurrence_interval 2 ≡ fortnightly)
('social_gathering_recurrence', 'daily', 10, 1, NULL, NULL),
('social_gathering_recurrence', 'weekly', 20, 1, NULL, NULL),
('social_gathering_recurrence', 'fortnightly', 25, 1, NULL, NULL),
('social_gathering_recurrence', 'monthly', 30, 1, NULL, NULL),
('social_gathering_recurrence', 'yearly', 40, 1, NULL, NULL),
-- social_digital_type (`social_digital.digital_type_id`)
('social_digital_type', 'ai_agent', 10, 1, NULL, NULL),
('social_digital_type', 'bot', 20, 1, NULL, NULL),
('social_digital_type', 'service_account', 30, 1, NULL, NULL),
('social_digital_type', 'automation', 40, 1, NULL, NULL),
('social_digital_type', 'integration', 50, 1, NULL, NULL),
-- social_join_policy (extensions: gathering, human, community, digital, profit, non_profit)
('social_join_policy', 'open', 10, 1, NULL, NULL),
('social_join_policy', 'request_to_join', 20, 1, NULL, NULL),
('social_join_policy', 'invite_only', 30, 1, NULL, NULL),
('social_join_policy', 'closed', 40, 1, NULL, NULL),
-- social_entity_relationship_type (starter set — extend as product grows)
('social_entity_relationship_type', 'family', 10, 1, NULL, NULL),
('social_entity_relationship_type', 'friend', 20, 1, NULL, NULL),
('social_entity_relationship_type', 'works_at', 30, 1, NULL, NULL),
('social_entity_relationship_type', 'studied_at', 40, 1, NULL, NULL),
('social_entity_relationship_type', 'member_of', 50, 1, NULL, NULL),
('social_entity_relationship_type', 'legal_owner', 60, 1, NULL, NULL),
('social_entity_relationship_type', 'moderator', 70, 1, NULL, NULL),
('social_entity_relationship_type', 'godmother', 80, 1, NULL, NULL);
