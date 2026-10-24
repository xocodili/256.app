//! Council thread reply dossiers: eight unique root briefings for the reply LLM.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::engagement::{self, PostFeel};
use crate::social::feed;
use crate::social::thread::{self, CouncilThreadFull, CouncilThreadNodeRow};

const MAX_REPLY_ROOTS: i64 = 8;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CouncilThreadRootMeta {
    pub id: u64,
    pub body_plain: String,
    pub body_markup: String,
    pub handle: String,
    pub entity_rating: Decimal,
    pub published_at: DateTime<Utc>,
    pub post_status_key: Option<String>,
    pub audience_for_label: Option<String>,
    pub post_engagement_rating: Decimal,
    pub engagement_comment_count: i64,
    pub non_author_root_vote_count: i64,
    pub url_canonical: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_site_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IndexedThreadNode {
    pub id: u64,
    pub parent_post_id: Option<u64>,
    pub entity_id: u64,
    pub handle: String,
    pub body_plain: String,
    pub body_markup: String,
    pub post_rating: Decimal,
    pub published_at: DateTime<Utc>,
    pub depth: u8,
    pub child_count: u64,
}

#[derive(Debug, Clone)]
pub struct CouncilThreadDossier {
    pub root_post_id: u64,
    pub root: CouncilThreadRootMeta,
    pub nodes: Vec<IndexedThreadNode>,
}

/// Newest eligible thread roots in the council mesh (up to eight unique roots).
pub async fn load_reply_root_ids(
    pool: &MySqlPool,
    council_entity_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    let sql = r#"
        SELECT root_id
        FROM (
            SELECT
                COALESCE(p.root_post_id, p.id) AS root_id,
                MAX(p.published_at) AS latest_activity
            FROM post p
            INNER JOIN post_audience_entity pae
                ON pae.post_id = COALESCE(p.root_post_id, p.id)
            WHERE pae.audience_entity_id IN (
                SELECT ser.to_social_entity_id
                FROM social_entity_relationship ser
                INNER JOIN ref_key_values rk
                    ON rk.id = ser.type_id
                   AND rk.`group` = ?
                   AND rk.`key` IN (?, ?)
                   AND rk.is_active = 1
                WHERE ser.from_social_entity_id = ?
                  AND ser.ended_at IS NULL
            )
              AND p.is_moderation_mirror = 0
              AND p.published_at >= UTC_TIMESTAMP(6) - INTERVAL 7 DAY
              AND p.entity_id <> ?
              AND NOT EXISTS (
                  SELECT 1
                  FROM post root
                  INNER JOIN ref_key_values rk_root
                      ON rk_root.id = root.status_id
                     AND rk_root.`group` = ?
                     AND rk_root.`key` IN (?, ?, ?)
                  WHERE root.id = COALESCE(p.root_post_id, p.id)
              )
            GROUP BY root_id
            ORDER BY latest_activity DESC, root_id DESC
            LIMIT ?
        ) AS roots
        "#;
    sqlx::query_scalar(sql)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::KEY_RELATIONSHIP_MODERATOR)
        .bind(council_entity_id)
        .bind(council_entity_id)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_POST_SUPPRESSED)
        .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
        .bind(ref_keys::KEY_FLAGGED)
        .bind(MAX_REPLY_ROOTS)
        .fetch_all(pool)
        .await
}

async fn load_root_meta(pool: &MySqlPool, root_id: u64) -> Result<Option<CouncilThreadRootMeta>, sqlx::Error> {
    let post_id_sql = "p.id";
    let poster_entity_sql = "p.entity_id";
    let audience_sql = feed::sql_audience_label_subquery(post_id_sql, poster_entity_sql);
    let comment_count_sql = engagement::SQL_ENGAGEMENT_COMMENT_COUNT;
    let sql = format!(
        r#"
        SELECT
            p.id,
            p.body_plain,
            p.body_markup,
            se.handle,
            COALESCE(se.rating, 0.00) AS entity_rating,
            COALESCE(p.published_at, p.created_at) AS published_at,
            rk_post_status.`key` AS post_status_key,
            {audience_sql} AS audience_for_label,
            COALESCE(p.rating, 0.00) AS post_engagement_rating,
            {comment_count_sql} AS engagement_comment_count,
            (
                SELECT COUNT(*)
                FROM post_engagement_vote pev
                INNER JOIN post pr ON pr.id = pev.target_post_id
                WHERE pev.target_post_id = p.id
                  AND pev.voter_user_id <> pr.created_by
            ) AS non_author_root_vote_count,
            cl.url_canonical,
            cl.og_title,
            cl.og_description,
            cl.og_site_name
        FROM post p
        INNER JOIN social_entity se ON se.id = p.entity_id
        LEFT JOIN ref_key_values rk_post_status
            ON rk_post_status.id = p.status_id
           AND rk_post_status.`group` = ?
        LEFT JOIN content_link cl ON cl.id = p.content_link_id
        WHERE p.id = ?
          AND p.parent_post_id IS NULL
        LIMIT 1
        "#,
    );
    sqlx::query_as(&sql)
        .bind(ref_keys::GROUP_STATUS)
        .bind(root_id)
        .fetch_optional(pool)
        .await
}

