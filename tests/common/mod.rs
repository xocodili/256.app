//! Test infrastructure: throwaway MySQL database with seed helpers.
//! Migrations are applied via `sqlx::migrate!` on each fresh database (includes ref_key_values seeds).

pub mod api;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use std::collections::HashMap;

const SEED_RNG: u64 = 256_256;

pub struct TestDb {
    pub pool: MySqlPool,
    admin_pool: MySqlPool,
    db_name: String,
}

impl TestDb {
    pub async fn new() -> Self {
        dotenvy::dotenv().ok();

        let base_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            let db_url = std::env::var("DATABASE_URL")
                .expect("TEST_DATABASE_URL or DATABASE_URL must be set");
            // Strip trailing database name: mysql://user:pass@host:port/dbname → mysql://user:pass@host:port
            match db_url.rfind('/') {
                Some(pos) if pos > db_url.find("://").map(|p| p + 2).unwrap_or(0) => {
                    db_url[..pos].to_string()
                }
                _ => db_url,
            }
        });

        let db_name = format!("test_256_{}", uuid::Uuid::new_v4().simple());

        let admin_pool = MySqlPoolOptions::new()
            .max_connections(2)
            .connect(&base_url)
            .await
            .expect("connect to MySQL server");

        sqlx::query(&format!(
            "CREATE DATABASE `{db_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        ))
        .execute(&admin_pool)
        .await
        .expect("create test database");

        let db_url = format!("{base_url}/{db_name}");
        let pool = MySqlPoolOptions::new()
            .max_connections(8)
            .connect(&db_url)
            .await
            .expect("connect to test database");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");

        Self {
            pool,
            admin_pool,
            db_name,
        }
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", self.db_name))
            .execute(&self.admin_pool)
            .await;
        self.admin_pool.close().await;
    }
}

/// Cache of ref_key_values ids keyed by (group, key).
pub struct RefKeys {
    map: HashMap<(String, String), u64>,
}

impl RefKeys {
    pub async fn load(pool: &MySqlPool) -> Self {
        let rows: Vec<(u64, String, String)> = sqlx::query_as(
            "SELECT id, `group`, `key` FROM ref_key_values WHERE is_active = 1",
        )
        .fetch_all(pool)
        .await
        .expect("load ref_key_values");

        let mut map = HashMap::with_capacity(rows.len());
        for (id, group, key) in rows {
            map.insert((group, key), id);
        }
        Self { map }
    }

    pub fn id(&self, group: &str, key: &str) -> u64 {
        *self
            .map
            .get(&(group.to_string(), key.to_string()))
            .unwrap_or_else(|| panic!("ref_key_values not found: ({group}, {key})"))
    }
}

pub struct SeedIds {
    pub user_ids: Vec<u64>,
    pub entity_ids: Vec<u64>,
    pub community_entity_ids: Vec<u64>,
    pub user_community_entity_ids: Vec<u64>,
    pub profit_entity_ids: Vec<u64>,
    pub non_profit_entity_ids: Vec<u64>,
    pub post_community_ids: Vec<u64>,
    pub post_private_ids: Vec<u64>,
}

/// Smaller graph for focused judicial integration tests (~128 users).
pub async fn seed_light(pool: &MySqlPool) -> SeedIds {
    seed_graph(pool, 128).await
}

/// Full simulation graph (4096 users).
pub async fn seed_all(pool: &MySqlPool) -> SeedIds {
    seed_graph(pool, 4096).await
}

