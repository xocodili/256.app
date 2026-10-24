//! Council thread reply dossiers: root candidacy, briefing assembly, parser regression.
mod common;



use app::ai::business_functions::{parse_thread_reply_response, ThreadReplyDecision};

use app::social::council_members::load_active_council_members;

use app::social::council_thread_dossier::{

    assemble_council_thread_dossier, build_council_reply_briefing, load_reply_root_ids,

};

use common::{insert_steward_mesh_root_post, insert_thread_comment, seed_light, TestDb};



async fn verified_status_id(pool: &sqlx::MySqlPool) -> u64 {

    sqlx::query_scalar(

        "SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1",

    )

    .fetch_one(pool)

    .await

    .expect("verified status ref")

}



#[tokio::test]

async fn steward_community_root_appears_in_reply_root_ids() {

    let db = TestDb::new().await;

    let pool = &db.pool;

    let seed = seed_light(pool).await;



    let members = load_active_council_members(pool)

        .await

        .expect("load council members");

    let member = members

        .first()

        .expect("expected at least one active councillor");



    let root_id = insert_steward_mesh_root_post(

        pool,

        seed.entity_ids[0],

        seed.user_ids[0],

        member.community_entity_id,

        verified_status_id(pool).await,

        "Council reply candidacy probe post",

    )

    .await;



    let root_ids = load_reply_root_ids(pool, member.entity_id)

        .await

        .expect("load reply root ids");



    assert!(

        root_ids.contains(&root_id),

        "steward mesh post {root_id} should appear in candidacy, got {root_ids:?}"

    );

}



#[tokio::test]

async fn dossier_includes_root_body_link_fields_and_nested_replies() {

    let db = TestDb::new().await;

    let pool = &db.pool;

    let seed = seed_light(pool).await;



    let members = load_active_council_members(pool)

        .await

        .expect("load council members");

    let member = members.first().expect("councillor");



    let root_id = insert_steward_mesh_root_post(

        pool,

        seed.entity_ids[0],

        seed.user_ids[0],

        member.community_entity_id,

        verified_status_id(pool).await,

        "Community post by test user for dossier",

    )

    .await;

    let human_entity_id = seed.entity_ids[0];

    let human_user_id = seed.user_ids[0];



    insert_thread_comment(

        pool,

        root_id,

        root_id,

        human_entity_id,

        human_user_id,

        "Council dossier nested reply plain body",

    )

    .await;



    let dossier = assemble_council_thread_dossier(pool, root_id)

        .await

        .expect("assemble dossier")

        .expect("dossier for seeded root");



    assert!(dossier.root.body_plain.contains("Community post by test user for dossier"));

    assert!(

        dossier

            .nodes

            .iter()

            .any(|n| n.body_plain.contains("Council dossier nested reply")),

        "expected nested reply in dossier nodes"

    );

}



#[tokio::test]

async fn briefing_parent_ids_only_from_loaded_roots_and_parser_rejects_foreign_id() {

    let db = TestDb::new().await;

    let pool = &db.pool;

    let seed = seed_light(pool).await;



    let members = load_active_council_members(pool)

        .await

        .expect("load council members");

    let member = members

        .first()

        .expect("expected at least one active councillor");



    let seeded_root = insert_steward_mesh_root_post(

        pool,

        seed.entity_ids[0],

        seed.user_ids[0],

        member.community_entity_id,

        verified_status_id(pool).await,

        "Council reply briefing probe post",

    )

    .await;



    let (document, parent_ids) = build_council_reply_briefing(pool, member.entity_id)

        .await

        .expect("build briefing");



    assert!(!document.is_empty(), "briefing document should not be empty");

    assert!(!parent_ids.is_empty(), "expected at least one candidate parent");

    assert!(

        document.contains("<thread root="),

        "document should contain thread dossier blocks"

    );

    assert!(

        !document.contains("body=\"") || document.contains("<body_plain>"),

        "document should use dossier XML, not one-line snippets"

    );



    let allowed = parent_ids[0];

    let valid = parse_thread_reply_response(

        &format!(

            r#"{{"action":"reply","parent_post_id":{allowed},"body_markup":"<p>hi</p>"}}"#

        ),

        &parent_ids,

    );

    assert!(matches!(valid, Some(ThreadReplyDecision::Reply { .. })));



    let foreign = parse_thread_reply_response(

        r#"{"action":"reply","parent_post_id":999999999,"body_markup":"<p>hi</p>"}"#,

        &parent_ids,

    );

    assert!(matches!(foreign, Some(ThreadReplyDecision::Decline { .. })));



    assert!(

        parent_ids.contains(&seeded_root),

        "seeded root {seeded_root} should be a reply parent candidate"

    );

}