fn index_thread_nodes(full: &CouncilThreadFull) -> Vec<IndexedThreadNode> {
    let mut by_id: HashMap<u64, &CouncilThreadNodeRow> = HashMap::new();
    for node in &full.nodes {
        by_id.insert(node.id, node);
    }

    let mut child_counts: HashMap<u64, u64> = HashMap::new();
    for node in &full.nodes {
        if let Some(parent_id) = node.parent_post_id {
            *child_counts.entry(parent_id).or_insert(0) += 1;
        }
    }

    let mut depths: HashMap<u64, u8> = HashMap::new();
    depths.insert(full.root_post_id, 0);

    let mut ordered: Vec<&CouncilThreadNodeRow> = full.nodes.iter().collect();
    ordered.sort_by_key(|n| (n.parent_post_id.is_none(), n.published_at, n.id));

    for node in ordered {
        if node.id == full.root_post_id {
            continue;
        }
        let depth = node
            .parent_post_id
            .and_then(|pid| depths.get(&pid).copied())
            .map(|d| d.saturating_add(1))
            .unwrap_or(1);
        depths.insert(node.id, depth);
    }

    full.nodes
        .iter()
        .map(|node| IndexedThreadNode {
            id: node.id,
            parent_post_id: node.parent_post_id,
            entity_id: node.entity_id,
            handle: node.handle.clone(),
            body_plain: node.body_plain.clone(),
            body_markup: node.body_markup.clone(),
            post_rating: node.post_rating,
            published_at: node.published_at,
            depth: *depths.get(&node.id).unwrap_or(&0),
            child_count: *child_counts.get(&node.id).unwrap_or(&0),
        })
        .collect()
}

/// One complete briefing for a thread root (metadata, link, engagement, full comment tree).
pub async fn assemble_council_thread_dossier(
    pool: &MySqlPool,
    root_id: u64,
) -> Result<Option<CouncilThreadDossier>, sqlx::Error> {
    let Some(root) = load_root_meta(pool, root_id).await? else {
        return Ok(None);
    };
    let Some(full) = thread::load_thread_full_for_council(pool, root_id).await? else {
        return Ok(None);
    };
    let nodes = index_thread_nodes(&full);
    Ok(Some(CouncilThreadDossier {
        root_post_id: root_id,
        root,
        nodes,
    }))
}

