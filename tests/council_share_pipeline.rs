//! Council share pipeline unit checks (parser + paywall narrowing helpers).

use app::ai::business_functions::response::{
    parse_content_shortlist_response, ContentShortlistDecision,
};
use app::content::article_extract::{html_indicates_paywall, ExtractFailure};
use app::content::domain_discover::article_like_url;
use app::content::domain_paywall::{paywall_sort_key, PAYWALL_FREE, PAYWALL_PARTIAL, PAYWALL_WHOLE};
use app::content::feed_poll::parse_feed_entry_urls;
use app::content::og_text::normalize_og_field;

#[test]
fn og_normalize_decodes_entities() {
    assert_eq!(normalize_og_field("Tom &amp; Jerry"), "Tom & Jerry");
}

#[test]
fn shortlist_parser_respects_allowlist() {
    let d = parse_content_shortlist_response(
        r#"{"action":"shortlist","content_link_ids":[1,2,3,4,5,6,7,8,9]}"#,
        &[1, 2, 3, 4, 5, 6, 7, 8],
    );
    match d {
        Some(ContentShortlistDecision::Shortlist { content_link_ids }) => {
            assert_eq!(content_link_ids.len(), 8);
            assert!(content_link_ids.iter().all(|id| (1..=8).contains(id)));
        }
        _ => panic!("expected shortlist"),
    }
}

#[test]
fn paywall_html_detected() {
    let html = r#"<html><body><div class="paywall">Subscribe to continue reading</div></body></html>"#;
    assert!(html_indicates_paywall(html));
}

#[test]
fn feed_poll_parses_rss_links() {
    let xml = r#"<rss><channel><item><link>https://news.test/a</link></item></channel></rss>"#;
    let urls = parse_feed_entry_urls(xml, "rss");
    assert_eq!(urls, vec!["https://news.test/a".to_string()]);
}

#[test]
fn article_like_skips_homepage() {
    assert!(!article_like_url("https://theguardian.com/", "theguardian.com"));
    assert!(article_like_url(
        "https://www.theguardian.com/world/2026/aug/14/story",
        "theguardian.com"
    ));
}

#[test]
fn extract_failure_paywall_is_distinct() {
    assert_eq!(ExtractFailure::Paywalled, ExtractFailure::Paywalled);
    assert_ne!(ExtractFailure::Paywalled, ExtractFailure::TooShort);
}

#[test]
fn paywall_sort_prefers_free_hosts_for_selection() {
    assert!(paywall_sort_key(PAYWALL_FREE) < paywall_sort_key(PAYWALL_PARTIAL));
    assert!(paywall_sort_key(PAYWALL_PARTIAL) < paywall_sort_key(PAYWALL_WHOLE));
}
