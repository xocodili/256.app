-- Death metadata for profit, non-profit, and gathering extension tables.

ALTER TABLE social_profit
    ADD COLUMN died_at TIMESTAMP(6) NULL COMMENT 'When the entity entered dead status' AFTER logo_content_link_id,
    ADD COLUMN death_source_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group graph_death_source'
        AFTER died_at;

ALTER TABLE social_profit
    ADD CONSTRAINT fk_social_profit_death_source
        FOREIGN KEY (death_source_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;

ALTER TABLE social_non_profit
    ADD COLUMN died_at TIMESTAMP(6) NULL COMMENT 'When the entity entered dead status' AFTER logo_content_link_id,
    ADD COLUMN death_source_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group graph_death_source'
        AFTER died_at;

ALTER TABLE social_non_profit
    ADD CONSTRAINT fk_social_non_profit_death_source
        FOREIGN KEY (death_source_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;

ALTER TABLE social_gathering
    ADD COLUMN died_at TIMESTAMP(6) NULL COMMENT 'When the entity entered dead status' AFTER event_portal_content_link_id,
    ADD COLUMN death_source_id BIGINT UNSIGNED NULL
        COMMENT 'FK ref_key_values.id, group graph_death_source'
        AFTER died_at;

ALTER TABLE social_gathering
    ADD CONSTRAINT fk_social_gathering_death_source
        FOREIGN KEY (death_source_id) REFERENCES ref_key_values (id) ON DELETE RESTRICT;
