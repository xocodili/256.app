//! Legal-card body HTML from locale strings (`ui.legal.*_body`).

use crate::html_escape::escape_html;

fn legal_paragraph_html(paragraph: &str) -> String {
    let trimmed = paragraph.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((id_part, title_part)) = rest.split_once(']') {
            let id = id_part.trim();
            let title = title_part.trim();
            if !id.is_empty()
                && !title.is_empty()
                && id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return format!(
                    r#"<h2 id="{id}" class="legal-card-section-heading sub">{title}</h2>"#,
                    id = escape_html(id),
                    title = escape_html(title),
                );
            }
        }
    }
    let lines: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let bullet_lines: Vec<&str> = lines
        .iter()
        .filter(|line| line.starts_with('\u{2022}') || line.starts_with("•"))
        .copied()
        .collect();
    if lines.len() > 1 && bullet_lines.len() == lines.len() {
        let items = bullet_lines
            .iter()
            .map(|line| {
                line.trim_start_matches('\u{2022}')
                    .trim_start_matches('•')
                    .trim()
            })
            .map(escape_html)
            .map(|text| format!(r#"<li class="legal-card-list-item">{}</li>"#, text))
            .collect::<String>();
        return format!(r#"<ul class="legal-card-list">{}</ul>"#, items);
    }
    format!(r#"<p class="sub">{}</p>"#, escape_html(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_heading_with_anchor_id() {
        let html = legal_paragraph_html("[civic-standing] Civic standing");
        assert!(html.contains("id=\"civic-standing\""));
        assert!(html.contains("Civic standing"));
    }
}

/// Renders `ui.legal.*_body` copy: paragraphs split on blank lines; all-bullet blocks become lists.
#[must_use]
pub fn legal_body_html(body: &str) -> String {
    body.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(legal_paragraph_html)
        .collect()
}