async fn seed_graph(pool: &MySqlPool, user_count: usize) -> SeedIds {
    let rk = RefKeys::load(pool).await;
    let mut rng = StdRng::seed_from_u64(SEED_RNG);

    let location_id = get_or_create_location(pool).await;
    let user_ids = seed_users(pool, &rk, user_count).await;
    let entity_ids = seed_entities(pool, &rk, location_id, &user_ids, &mut rng).await;
    seed_humans(pool, &rk, location_id, &entity_ids).await;
    seed_owner_roles(pool, &rk, &user_ids, &entity_ids).await;

    let community_entity_ids = get_council_community_ids(pool).await;
    seed_community_memberships(pool, &rk, &entity_ids, &community_entity_ids, &mut rng).await;

    let user_community_entity_ids =
        seed_user_communities(pool, &rk, &entity_ids, &community_entity_ids, location_id, &user_ids, &mut rng).await;
    let profit_entity_ids =
        seed_profits(pool, &rk, &entity_ids, location_id, &user_ids, &mut rng).await;
    let non_profit_entity_ids =
        seed_non_profits(pool, &rk, &entity_ids, location_id, &user_ids, &mut rng).await;
    seed_follows_and_joins(
        pool, &rk, &entity_ids, &user_community_entity_ids,
        &profit_entity_ids, &non_profit_entity_ids, &mut rng,
    ).await;

    seed_constitution_items(pool, &community_entity_ids).await;
    seed_friendships(pool, &rk, &entity_ids, &mut rng).await;

    let verified_status_id = rk.id("status", "verified");
    let post_community_ids =
        seed_community_posts(pool, &entity_ids, &user_ids, &community_entity_ids, verified_status_id).await;
    let post_private_ids =
        seed_private_posts(pool, &entity_ids, &user_ids, verified_status_id).await;

    seed_noms(pool, &entity_ids, 3).await;

    SeedIds {
        user_ids,
        entity_ids,
        community_entity_ids,
        user_community_entity_ids,
        profit_entity_ids,
        non_profit_entity_ids,
        post_community_ids,
        post_private_ids,
    }
}

async fn get_or_create_location(pool: &MySqlPool) -> u64 {
    let existing: Option<u64> =
        sqlx::query_scalar("SELECT id FROM locations ORDER BY id ASC LIMIT 1")
            .fetch_optional(pool)
            .await
            .expect("query locations");
    if let Some(id) = existing {
        return id;
    }
    let lifecycle_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'location_lifecycle' AND `key` = 'active' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("location_lifecycle active");

    let result = sqlx::query(
        r#"INSERT INTO locations (country_code, locality, display_label, validation_source, validation_ref, canonical_fingerprint, location_lifecycle_id)
           VALUES ('ZZ', 'Test City', 'Test City', 'seed', 'test-harness', UNHEX(SHA2('test-harness-loc', 256)), ?)"#,
    )
    .bind(lifecycle_id)
    .execute(pool)
    .await
    .expect("insert location");

    result.last_insert_id()
}

async fn seed_users(pool: &MySqlPool, rk: &RefKeys, count: usize) -> Vec<u64> {
    let status_id = rk.id("status", "verified");
    let user_status_id = rk.id("user_status", "verified");
    let user_type_id = rk.id("user_type", "human");
    let tier_id = rk.id("verification_tier", "basic");

    let batch_size = 500;
    for chunk_start in (0..count).step_by(batch_size) {
        let chunk_end = (chunk_start + batch_size).min(count);
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO users (email, password_hash, email_verified_at, status_id, user_status_id, user_type_id, verification_tier_id) ",
        );
        qb.push_values(chunk_start..chunk_end, |mut b, i| {
            b.push_bind(format!("test.{i:04}@256.test"));
            b.push_bind(None::<String>);
            b.push("UTC_TIMESTAMP()");
            b.push_bind(status_id);
            b.push_bind(user_status_id);
            b.push_bind(user_type_id);
            b.push_bind(tier_id);
        });
        qb.build().execute(pool).await.expect("insert users batch");
    }

    let rows: Vec<u64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE email LIKE 'test.%@256.test' ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .expect("fetch user ids");

    assert_eq!(rows.len(), count, "expected {count} test users");
    rows
}

async fn seed_entities(
    pool: &MySqlPool,
    rk: &RefKeys,
    location_id: u64,
    user_ids: &[u64],
    rng: &mut StdRng,
) -> Vec<u64> {
    let type_id = rk.id("social_entity_type", "human");
    let status_id = rk.id("status", "verified");
    let entity_status_id = rk.id("social_entity_status", "verified");

    let batch_size = 500;
    for (chunk_idx, chunk) in user_ids.chunks(batch_size).enumerate() {
        let chunk_offset = chunk_idx * batch_size;
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by) ",
        );
        qb.push_values(chunk.iter().enumerate(), |mut b, (i_in_chunk, &uid)| {
            let global_idx = chunk_offset + i_in_chunk;
            let handle = format!("test_{global_idx:04}");
            let rating: f64 = 1.50 + rng.gen::<f64>() * 2.0; // 1.50 - 3.50
            let rating_str = format!("{rating:.2}");
            b.push_bind(handle);
            b.push_bind(type_id);
            b.push_bind(status_id);
            b.push_bind(entity_status_id);
            b.push_bind(location_id);
            b.push_bind(rating_str);
            b.push_bind(uid);
            b.push_bind(uid);
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert entities batch");
    }

    let rows: Vec<u64> = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle LIKE 'test\\_%' AND type_id = ? ORDER BY id ASC",
    )
    .bind(type_id)
    .fetch_all(pool)
    .await
    .expect("fetch entity ids");

    assert_eq!(rows.len(), user_ids.len(), "entity count mismatch");
    rows
}