async fn recent_council_reply_parent_ids(
    pool: &MySqlPool,
    council_entity_id: u64,
) -> Result<HashSet<u64>, sqlx::Error> {
    let rows: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT r.parent_post_id
        FROM post r
        WHERE r.entity_id = ?
          AND r.parent_post_id IS NOT NULL
          AND r.is_moderation_mirror = 0
          AND r.published_at >= UTC_TIMESTAMP(6) - INTERVAL 1 DAY
        "#,
    )
    .bind(council_entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Post ids the councillor may reply under across the loaded dossiers.
pub fn collect_council_reply_parent_ids(
    council_entity_id: u64,
    dossiers: &[CouncilThreadDossier],
    recent_council_parents: &HashSet<u64>,
) -> Vec<u64> {
    let mut ids = Vec::new();
    for dossier in dossiers {
        for node in &dossier.nodes {
            if node.entity_id == council_entity_id {
                continue;
            }
            if recent_council_parents.contains(&node.id) {
                continue;
            }
            if node.depth > 2 {
                continue;
            }
            if !engagement::can_add_reply_at_depth(node.depth, node.child_count) {
                continue;
            }
            ids.push(node.id);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn xml_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn stars_and_feel(rating: Decimal) -> (f64, &'static str) {
    let stars = rating.to_string().parse::<f64>().unwrap_or(2.5);
    (stars, PostFeel::from_stars(stars).key())
}

fn serialize_dossier(dossier: &CouncilThreadDossier, replyable_ids: &HashSet<u64>) -> String {
    let (root_stars, root_feel) = stars_and_feel(dossier.root.post_engagement_rating);
    let audience = dossier
        .root
        .audience_for_label
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("@public");
    let status = dossier
        .root
        .post_status_key
        .as_deref()
        .unwrap_or("verified");
    let root_replyable = replyable_ids.contains(&dossier.root.id);

    let mut out = String::new();
    out.push_str(&format!(
        r#"<thread root="{root_id}">"#,
        root_id = dossier.root_post_id
    ));
    out.push('\n');
    if root_replyable {
        out.push_str(&format!(
            r#"  <root post_id="{id}" replyable="true" author="@{handle}" entity_rating="{rating}" published_at="{published}" audience="For: {audience}" status="{status}">"#,
            id = dossier.root.id,
            handle = xml_escape(&dossier.root.handle),
            rating = dossier.root.entity_rating,
            published = dossier.root.published_at.to_rfc3339(),
            audience = xml_escape(audience),
            status = xml_escape(status),
        ));
    } else {
        out.push_str(&format!(
            r#"  <root replyable="false" author="@{handle}" entity_rating="{rating}" published_at="{published}" audience="For: {audience}" status="{status}">"#,
            handle = xml_escape(&dossier.root.handle),
            rating = dossier.root.entity_rating,
            published = dossier.root.published_at.to_rfc3339(),
            audience = xml_escape(audience),
            status = xml_escape(status),
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "    <body_plain>{}</body_plain>\n",
        xml_escape(&dossier.root.body_plain)
    ));
    out.push_str(&format!(
        "    <body_markup>{}</body_markup>\n",
        xml_escape(&dossier.root.body_markup)
    ));

    if dossier.root.url_canonical.is_some()
        || dossier.root.og_title.is_some()
        || dossier.root.og_description.is_some()
        || dossier.root.og_site_name.is_some()
    {
        out.push_str(&format!(
            r#"    <link url="{url}" title="{title}" description="{desc}" site="{site}"/>"#,
            url = xml_escape(dossier.root.url_canonical.as_deref().unwrap_or("")),
            title = xml_escape(dossier.root.og_title.as_deref().unwrap_or("")),
            desc = xml_escape(dossier.root.og_description.as_deref().unwrap_or("")),
            site = xml_escape(dossier.root.og_site_name.as_deref().unwrap_or("")),
        ));
        out.push('\n');
    }

    out.push_str(&format!(
        r#"    <engagement stars="{stars:.2}" feel="{feel}" comment_count="{comments}" non_author_votes="{votes}"/>"#,
        stars = root_stars,
        feel = root_feel,
        comments = dossier.root.engagement_comment_count,
        votes = dossier.root.non_author_root_vote_count,
    ));
    out.push('\n');
    out.push_str("  </root>\n");

    let comments: Vec<&IndexedThreadNode> = dossier
        .nodes
        .iter()
        .filter(|n| n.id != dossier.root_post_id)
        .collect();
    if !comments.is_empty() {
        out.push_str("  <comments>\n");
        for node in comments {
            let (stars, feel) = stars_and_feel(node.post_rating);
            let replyable = replyable_ids.contains(&node.id);
            if replyable {
                let parent = node
                    .parent_post_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| dossier.root_post_id.to_string());
                out.push_str(&format!(
                    r#"    <comment post_id="{id}" parent_post_id="{parent}" depth="{depth}" replyable="true" author="@{handle}" published_at="{published}" stars="{stars:.2}" feel="{feel}">"#,
                    id = node.id,
                    parent = parent,
                    depth = node.depth,
                    handle = xml_escape(&node.handle),
                    published = node.published_at.to_rfc3339(),
                    stars = stars,
                    feel = feel,
                ));
            } else {
                out.push_str(&format!(
                    r#"    <comment replyable="false" depth="{depth}" author="@{handle}" published_at="{published}" stars="{stars:.2}" feel="{feel}">"#,
                    depth = node.depth,
                    handle = xml_escape(&node.handle),
                    published = node.published_at.to_rfc3339(),
                    stars = stars,
                    feel = feel,
                ));
            }
            out.push('\n');
            out.push_str(&format!(
                "      <body_plain>{}</body_plain>\n",
                xml_escape(&node.body_plain)
            ));
            out.push_str(&format!(
                "      <body_markup>{}</body_markup>\n",
                xml_escape(&node.body_markup)
            ));
            out.push_str("    </comment>\n");
        }
        out.push_str("  </comments>\n");
    }

    out.push_str("</thread>\n");
    out
}

/// Build the LLM thread document and flat parent-id list for one councillor tick.
pub async fn build_council_reply_briefing(
    pool: &MySqlPool,
    council_entity_id: u64,
) -> Result<(String, Vec<u64>), sqlx::Error> {
    let root_ids = load_reply_root_ids(pool, council_entity_id).await?;
    let recent_parents = recent_council_reply_parent_ids(pool, council_entity_id).await?;

    let mut dossiers = Vec::with_capacity(root_ids.len());
    for root_id in root_ids {
        if let Some(dossier) = assemble_council_thread_dossier(pool, root_id).await? {
            dossiers.push(dossier);
        }
    }

    let parent_ids = collect_council_reply_parent_ids(council_entity_id, &dossiers, &recent_parents);
    let replyable_ids: HashSet<u64> = parent_ids.iter().copied().collect();
    let document = dossiers
        .iter()
        .map(|d| serialize_dossier(d, &replyable_ids))
        .collect::<Vec<_>>()
        .join("\n");

    Ok((document, parent_ids))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dossier_serializer_includes_root_link_and_nested_reply() {
        let dossier = CouncilThreadDossier {
            root_post_id: 100,
            root: CouncilThreadRootMeta {
                id: 100,
                body_plain: "Root body full text".to_string(),
                body_markup: "<p>Root body full text</p>".to_string(),
                handle: "alice".to_string(),
                entity_rating: Decimal::new(250, 2),
                published_at: Utc::now(),
                post_status_key: Some("verified".to_string()),
                audience_for_label: Some("@public, @intellectual_life_education".to_string()),
                post_engagement_rating: Decimal::new(350, 2),
                engagement_comment_count: 1,
                non_author_root_vote_count: 2,
                url_canonical: Some("https://example.com/article".to_string()),
                og_title: Some("Example title".to_string()),
                og_description: Some("Example description".to_string()),
                og_site_name: Some("example.com".to_string()),
            },
            nodes: vec![
                IndexedThreadNode {
                    id: 100,
                    parent_post_id: None,
                    entity_id: 1,
                    handle: "alice".to_string(),
                    body_plain: "Root body full text".to_string(),
                    body_markup: "<p>Root body full text</p>".to_string(),
                    post_rating: Decimal::new(350, 2),
                    published_at: Utc::now(),
                    depth: 0,
                    child_count: 1,
                },
                IndexedThreadNode {
                    id: 101,
                    parent_post_id: Some(100),
                    entity_id: 2,
                    handle: "bob".to_string(),
                    body_plain: "Nested reply body".to_string(),
                    body_markup: "<p>Nested reply body</p>".to_string(),
                    post_rating: Decimal::new(250, 2),
                    published_at: Utc::now(),
                    depth: 1,
                    child_count: 0,
                },
            ],
        };

        let xml = serialize_dossier(&dossier, &HashSet::from([100, 101]));
        assert!(xml.contains("Root body full text"));
        assert!(xml.contains("https://example.com/article"));
        assert!(xml.contains("Example title"));
        assert!(xml.contains("Nested reply body"));
        assert!(xml.contains(r#"parent_post_id="100""#));
        assert!(xml.contains(r#"<thread root="100">"#));
    }

    #[test]
    fn candidate_parent_ids_only_from_loaded_dossiers() {
        let dossier = CouncilThreadDossier {
            root_post_id: 10,
            root: CouncilThreadRootMeta {
                id: 10,
                body_plain: "root".to_string(),
                body_markup: "<p>root</p>".to_string(),
                handle: "human".to_string(),
                entity_rating: Decimal::new(250, 2),
                published_at: Utc::now(),
                post_status_key: Some("verified".to_string()),
                audience_for_label: None,
                post_engagement_rating: Decimal::new(250, 2),
                engagement_comment_count: 0,
                non_author_root_vote_count: 0,
                url_canonical: None,
                og_title: None,
                og_description: None,
                og_site_name: None,
            },
            nodes: vec![IndexedThreadNode {
                id: 10,
                parent_post_id: None,
                entity_id: 1,
                handle: "human".to_string(),
                body_plain: "root".to_string(),
                body_markup: "<p>root</p>".to_string(),
                post_rating: Decimal::new(250, 2),
                published_at: Utc::now(),
                depth: 0,
                child_count: 0,
            }],
        };

        let ids = collect_council_reply_parent_ids(999, &[dossier], &HashSet::new());
        assert_eq!(ids, vec![10]);

        let foreign = collect_council_reply_parent_ids(999, &[], &HashSet::new());
        assert!(foreign.is_empty());
    }
}
