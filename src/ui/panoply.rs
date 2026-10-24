//! Panoply ops dashboard card HTML.

use crate::html_escape::escape_html;
use crate::i18n;
use crate::social::panoply::{AuditRow, ContainedEmergencyRow};

#[derive(Debug, Clone)]
pub struct EmergencyClauseOption {
    pub id: u64,
    pub point: u16,
    pub label: String,
}

pub fn render_card(
    loc: &str,
    emergency_clauses: &[EmergencyClauseOption],
    queue: &[ContainedEmergencyRow],
    recent: &[AuditRow],
    flash: Option<&str>,
) -> String {
    let c = i18n::catalog();
    let flash_html = flash
        .map(|m| format!(r#"<p class="sub" role="status">{}</p>"#, escape_html(m)))
        .unwrap_or_default();

    let mut clause_options = String::new();
    for cl in emergency_clauses {
        clause_options.push_str(&format!(
            r#"<option value="{id}">§{point} — {label}</option>"#,
            id = cl.id,
            point = cl.point,
            label = escape_html(&cl.label),
        ));
    }

    let mut queue_html = String::new();
    if queue.is_empty() {
        queue_html.push_str(&format!(
            r#"<p class="sub">{}</p>"#,
            escape_html(&c.t(loc, "ui.panoply.queue_empty"))
        ));
    } else {
        queue_html.push_str("<ul class=\"panoply-queue-list\">");
        for row in queue {
            queue_html.push_str(&format!(
                r#"<li><span class="mono">#{case_id}</span> post {post_id} · @{handle} · <time>{opened}</time></li>"#,
                case_id = row.case_id,
                post_id = row.root_post_id,
                handle = escape_html(&row.target_handle),
                opened = row.opened_at.format("%Y-%m-%d %H:%M UTC"),
            ));
        }
        queue_html.push_str("</ul>");
    }

    let mut audit_html = String::new();
    if recent.is_empty() {
        audit_html.push_str(&format!(
            r#"<p class="sub">{}</p>"#,
            escape_html(&c.t(loc, "ui.panoply.audit_empty"))
        ));
    } else {
        audit_html.push_str("<ul class=\"panoply-audit-list\">");
        for row in recent {
            audit_html.push_str(&format!(
                r#"<li><span class="mono">{}</span> — {}</li>"#,
                escape_html(&row.action_type),
                escape_html(&row.note),
            ));
        }
        audit_html.push_str("</ul>");
    }

    format!(
        r#"<section class="card app-card panoply-ops-card">
<div class="card-accent" role="presentation"></div>
<h1>{heading}</h1>
<p class="sub">{intro}</p>
{flash}
<form class="panoply-ops-form" method="post" action="/home/ops">
<label class="auth-label">{action_label}
<select class="auth-input" name="action" required>
<option value="emergency_file">{emergency_label}</option>
<option value="civic_file">{civic_label}</option>
<option value="vacate_emergency">{vacate_label}</option>
<option value="eject">{eject_label}</option>
<option value="session_kill">{session_label}</option>
<option value="domain_block">{domain_label}</option>
</select>
</label>
<label class="auth-label">{post_id_label}<input class="auth-input" type="number" name="post_id" min="1" step="1" inputmode="numeric"></label>
<label class="auth-label">{clause_label}
<select class="auth-input" name="clause_id">
{clause_options}
</select>
</label>
<label class="auth-label">{case_id_label}<input class="auth-input" type="number" name="case_id" min="1" step="1" inputmode="numeric"></label>
<label class="auth-label">{handle_label}<input class="auth-input" type="text" name="handle" autocomplete="off"></label>
<label class="auth-label">{domain_label_field}<input class="auth-input" type="text" name="domain" autocomplete="off"></label>
<label class="auth-label">{note_label}<textarea class="auth-input" name="note" required rows="3"></textarea></label>
<button type="submit" class="auth-text-btn btn-cta">{submit}</button>
</form>
<h2>{queue_heading}</h2>
{queue_html}
<h2>{audit_heading}</h2>
{audit_html}
</section>"#,
        heading = escape_html(&c.t(loc, "ui.panoply.heading")),
        intro = escape_html(&c.t(loc, "ui.panoply.intro")),
        flash = flash_html,
        action_label = escape_html(&c.t(loc, "ui.panoply.action_label")),
        emergency_label = escape_html(&c.t(loc, "ui.panoply.action_emergency")),
        civic_label = escape_html(&c.t(loc, "ui.panoply.action_civic")),
        vacate_label = escape_html(&c.t(loc, "ui.panoply.action_vacate")),
        eject_label = escape_html(&c.t(loc, "ui.panoply.action_eject")),
        session_label = escape_html(&c.t(loc, "ui.panoply.action_session_kill")),
        domain_label = escape_html(&c.t(loc, "ui.panoply.action_domain_block")),
        post_id_label = escape_html(&c.t(loc, "ui.panoply.post_id_label")),
        clause_label = escape_html(&c.t(loc, "ui.panoply.clause_label")),
        clause_options = clause_options,
        case_id_label = escape_html(&c.t(loc, "ui.panoply.case_id_label")),
        handle_label = escape_html(&c.t(loc, "ui.panoply.handle_label")),
        domain_label_field = escape_html(&c.t(loc, "ui.panoply.domain_label")),
        note_label = escape_html(&c.t(loc, "ui.panoply.note_label")),
        submit = escape_html(&c.t(loc, "ui.panoply.submit")),
        queue_heading = escape_html(&c.t(loc, "ui.panoply.queue_heading")),
        queue_html = queue_html,
        audit_heading = escape_html(&c.t(loc, "ui.panoply.audit_heading")),
        audit_html = audit_html,
    )
}
