//! Locale-aware calendar date formatting for profile and post surfaces.

use chrono::{DateTime, Datelike, NaiveDate, Utc};

use crate::html_escape::escape_html;

fn ordinal_day(day: u32) -> String {
    let suffix = match day {
        11 | 12 | 13 => "th",
        n if n % 10 == 1 => "st",
        n if n % 10 == 2 => "nd",
        n if n % 10 == 3 => "rd",
        _ => "th",
    };
    format!("{day}{suffix}")
}

/// Calendar date in a locale-typical order (month names follow chrono `%B`, English today).
#[must_use]
pub fn format_calendar_date_localized(loc: &str, d: NaiveDate) -> String {
    let month = d.format("%B").to_string();
    let day = ordinal_day(d.day());
    let year = d.year();
    if loc.trim().to_ascii_lowercase().starts_with("en-us") {
        format!("{month} {day} {year}")
    } else {
        format!("{day} {month} {year}")
    }
}

/// Basic hour:minute suffix for post timestamps (UTC wall time).
fn format_post_time_localized(loc: &str, dt: DateTime<Utc>) -> String {
    if loc.trim().to_ascii_lowercase().starts_with("en-us") {
        dt.format("%I:%M %p").to_string()
    } else {
        dt.format("%H:%M").to_string()
    }
}

/// Visible post publish date and time from UTC wall time.
#[must_use]
pub fn format_post_published_localized(loc: &str, dt: DateTime<Utc>) -> String {
    format!(
        "{date}, {time}",
        date = format_calendar_date_localized(loc, dt.date_naive()),
        time = format_post_time_localized(loc, dt),
    )
}

/// `<time>` element for post card headers (`datetime` is full RFC 3339).
#[must_use]
pub fn post_published_at_html(loc: &str, dt: DateTime<Utc>) -> String {
    let iso = dt.to_rfc3339();
    let label = format_post_published_localized(loc, dt);
    format!(
        r#"<time class="public-post-published-at" datetime="{iso}">{label}</time>"#,
        iso = escape_html(&iso),
        label = escape_html(&label),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn en_default_is_day_first() {
        let d = NaiveDate::from_ymd_opt(2026, 6, 30).expect("date");
        assert_eq!(
            format_calendar_date_localized("en", d),
            "30th June 2026"
        );
    }

    #[test]
    fn en_us_is_month_first() {
        let d = NaiveDate::from_ymd_opt(2026, 6, 30).expect("date");
        assert_eq!(
            format_calendar_date_localized("en-US", d),
            "June 30th 2026"
        );
    }

    #[test]
    fn post_published_includes_time_en() {
        let dt = Utc.with_ymd_and_hms(2026, 6, 30, 14, 5, 0).unwrap();
        assert_eq!(
            format_post_published_localized("en", dt),
            "30th June 2026, 14:05"
        );
    }

    #[test]
    fn post_published_includes_time_en_us() {
        let dt = Utc.with_ymd_and_hms(2026, 6, 30, 14, 5, 0).unwrap();
        assert_eq!(
            format_post_published_localized("en-US", dt),
            "June 30th 2026, 02:05 PM"
        );
    }

    #[test]
    fn post_published_at_html_includes_datetime() {
        let dt = Utc.with_ymd_and_hms(2026, 6, 30, 14, 5, 0).unwrap();
        let html = post_published_at_html("en", dt);
        assert!(html.contains("public-post-published-at"));
        assert!(html.contains("datetime=\"2026-06-30T14:05:00+00:00\""));
        assert!(html.contains("30th June 2026, 14:05"));
    }
}
