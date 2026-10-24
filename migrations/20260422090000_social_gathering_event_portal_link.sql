ALTER TABLE social_gathering
    ADD COLUMN event_portal_content_link_id BIGINT UNSIGNED NULL
        COMMENT 'Optional external event portal URL as canonical content_link.id',
    ADD KEY idx_social_gathering_event_portal_content_link (event_portal_content_link_id),
    ADD CONSTRAINT fk_social_gathering_event_portal_content_link
        FOREIGN KEY (event_portal_content_link_id) REFERENCES content_link (id) ON DELETE SET NULL;
