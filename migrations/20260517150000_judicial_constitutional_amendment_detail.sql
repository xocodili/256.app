-- Proposal metadata for `judicial_case` rows with trigger `constitutional_amendment`.

CREATE TABLE IF NOT EXISTS judicial_case_constitutional_amendment (
    judicial_case_id BIGINT UNSIGNED NOT NULL COMMENT 'FK judicial_case.id',
    amendment_kind VARCHAR(32) NOT NULL COMMENT 'new_clause | amend_clause | remove_clause',
    target_constitution_item_id BIGINT UNSIGNED NULL COMMENT 'FK community_constitution_item.id — amend/remove target',
    proposed_body_plain NVARCHAR(4000) NOT NULL DEFAULT '',
    proposed_body_markup NVARCHAR(4000) NOT NULL DEFAULT '',
    proposed_body_html_sanitized NVARCHAR(4000) NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (judicial_case_id),
    KEY idx_jcca_target_item (target_constitution_item_id),
    CONSTRAINT chk_jcca_amendment_kind
        CHECK (amendment_kind IN ('new_clause', 'amend_clause', 'remove_clause')),
    CONSTRAINT fk_jcca_case
        FOREIGN KEY (judicial_case_id) REFERENCES judicial_case (id) ON DELETE CASCADE,
    CONSTRAINT fk_jcca_target_item
        FOREIGN KEY (target_constitution_item_id) REFERENCES community_constitution_item (id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