async fn seed_humans(
    pool: &MySqlPool,
    rk: &RefKeys,
    location_id: u64,
    entity_ids: &[u64],
) {
    let join_policy_id = rk.id("social_join_policy", "open");

    let first_names = [
        "Ava", "Liam", "Mila", "Noah", "Zoe", "Ethan", "Nora", "Leo", "Ruby", "Mason", "Ivy",
        "Felix", "Luna", "Owen", "Aria", "Kai",
    ];
    let last_names = [
        "Parker", "Nguyen", "Patel", "Rivera", "Brooks", "Kim", "Ali", "Morris", "Santos",
        "Reed", "Bennett", "Khan", "Shaw", "Wright", "Coleman", "Dawson",
    ];

    let batch_size = 500;
    for chunk_start in (0..entity_ids.len()).step_by(batch_size) {
        let chunk_end = (chunk_start + batch_size).min(entity_ids.len());
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO social_human (social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id) ",
        );
        qb.push_values(chunk_start..chunk_end, |mut b, i| {
            let given = first_names[i % first_names.len()];
            let family = last_names[i % last_names.len()];
            let birth = format!("1980-01-{:02}", (i % 28) + 1);
            b.push_bind(entity_ids[i]);
            b.push_bind(family.to_string());
            b.push_bind(given.to_string());
            b.push_bind(birth);
            b.push_bind(location_id);
            b.push_bind(join_policy_id);
        });
        qb.build().execute(pool).await.expect("insert humans batch");
    }
}

async fn seed_owner_roles(pool: &MySqlPool, rk: &RefKeys, user_ids: &[u64], entity_ids: &[u64]) {
    let role_id = rk.id("user_social_entity_role", "owner");

    let batch_size = 500;
    for chunk_start in (0..user_ids.len()).step_by(batch_size) {
        let chunk_end = (chunk_start + batch_size).min(user_ids.len());
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at) ",
        );
        qb.push_values(chunk_start..chunk_end, |mut b, i| {
            b.push_bind(user_ids[i]);
            b.push_bind(entity_ids[i]);
            b.push_bind(role_id);
            b.push_bind(1u8);
            b.push("UTC_TIMESTAMP()");
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert owner roles batch");
    }
}

async fn get_council_community_ids(pool: &MySqlPool) -> Vec<u64> {
    let rows: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT sc.social_entity_id
        FROM social_community sc
        INNER JOIN social_entity se ON se.id = sc.social_entity_id
        INNER JOIN ref_key_values rk ON rk.id = se.type_id
            AND rk.`group` = 'social_entity_type'
            AND rk.`key` = 'community'
        WHERE se.handle NOT IN ('operating_charter_256', '256app')
        ORDER BY sc.social_entity_id ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .expect("fetch council communities");

    assert!(
        rows.len() >= 12,
        "expected at least 12 council communities, got {}",
        rows.len()
    );
    rows
}

async fn seed_community_memberships(
    pool: &MySqlPool,
    rk: &RefKeys,
    entity_ids: &[u64],
    community_ids: &[u64],
    rng: &mut StdRng,
) {
    let type_id = rk.id("social_entity_relationship_type", "member_of");
    let status_id = rk.id("status", "verified");
    let rel_status_id = rk.id("social_relationship_status", "verified");

    let batch_size = 500;
    let mut pairs: Vec<(u64, u64, String)> = Vec::with_capacity(entity_ids.len());

    for &eid in entity_ids {
        let comm_idx = (eid as usize) % community_ids.len();
        let cid = community_ids[comm_idx];
        let rating: f64 = 1.50 + rng.gen::<f64>() * 2.0;
        pairs.push((eid, cid, format!("{rating:.2}")));
    }

    for chunk in pairs.chunks(batch_size) {
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT IGNORE INTO social_entity_relationship (from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, rating, started_at) ",
        );
        qb.push_values(chunk, |mut b, (eid, cid, rating)| {
            b.push_bind(*eid);
            b.push_bind(*cid);
            b.push_bind(type_id);
            b.push_bind(status_id);
            b.push_bind(rel_status_id);
            b.push_bind(rating.as_str());
            b.push("UTC_TIMESTAMP()");
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert memberships batch");
    }
}

async fn seed_user_communities(
    pool: &MySqlPool,
    rk: &RefKeys,
    entity_ids: &[u64],
    council_community_ids: &[u64],
    location_id: u64,
    user_ids: &[u64],
    rng: &mut StdRng,
) -> Vec<u64> {
    let community_type_id = rk.id("social_entity_type", "community");
    let status_id = rk.id("status", "verified");
    let entity_status_id = rk.id("social_entity_status", "verified");
    let join_open = rk.id("social_join_policy", "open");
    let join_reviewed = rk.id("social_join_policy", "reviewed");
    let member_of_type = rk.id("social_entity_relationship_type", "member_of");
    let moderator_type = rk.id("social_entity_relationship_type", "moderator");
    let rel_status_id = rk.id("social_relationship_status", "verified");

    let mut created_ids: Vec<u64> = Vec::new();

    for (i, &eid) in entity_ids.iter().enumerate() {
        if rng.gen::<f64>() >= 0.03 {
            continue;
        }

        let handle = format!("comm_usr_{i:04}");
        let parent_idx = (eid as usize) % council_community_ids.len();
        let parent_id = council_community_ids[parent_idx];
        let join_policy = if rng.gen_bool(0.5) { join_open } else { join_reviewed };

        let result = sqlx::query(
            r#"INSERT INTO social_entity
               (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
               VALUES (?, ?, ?, ?, ?, '2.50', ?, ?)"#,
        )
        .bind(&handle)
        .bind(community_type_id)
        .bind(status_id)
        .bind(entity_status_id)
        .bind(location_id)
        .bind(user_ids[i])
        .bind(user_ids[i])
        .execute(pool)
        .await
        .expect("insert community entity");

        let comm_eid = result.last_insert_id();

        sqlx::query(
            r#"INSERT INTO social_community
               (social_entity_id, label, description, intention, rules, founded_at, join_policy_id, parent_community_id)
               VALUES (?, ?, ?, 'User-created test community', 'Be respectful.', CURDATE(), ?, ?)"#,
        )
        .bind(comm_eid)
        .bind(format!("Community by User {i}"))
        .bind(format!("A test community created by user {i}"))
        .bind(join_policy)
        .bind(parent_id)
        .execute(pool)
        .await
        .expect("insert social_community");

        // Creator gets moderator + member_of edges
        let rating: f64 = 1.50 + rng.gen::<f64>() * 2.0;
        let rating_str = format!("{rating:.2}");

        for &rel_type in &[moderator_type, member_of_type] {
            sqlx::query(
                r#"INSERT IGNORE INTO social_entity_relationship
                   (from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, rating, started_at)
                   VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP())"#,
            )
            .bind(eid)
            .bind(comm_eid)
            .bind(rel_type)
            .bind(status_id)
            .bind(rel_status_id)
            .bind(&rating_str)
            .execute(pool)
            .await
            .expect("insert community creator edge");
        }

        created_ids.push(comm_eid);
    }

    created_ids
}

async fn seed_profits(
    pool: &MySqlPool,
    rk: &RefKeys,
    entity_ids: &[u64],
    location_id: u64,
    user_ids: &[u64],
    rng: &mut StdRng,
) -> Vec<u64> {
    let profit_type_id = rk.id("social_entity_type", "profit");
    let status_id = rk.id("status", "verified");
    let entity_status_id = rk.id("social_entity_status", "verified");
    let join_open = rk.id("social_join_policy", "open");

    let mut created_ids: Vec<u64> = Vec::new();

    for (i, &_eid) in entity_ids.iter().enumerate() {
        if rng.gen::<f64>() >= 0.03 {
            continue;
        }

        let handle = format!("profit_usr_{i:04}");

        let result = sqlx::query(
            r#"INSERT INTO social_entity
               (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
               VALUES (?, ?, ?, ?, ?, '2.50', ?, ?)"#,
        )
        .bind(&handle)
        .bind(profit_type_id)
        .bind(status_id)
        .bind(entity_status_id)
        .bind(location_id)
        .bind(user_ids[i])
        .bind(user_ids[i])
        .execute(pool)
        .await
        .expect("insert profit entity");

        let profit_eid = result.last_insert_id();

        sqlx::query(
            r#"INSERT INTO social_profit
               (social_entity_id, join_policy_id, label, description, intention)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(profit_eid)
        .bind(join_open)
        .bind(format!("Test Profit {i}"))
        .bind(format!("A test profit entity created by user {i}"))
        .bind("Commercial venture for testing")
        .execute(pool)
        .await
        .expect("insert social_profit");

        created_ids.push(profit_eid);
    }

    created_ids
}

async fn seed_non_profits(
    pool: &MySqlPool,
    rk: &RefKeys,
    entity_ids: &[u64],
    location_id: u64,
    user_ids: &[u64],
    rng: &mut StdRng,
) -> Vec<u64> {
    let non_profit_type_id = rk.id("social_entity_type", "non_profit");
    let status_id = rk.id("status", "verified");
    let entity_status_id = rk.id("social_entity_status", "verified");
    let join_open = rk.id("social_join_policy", "open");

    let mut created_ids: Vec<u64> = Vec::new();

    for (i, &_eid) in entity_ids.iter().enumerate() {
        if rng.gen::<f64>() >= 0.03 {
            continue;
        }

        let handle = format!("npo_usr_{i:04}");

        let result = sqlx::query(
            r#"INSERT INTO social_entity
               (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
               VALUES (?, ?, ?, ?, ?, '2.50', ?, ?)"#,
        )
        .bind(&handle)
        .bind(non_profit_type_id)
        .bind(status_id)
        .bind(entity_status_id)
        .bind(location_id)
        .bind(user_ids[i])
        .bind(user_ids[i])
        .execute(pool)
        .await
        .expect("insert non-profit entity");

        let npo_eid = result.last_insert_id();

        sqlx::query(
            r#"INSERT INTO social_non_profit
               (social_entity_id, join_policy_id, label, description, intention)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(npo_eid)
        .bind(join_open)
        .bind(format!("Test Non-Profit {i}"))
        .bind(format!("A test non-profit entity created by user {i}"))
        .bind("Charitable mission for testing")
        .execute(pool)
        .await
        .expect("insert social_non_profit");

        created_ids.push(npo_eid);
    }

    created_ids
}

async fn seed_follows_and_joins(
    pool: &MySqlPool,
    rk: &RefKeys,
    entity_ids: &[u64],
    user_community_ids: &[u64],
    profit_ids: &[u64],
    non_profit_ids: &[u64],
    rng: &mut StdRng,
) {
    let follow_type = rk.id("social_entity_relationship_type", "follow");
    let member_of_type = rk.id("social_entity_relationship_type", "member_of");
    let status_id = rk.id("status", "verified");
    let rel_status_id = rk.id("social_relationship_status", "verified");

    let followable: Vec<u64> = profit_ids.iter().chain(non_profit_ids.iter()).copied().collect();

    for &eid in entity_ids {
        // Independent 3% roll: follow a random profit/non-profit
        if !followable.is_empty() && rng.gen::<f64>() < 0.03 {
            let target = followable[rng.gen_range(0..followable.len())];
            sqlx::query(
                r#"INSERT IGNORE INTO social_entity_relationship
                   (from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, started_at)
                   VALUES (?, ?, ?, ?, ?, UTC_TIMESTAMP())"#,
            )
            .bind(eid)
            .bind(target)
            .bind(follow_type)
            .bind(status_id)
            .bind(rel_status_id)
            .execute(pool)
            .await
            .expect("insert follow edge");
        }

        // Independent 3% roll: join a random user-created community
        if !user_community_ids.is_empty() && rng.gen::<f64>() < 0.03 {
            let target = user_community_ids[rng.gen_range(0..user_community_ids.len())];
            let rating: f64 = 1.50 + rng.gen::<f64>() * 2.0;
            let rating_str = format!("{rating:.2}");
            sqlx::query(
                r#"INSERT IGNORE INTO social_entity_relationship
                   (from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, rating, started_at)
                   VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP())"#,
            )
            .bind(eid)
            .bind(target)
            .bind(member_of_type)
            .bind(status_id)
            .bind(rel_status_id)
            .bind(&rating_str)
            .execute(pool)
            .await
            .expect("insert community join edge");
        }
    }
}

async fn seed_constitution_items(pool: &MySqlPool, community_ids: &[u64]) {
    let clauses = [
        "Members must engage respectfully and in good faith.",
        "Content must be relevant to the community's stated purpose.",
        "Harassment, abuse, or deceptive conduct is prohibited.",
    ];

    for &cid in community_ids {
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM community_constitution_item WHERE community_id = ?",
        )
        .bind(cid)
        .fetch_one(pool)
        .await
        .expect("count constitution items");

        if existing > 0 {
            continue;
        }

        for (i, clause) in clauses.iter().enumerate() {
            sqlx::query(
                "INSERT INTO community_constitution_item (community_id, point, item, status) VALUES (?, ?, ?, 'active')",
            )
            .bind(cid)
            .bind((i + 1) as u16)
            .bind(*clause)
            .execute(pool)
            .await
            .expect("insert constitution item");
        }
    }
}

async fn seed_friendships(
    pool: &MySqlPool,
    rk: &RefKeys,
    entity_ids: &[u64],
    rng: &mut StdRng,
) {
    let friend_type_id = rk.id("social_entity_relationship_type", "friend");
    let family_type_id = rk.id("social_entity_relationship_type", "family");
    let status_id = rk.id("status", "verified");
    let rel_status_id = rk.id("social_relationship_status", "verified");

    let n = entity_ids.len();
    let mut pairs: Vec<(u64, u64, u64)> = Vec::with_capacity(n * 20);

    for (i, &eid) in entity_ids.iter().enumerate() {
        let friend_count = 15 + (i % 6); // 15-20 friends
        let family_count = 3 + (i % 3); // 3-5 family

        for _ in 0..friend_count {
            let j = rng.gen_range(0..n);
            if j != i {
                pairs.push((eid, entity_ids[j], friend_type_id));
            }
        }
        for _ in 0..family_count {
            let j = rng.gen_range(0..n);
            if j != i {
                pairs.push((eid, entity_ids[j], family_type_id));
            }
        }
    }

    let batch_size = 500;
    for chunk in pairs.chunks(batch_size) {
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT IGNORE INTO social_entity_relationship (from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, started_at) ",
        );
        qb.push_values(chunk, |mut b, (from, to, tid)| {
            b.push_bind(*from);
            b.push_bind(*to);
            b.push_bind(*tid);
            b.push_bind(status_id);
            b.push_bind(rel_status_id);
            b.push("UTC_TIMESTAMP()");
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert friendships batch");
    }
}

async fn seed_community_posts(
    pool: &MySqlPool,
    entity_ids: &[u64],
    user_ids: &[u64],
    community_ids: &[u64],
    verified_status_id: u64,
) -> Vec<u64> {
    let batch_size = 500;
    for chunk_start in (0..entity_ids.len()).step_by(batch_size) {
        let chunk_end = (chunk_start + batch_size).min(entity_ids.len());
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO post (entity_id, content_link_id, status_id, body_plain, body_markup, body_html_sanitized, published_at, created_by, modified_by) ",
        );
        qb.push_values(chunk_start..chunk_end, |mut b, i| {
            b.push_bind(entity_ids[i]);
            b.push_bind(None::<u64>);
            b.push_bind(verified_status_id);
            b.push_bind(format!("Community post by test user {i}"));
            b.push_bind(format!("Community post by test user {i}"));
            b.push_bind(format!("<p>Community post by test user {i}</p>"));
            b.push("UTC_TIMESTAMP(6)");
            b.push_bind(user_ids[i]);
            b.push_bind(user_ids[i]);
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert community posts batch");
    }

    let post_ids: Vec<u64> = sqlx::query_scalar(
        "SELECT id FROM post WHERE body_plain LIKE 'Community post by test user%' ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .expect("fetch community post ids");

    // Wire audience: each post targets the author's community
    let batch_size = 500;
    for chunk_start in (0..post_ids.len()).step_by(batch_size) {
        let chunk_end = (chunk_start + batch_size).min(post_ids.len());
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO post_audience_entity (post_id, audience_entity_id) ",
        );
        qb.push_values(chunk_start..chunk_end, |mut b, i| {
            let comm_idx = (entity_ids[i] as usize) % community_ids.len();
            b.push_bind(post_ids[i]);
            b.push_bind(community_ids[comm_idx]);
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert post audience entity batch");
    }

    post_ids
}

async fn seed_private_posts(
    pool: &MySqlPool,
    entity_ids: &[u64],
    user_ids: &[u64],
    verified_status_id: u64,
) -> Vec<u64> {
    let batch_size = 500;
    for chunk_start in (0..entity_ids.len()).step_by(batch_size) {
        let chunk_end = (chunk_start + batch_size).min(entity_ids.len());
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO post (entity_id, content_link_id, status_id, body_plain, body_markup, body_html_sanitized, published_at, created_by, modified_by) ",
        );
        qb.push_values(chunk_start..chunk_end, |mut b, i| {
            b.push_bind(entity_ids[i]);
            b.push_bind(None::<u64>);
            b.push_bind(verified_status_id);
            b.push_bind(format!("Private post by test user {i}"));
            b.push_bind(format!("Private post by test user {i}"));
            b.push_bind(format!("<p>Private post by test user {i}</p>"));
            b.push("UTC_TIMESTAMP(6)");
            b.push_bind(user_ids[i]);
            b.push_bind(user_ids[i]);
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert private posts batch");
    }

    let post_ids: Vec<u64> = sqlx::query_scalar(
        "SELECT id FROM post WHERE body_plain LIKE 'Private post by test user%' ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .expect("fetch private post ids");

    // Wire audience scope: friends + family
    for chunk in post_ids.chunks(batch_size) {
        let mut qb =
            sqlx::QueryBuilder::new("INSERT INTO post_audience_scope (post_id, scope_key) ");
        let mut values: Vec<(u64, &str)> = Vec::with_capacity(chunk.len() * 2);
        for &pid in chunk {
            values.push((pid, "friends"));
            values.push((pid, "family"));
        }
        qb.push_values(&values, |mut b, (pid, scope)| {
            b.push_bind(*pid);
            b.push_bind(*scope);
        });
        qb.build()
            .execute(pool)
            .await
            .expect("insert post audience scope batch");
    }

    post_ids
}

async fn seed_noms(pool: &MySqlPool, entity_ids: &[u64], amount: u16) {
    let batch_size = 500;
    for chunk in entity_ids.chunks(batch_size) {
        let mut qb = sqlx::QueryBuilder::new("UPDATE social_human SET ai_tokens = ");
        qb.push_bind(amount);
        qb.push(" WHERE social_entity_id IN (");
        let mut sep = qb.separated(", ");
        for &eid in chunk {
            sep.push_bind(eid);
        }
        qb.push(")");
        qb.build().execute(pool).await.expect("update noms batch");
    }
}

// ---- Post engagement / thread test helpers ----

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EngagementSimStats {
    pub users_processed: u32,
    pub l1: u32,
    pub l2: u32,
    pub l3: u32,
}

pub const ENGAGEMENT_SIM_L2_CHANCE: f64 = 0.64;
pub const ENGAGEMENT_SIM_L3_CHANCE: f64 = 0.08;

/// Pick a random author whose community-scoped post is visible to `viewer_idx` (shared council community bucket).
pub fn pick_shared_community_author_idx(
    seed: &SeedIds,
    viewer_idx: usize,
    rng: &mut StdRng,
) -> Option<usize> {
    let comm_count = seed.community_entity_ids.len();
    if comm_count == 0 || viewer_idx >= seed.entity_ids.len() {
        return None;
    }
    let viewer_bucket = (seed.entity_ids[viewer_idx] as usize) % comm_count;
    let candidates: Vec<usize> = (0..seed.entity_ids.len())
        .filter(|&j| {
            j != viewer_idx && (seed.entity_ids[j] as usize) % comm_count == viewer_bucket
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    Some(candidates[rng.gen_range(0..candidates.len())])
}

pub async fn insert_engagement_vote(pool: &MySqlPool, root_post_id: u64, voter_user_id: u64) {
    use app::social::engagement::{self, EngagementDirection};
    let mut tx = pool.begin().await.expect("vote tx");
    engagement::record_root_engagement_vote_tx(
        &mut tx,
        root_post_id,
        voter_user_id,
        EngagementDirection::Yes,
    )
    .await
    .expect("record vote");
    tx.commit().await.expect("vote commit");
}

pub async fn insert_thread_comment(
    pool: &MySqlPool,
    root_id: u64,
    parent_id: u64,
    entity_id: u64,
    user_id: u64,
    body: &str,
) -> u64 {
    use app::social::engagement;
    let mut tx = pool.begin().await.expect("comment tx");
    engagement::assert_can_add_child_tx(&mut tx, parent_id, root_id, false)
        .await
        .expect("can add child");
    let plain = body.to_string();
    let markup = format!("<p>{plain}</p>");
    let res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code,
            is_moderation_mirror
        )
        VALUES (?, NULL, ?, ?, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, NULL, NULL, 0)
        "#,
    )
    .bind(entity_id)
    .bind(parent_id)
    .bind(root_id)
    .bind(&plain)
    .bind(&markup)
    .bind(&markup)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .expect("insert comment");
    let id = res.last_insert_id();
    tx.commit().await.expect("comment commit");
    id
}

/// Ensure a thread root is among the eight newest council reply candidacy roots.
///
/// Harness seeds 128 community posts with identical `published_at`; without a bump,
/// `load_reply_root_ids` may omit an arbitrary root under the LIMIT.
pub async fn bump_post_for_reply_candidacy(pool: &MySqlPool, root_post_id: u64) {
    sqlx::query("UPDATE post SET published_at = UTC_TIMESTAMP(6) WHERE id = ?")
        .bind(root_post_id)
        .execute(pool)
        .await
        .expect("bump post for reply candidacy");
}

/// Root post in a steward community the councillor can read (moderator / member_of mesh).
pub async fn insert_steward_mesh_root_post(
    pool: &MySqlPool,
    author_entity_id: u64,
    author_user_id: u64,
    steward_community_id: u64,
    verified_status_id: u64,
    body: &str,
) -> u64 {
    let plain = body.to_string();
    let markup = format!("<p>{plain}</p>");
    let res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, status_id, body_plain, body_markup, body_html_sanitized,
            published_at, created_by, modified_by
        )
        VALUES (?, NULL, ?, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?)
        "#,
    )
    .bind(author_entity_id)
    .bind(verified_status_id)
    .bind(&plain)
    .bind(&markup)
    .bind(&markup)
    .bind(author_user_id)
    .bind(author_user_id)
    .execute(pool)
    .await
    .expect("insert steward mesh root post");
    let post_id = res.last_insert_id();
    sqlx::query(
        "INSERT INTO post_audience_entity (post_id, audience_entity_id) VALUES (?, ?)",
    )
    .bind(post_id)
    .bind(steward_community_id)
    .execute(pool)
    .await
    .expect("insert steward mesh post audience");
    post_id
}

/// Every seeded user votes and L1-comments on a random peer post in their shared community bucket;
/// 64% also L2-reply; 8% of those L3-reply.
pub async fn simulate_engagement_comment_threads(
    pool: &MySqlPool,
    seed: &SeedIds,
    rng_seed: u64,
) -> EngagementSimStats {
    let mut rng = StdRng::seed_from_u64(rng_seed);
    let mut stats = EngagementSimStats::default();

    for i in 0..seed.entity_ids.len() {
        let Some(_author_idx) = pick_shared_community_author_idx(seed, i, &mut rng) else {
            continue;
        };
        let root_id = seed.post_community_ids[_author_idx];
        let voter_uid = seed.user_ids[i];
        let voter_eid = seed.entity_ids[i];

        stats.users_processed += 1;
        insert_engagement_vote(pool, root_id, voter_uid).await;
        stats.l1 += 1;
        let l1_id = insert_thread_comment(
            pool,
            root_id,
            root_id,
            voter_eid,
            voter_uid,
            &format!("L1 comment from user {i}"),
        )
        .await;

        if rng.gen::<f64>() < ENGAGEMENT_SIM_L2_CHANCE {
            stats.l2 += 1;
            let l2_id = insert_thread_comment(
                pool,
                root_id,
                l1_id,
                voter_eid,
                voter_uid,
                &format!("L2 reply from user {i}"),
            )
            .await;
            if rng.gen::<f64>() < ENGAGEMENT_SIM_L3_CHANCE {
                stats.l3 += 1;
                let _ = insert_thread_comment(
                    pool,
                    root_id,
                    l2_id,
                    voter_eid,
                    voter_uid,
                    &format!("L3 reply from user {i}"),
                )
                .await;
            }
        }
    }

    stats
}

pub fn expected_engagement_stats(seed: &SeedIds, rng_seed: u64) -> EngagementSimStats {
    let mut rng = StdRng::seed_from_u64(rng_seed);
    let mut stats = EngagementSimStats::default();
    for i in 0..seed.entity_ids.len() {
        if pick_shared_community_author_idx(seed, i, &mut rng).is_none() {
            continue;
        }
        stats.users_processed += 1;
        stats.l1 += 1;
        if rng.gen::<f64>() < ENGAGEMENT_SIM_L2_CHANCE {
            stats.l2 += 1;
            if rng.gen::<f64>() < ENGAGEMENT_SIM_L3_CHANCE {
                stats.l3 += 1;
            }
        }
    }
    stats
}
