//! Entity profile edit forms — mirrors create-entity card fragments (profit / non-profit / gathering).

use crate::auth::password::MIN_PASSWORD_LEN;
use crate::html_escape::escape_html;
use crate::i18n::Catalog;
use crate::ref_keys;
use crate::social::entity_profile_edit::{
    BrandEditFields, EntityProfileEditSnapshot, GatheringEditFields,
};

fn edit_entity_card_title(type_key: &str, loc: &str, c: &Catalog) -> String {
    let key = match type_key {
        ref_keys::KEY_ENTITY_TYPE_PROFIT => "ui.home.edit_entity_card_title_business",
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => "ui.home.edit_entity_card_title_nonprofit",
        ref_keys::KEY_ENTITY_TYPE_GATHERING => "ui.home.edit_entity_card_title_event",
        _ => "ui.entity_private.entity_edit_heading",
    };
    c.t(loc, key)
}

fn entity_delete_open_label(type_key: &str, loc: &str, c: &Catalog) -> String {
    match type_key {
        ref_keys::KEY_ENTITY_TYPE_GATHERING => c.t(loc, "ui.entity_private.delete_event_open"),
        _ => c.t(loc, "ui.entity_private.delete_entity_open"),
    }
}

fn entity_delete_heading(type_key: &str, loc: &str, c: &Catalog) -> String {
    match type_key {
        ref_keys::KEY_ENTITY_TYPE_GATHERING => c.t(loc, "ui.entity_private.delete_event_heading"),
        _ => c.t(loc, "ui.entity_private.delete_entity_heading"),
    }
}

fn entity_delete_confirm_label(type_key: &str, loc: &str, c: &Catalog) -> String {
    match type_key {
        ref_keys::KEY_ENTITY_TYPE_GATHERING => c.t(loc, "ui.entity_private.delete_event_confirm"),
        _ => c.t(loc, "ui.entity_private.delete_entity_confirm"),
    }
}

fn format_owner_handles_for_edit(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.to_ascii_lowercase().starts_with("add:") {
        return trimmed.to_string();
    }
    format!("Add: {trimmed}")
}

/// Matches client `markupLength`: normalized HTML string length (UTF-8 code points).
fn description_markup_char_count(html: &str) -> usize {
    html.replace("\r\n", "\n").chars().count()
}

const ENTITY_DESCRIPTION_TOOLBAR: &str = r##"<div class="create-post-editor-toolbar create-entity-description-toolbar" role="toolbar" aria-label="Text formatting">
      <button type="button" data-cmd="bold" class="create-post-editor-btn" aria-label="Bold" title="Bold"><b>B</b></button>
      <button type="button" data-cmd="italic" class="create-post-editor-btn" aria-label="Italic" title="Italic"><i>I</i></button>
      <button type="button" data-cmd="underline" class="create-post-editor-btn" aria-label="Underline" title="Underline"><u>U</u></button>
      <button type="button" data-cmd="foreColor" data-value="#f6f6fa" class="create-post-editor-btn create-post-editor-color-btn" style="--swatch:#f6f6fa" aria-label="White text" title="White text"></button>
      <button type="button" data-cmd="foreColor" data-value="#f4c430" class="create-post-editor-btn create-post-editor-color-btn" style="--swatch:#f4c430" aria-label="Yellow text" title="Yellow text"></button>
      <button type="button" data-cmd="foreColor" data-value="#4169e1" class="create-post-editor-btn create-post-editor-color-btn" style="--swatch:#4169e1" aria-label="Blue text" title="Blue text"></button>
      <button type="button" data-cmd="foreColor" data-value="#4fc3f7" class="create-post-editor-btn create-post-editor-color-btn" style="--swatch:#4fc3f7" aria-label="Cyan text" title="Cyan text"></button>
      <button type="button" data-cmd="foreColor" data-value="#1f6b3a" class="create-post-editor-btn create-post-editor-color-btn" style="--swatch:#1f6b3a" aria-label="Green text" title="Green text"></button>
      <button type="button" data-cmd="foreColor" data-value="#a3232b" class="create-post-editor-btn create-post-editor-color-btn" style="--swatch:#a3232b" aria-label="Red text" title="Red text"></button>
      <button type="button" data-cmd="foreColor" data-value="#7b1e3d" class="create-post-editor-btn create-post-editor-color-btn" style="--swatch:#7b1e3d" aria-label="Magenta text" title="Magenta text"></button>
    </div>"##;

pub(super) fn profile_entity_edit_block_html(
    handle: &str,
    edit: &EntityProfileEditSnapshot,
    loc: &str,
    c: &Catalog,
) -> String {
    match edit {
        EntityProfileEditSnapshot::Profit(fields) => {
            brand_entity_edit_block_html(handle, fields, true, loc, c)
        }
        EntityProfileEditSnapshot::NonProfit(fields) => {
            brand_entity_edit_block_html(handle, fields, false, loc, c)
        }
        EntityProfileEditSnapshot::Gathering(fields) => {
            gathering_entity_edit_block_html(handle, fields, loc, c)
        }
    }
}

fn brand_entity_edit_block_html(
    handle: &str,
    fields: &BrandEditFields,
    is_profit: bool,
    loc: &str,
    c: &Catalog,
) -> String {
    let type_key = if is_profit {
        ref_keys::KEY_ENTITY_TYPE_PROFIT
    } else {
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
    };
    let form_class = if is_profit {
        "create-entity-form create-entity-form--profit profile-entity-edit-form"
    } else {
        "create-entity-form create-entity-form--non-profit profile-entity-edit-form"
    };
    let accent = if is_profit {
        "entity-profile-accent--profit"
    } else {
        "entity-profile-accent--non_profit"
    };
    let intro = if is_profit {
        c.t(loc, "ui.home.edit_entity_profit_intro")
    } else {
        c.t(loc, "ui.home.edit_entity_non_profit_intro")
    };
    let trading_label = if is_profit {
        c.t(loc, "ui.home.create_entity_trading_label")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_public_name_label")
    };
    let trading_hint = if is_profit {
        c.t(loc, "ui.home.create_entity_trading_hint")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_public_name_hint")
    };
    let legal_label = if is_profit {
        c.t(loc, "ui.home.create_entity_legal_name_label")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_legal_name_label")
    };
    let founded_label = if is_profit {
        c.t(loc, "ui.home.create_entity_founded_label")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_founded_label")
    };
    let founded_hint = if is_profit {
        c.t(loc, "ui.home.create_entity_founded_hint")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_founded_hint")
    };
    let description_label = if is_profit {
        c.t(loc, "ui.home.create_entity_description_label")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_description_label")
    };
    let description_placeholder = if is_profit {
        c.t(loc, "ui.home.create_entity_description_placeholder")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_description_placeholder")
    };
    let intention_label = if is_profit {
        c.t(loc, "ui.home.create_entity_intention_label")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_intention_label")
    };
    let place_heading = if is_profit {
        c.t(loc, "ui.home.create_entity_place_heading")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_place_heading")
    };
    let logo_hint = if is_profit {
        c.t(loc, "ui.home.create_entity_logo_url_hint")
    } else {
        c.t(loc, "ui.home.create_entity_non_profit_logo_url_hint")
    };
    let founded_display = fields.founded_at.format("%Y-%m-%d").to_string();
    let description_initial = fields.description.as_deref().unwrap_or("");
    let desc_len = description_markup_char_count(description_initial).min(4000);
    let place_badge = if fields.location_place_label.trim().is_empty() {
        r#"<span id="profile-entity-edit-place-badge" class="handle-availability-badge handle-availability-badge--label" hidden></span>"#.to_string()
    } else {
        format!(
            r#"<span id="profile-entity-edit-place-badge" class="handle-availability-badge handle-availability-badge--label handle-availability-badge--success">{}</span>"#,
            escape_html(&c.t(loc, "ui.home.handle_ready"))
        )
    };

    let mut body = format!(
        r#"<label for="profile-entity-edit-trading-label" class="create-entity-handle-label">{trading_label}</label>
      <input id="profile-entity-edit-trading-label" name="label" class="search-add-input" type="text" autocomplete="organization" maxlength="512" required aria-required="true" value="{label_val}">
      <p class="hint">{trading_hint}</p>
      <label for="profile-entity-edit-legal-name" class="create-entity-handle-label">{legal_label}</label>
      <input id="profile-entity-edit-legal-name" name="legal_name" class="search-add-input" type="text" autocomplete="organization" maxlength="512" required aria-required="true" value="{legal_val}">
      <label for="profile-entity-edit-founded" class="create-entity-handle-label">{founded_label}</label>
      <input id="profile-entity-edit-founded" name="founded_at" class="search-add-input" type="date" required aria-required="true" value="{founded}">
      <p class="hint">{founded_hint}</p>"#,
        trading_label = escape_html(&trading_label),
        label_val = escape_html(&fields.label),
        trading_hint = escape_html(&trading_hint),
        legal_label = escape_html(&legal_label),
        legal_val = escape_html(&fields.legal_name),
        founded_label = escape_html(&founded_label),
        founded = escape_html(&founded_display),
        founded_hint = escape_html(&founded_hint),
    );

    if is_profit {
        body.push_str(&description_block_html(
            &description_label,
            &description_placeholder,
            description_initial,
            desc_len,
        ));
        body.push_str(&format!(
            r#"<label for="profile-entity-edit-intention" class="create-entity-handle-label">{intention_label}</label>
      <input id="profile-entity-edit-intention" name="intention" class="search-add-input" type="text" maxlength="512" autocomplete="off" spellcheck="true" value="{intention}">
      <label for="profile-entity-edit-website" class="create-entity-handle-label">{website_label}</label>
      <input id="profile-entity-edit-website" name="website_url" class="search-add-input" type="url" inputmode="url" autocomplete="url" spellcheck="false" placeholder="{website_ph}" value="{website}">
      <label for="profile-entity-edit-logo-url" class="create-entity-handle-label">{logo_label}</label>
      <input id="profile-entity-edit-logo-url" name="logo_url" class="search-add-input" type="url" inputmode="url" autocomplete="off" spellcheck="false" placeholder="{logo_ph}" value="{logo}">
      <p class="hint">{logo_hint}</p>"#,
            intention_label = escape_html(&intention_label),
            intention = escape_html(fields.intention.as_deref().unwrap_or("")),
            website_label = escape_html(&c.t(loc, "ui.home.create_entity_website_label")),
            website_ph = escape_html(&c.t(loc, "ui.home.create_entity_website_placeholder")),
            website = escape_html(fields.website_url.as_deref().unwrap_or("")),
            logo_label = escape_html(&c.t(loc, "ui.home.create_entity_logo_url_label")),
            logo_ph = escape_html(&c.t(loc, "ui.home.create_entity_logo_url_placeholder")),
            logo = escape_html(fields.logo_url.as_deref().unwrap_or("")),
            logo_hint = escape_html(&logo_hint),
        ));
    } else {
        body.push_str(&format!(
            r#"<label for="profile-entity-edit-intention" class="create-entity-handle-label">{intention_label}</label>
      <input id="profile-entity-edit-intention" name="intention" class="search-add-input" type="text" maxlength="512" autocomplete="off" spellcheck="true" value="{intention}">
      <label for="profile-entity-edit-website" class="create-entity-handle-label">{website_label}</label>
      <input id="profile-entity-edit-website" name="website_url" class="search-add-input" type="url" inputmode="url" autocomplete="url" spellcheck="false" placeholder="{website_ph}" value="{website}">"#,
            intention_label = escape_html(&intention_label),
            intention = escape_html(fields.intention.as_deref().unwrap_or("")),
            website_label = escape_html(&c.t(loc, "ui.home.create_entity_website_label")),
            website_ph = escape_html(&c.t(loc, "ui.home.create_entity_website_placeholder")),
            website = escape_html(fields.website_url.as_deref().unwrap_or("")),
        ));
        body.push_str(&description_block_html(
            &description_label,
            &description_placeholder,
            description_initial,
            desc_len,
        ));
        body.push_str(&format!(
            r#"<label for="profile-entity-edit-logo-url" class="create-entity-handle-label">{logo_label}</label>
      <input id="profile-entity-edit-logo-url" name="logo_url" class="search-add-input" type="url" inputmode="url" autocomplete="off" spellcheck="false" placeholder="{logo_ph}" value="{logo}">
      <p class="hint">{logo_hint}</p>"#,
            logo_label = escape_html(&c.t(loc, "ui.home.create_entity_logo_url_label")),
            logo_ph = escape_html(&c.t(loc, "ui.home.create_entity_logo_url_placeholder")),
            logo = escape_html(fields.logo_url.as_deref().unwrap_or("")),
            logo_hint = escape_html(&logo_hint),
        ));
    }

    body.push_str(&place_block_html(
        &place_heading,
        &place_badge,
        &fields.location_place_label,
        &c.t(loc, "ui.home.create_entity_place_query_placeholder"),
    ));
    body.push_str(&owner_handles_block_html(
        &c.t(loc, "ui.home.create_entity_owner_handles_label"),
        &c.t(loc, "ui.home.create_entity_owner_handles_placeholder"),
        &format_owner_handles_for_edit(&fields.owner_handles),
    ));

    entity_edit_shell_html(
        handle,
        type_key,
        accent,
        &intro,
        form_class,
        &body,
        Some(fields.location_id),
        loc,
        c,
    )
}

fn gathering_entity_edit_block_html(
    handle: &str,
    fields: &GatheringEditFields,
    loc: &str,
    c: &Catalog,
) -> String {
    let description_initial = fields.description.as_deref().unwrap_or("");
    let desc_len = description_markup_char_count(description_initial).min(4000);
    let description_html = description_block_html(
        &c.t(loc, "ui.home.create_entity_gathering_description_label"),
        &c.t(loc, "ui.home.create_entity_gathering_description_placeholder"),
        description_initial,
        desc_len,
    );
    let owners_html = owner_handles_block_html(
        &c.t(loc, "ui.home.create_entity_gathering_cohosts_label"),
        &c.t(loc, "ui.home.create_entity_owner_handles_placeholder"),
        &format_owner_handles_for_edit(&fields.owner_handles),
    );
    let place_heading = c.t(loc, "ui.home.create_entity_place_heading");
    let place_badge = if fields.location_place_label.trim().is_empty() {
        r#"<span id="profile-entity-edit-place-badge" class="handle-availability-badge handle-availability-badge--label" hidden></span>"#.to_string()
    } else {
        format!(
            r#"<span id="profile-entity-edit-place-badge" class="handle-availability-badge handle-availability-badge--label handle-availability-badge--success">{}</span>"#,
            escape_html(&c.t(loc, "ui.home.handle_ready"))
        )
    };
    let place_html = place_block_html(
        &place_heading,
        &place_badge,
        &fields.location_place_label,
        &c.t(loc, "ui.home.create_entity_place_query_placeholder"),
    );
    let body = format!(
        r#"<label for="profile-entity-edit-trading-label" class="create-entity-handle-label">{label}</label>
      <input id="profile-entity-edit-trading-label" name="label" class="search-add-input" type="text" autocomplete="off" maxlength="512" required aria-required="true" value="{label_val}">
      {description_html}
      <label for="profile-entity-edit-intention" class="create-entity-handle-label">{intention_label}</label>
      <input id="profile-entity-edit-intention" name="intention" class="search-add-input" type="text" maxlength="512" autocomplete="off" spellcheck="true" value="{intention}">
      <label for="profile-entity-edit-event-url" class="create-entity-handle-label">{event_url_label}</label>
      <input id="profile-entity-edit-event-url" name="event_url" class="search-add-input" type="url" inputmode="url" autocomplete="url" spellcheck="false" placeholder="{event_url_ph}" value="{event_url}">
      <p class="hint">{event_url_hint}</p>
      {place_html}
      {owners_html}"#,
        label = escape_html(&c.t(loc, "ui.home.create_entity_gathering_name_label")),
        label_val = escape_html(&fields.label),
        description_html = description_html,
        intention_label = escape_html(&c.t(loc, "ui.home.create_entity_gathering_intention_label")),
        intention = escape_html(fields.intention.as_deref().unwrap_or("")),
        event_url_label = escape_html(&c.t(loc, "ui.home.create_entity_gathering_event_url_label")),
        event_url_ph = escape_html(&c.t(loc, "ui.home.create_entity_gathering_event_url_placeholder")),
        event_url = escape_html(fields.event_url.as_deref().unwrap_or("")),
        event_url_hint = escape_html(&c.t(loc, "ui.home.create_entity_gathering_event_url_hint")),
        place_html = place_html,
        owners_html = owners_html,
    );

    entity_edit_shell_html(
        handle,
        ref_keys::KEY_ENTITY_TYPE_GATHERING,
        "entity-profile-accent--gathering",
        &c.t(loc, "ui.home.edit_entity_gathering_intro"),
        "create-entity-form create-entity-form--gathering profile-entity-edit-form",
        &body,
        Some(fields.location_id),
        loc,
        c,
    )
}

fn description_block_html(
    label: &str,
    placeholder: &str,
    initial_html: &str,
    char_count: usize,
) -> String {
    format!(
        r#"<label for="profile-entity-edit-description-editor" class="create-entity-handle-label">{label}</label>
      <input type="hidden" id="profile-entity-edit-description" name="description" />
      <textarea id="profile-entity-edit-description-initial" hidden readonly>{initial}</textarea>
      {toolbar}
      <div id="profile-entity-edit-description-editor" class="create-post-editor create-entity-description-editor" contenteditable="true" role="textbox" aria-multiline="true" aria-label="{label}" data-placeholder="{placeholder}"></div>
      <p id="profile-entity-edit-description-char-hint" class="hint create-post-char-hint">{char_count}/4000</p>"#,
        label = escape_html(label),
        initial = escape_html(initial_html),
        toolbar = ENTITY_DESCRIPTION_TOOLBAR,
        placeholder = escape_html(placeholder),
        char_count = char_count,
    )
}

fn place_block_html(
    heading: &str,
    place_badge: &str,
    place_query: &str,
    query_placeholder: &str,
) -> String {
    format!(
        r#"<p class="create-entity-handle-label create-entity-place-heading-row">
        <span id="profile-entity-edit-place-heading-text">{heading}</span>
        {place_badge}
      </p>
      <div class="create-entity-place-row">
        <input id="profile-entity-edit-google-place-query" class="search-add-input" type="text" autocomplete="off" spellcheck="false" placeholder="{query_ph}" aria-labelledby="profile-entity-edit-place-heading-text" value="{place_query}" />
        <input type="hidden" id="profile-entity-edit-google-place-id" />
      </div>
      <ul id="profile-entity-edit-place-results" class="create-entity-place-results" hidden></ul>"#,
        heading = escape_html(heading),
        place_badge = place_badge,
        query_ph = escape_html(query_placeholder),
        place_query = escape_html(place_query),
    )
}

fn owner_handles_block_html(label: &str, placeholder: &str, value: &str) -> String {
    format!(
        r#"<label for="profile-entity-edit-owner-handles" class="create-entity-handle-label">{label}</label>
      <input id="profile-entity-edit-owner-handles" name="owner_handles" class="search-add-input" type="text" autocomplete="off" spellcheck="false" placeholder="{placeholder}" value="{value}" />"#,
        label = escape_html(label),
        placeholder = escape_html(placeholder),
        value = escape_html(value),
    )
}

fn entity_edit_shell_html(
    handle: &str,
    type_key: &str,
    accent: &str,
    intro: &str,
    form_class: &str,
    form_body: &str,
    location_id: Option<u64>,
    loc: &str,
    c: &Catalog,
) -> String {
    let location_hidden = location_id.map_or(String::new(), |id| {
        format!(
            r#"<input type="hidden" id="profile-entity-edit-location-id" name="location_id" value="{id}" />"#,
            id = id
        )
    });
    format!(
        r#"<section class="profile-block profile-entity-edit-block app-create-entity-card {accent}" id="profile-entity-edit-block" data-entity-handle="{handle}" data-entity-type-key="{type_key}" data-password-min-len="{password_min}" hidden>
  <div id="profile-entity-edit-panel">
    <header class="create-post-card-header">
      <h1>{heading}</h1>
    </header>
    <p class="sub create-entity-card-lead">{intro}</p>
    <form id="profile-entity-edit-form" class="{form_class}" autocomplete="on" novalidate>
      {location_hidden}
      <label for="profile-entity-edit-handle" class="create-entity-handle-label">{handle_label}</label>
      <input id="profile-entity-edit-handle" class="search-add-input" type="text" value="{handle_at}" readonly autocomplete="username">
      {form_body}
      <p class="hint create-post-submit-status" id="profile-entity-edit-status" hidden aria-live="polite"></p>
    </form>
    <div class="profile-human-edit-actions">
      <button type="submit" form="profile-entity-edit-form" class="auth-text-btn btn-cta profile-connect-cta-btn" id="profile-entity-edit-save">{save_label}</button>
      <button type="button" class="auth-text-btn profile-account-delete-btn profile-connect-cta-btn" id="profile-delete-entity-open">{delete_entity_label}</button>
      <button type="button" class="auth-text-btn profile-connect-cta-btn" id="profile-entity-edit-cancel">{cancel_label}</button>
    </div>
  </div>
  {delete_panel}
</section>"#,
        accent = accent,
        handle = escape_html(handle),
        type_key = escape_html(type_key),
        password_min = MIN_PASSWORD_LEN,
        heading = escape_html(&edit_entity_card_title(type_key, loc, c)),
        intro = escape_html(intro),
        form_class = form_class,
        location_hidden = location_hidden,
        form_body = form_body,
        handle_label = escape_html(&c.t(loc, "ui.home.create_entity_handle_label")),
        handle_at = escape_html(&format!("@{handle}")),
        save_label = escape_html(&c.t(loc, "ui.entity_private.profile_edit_save")),
        delete_entity_label = escape_html(&entity_delete_open_label(type_key, loc, c)),
        cancel_label = escape_html(&c.t(loc, "ui.shared.cancel")),
        delete_panel = delete_entity_panel_html(type_key, loc, c),
    )
}

fn delete_entity_panel_html(type_key: &str, loc: &str, c: &Catalog) -> String {
    let required_abbr = escape_html(&c.t(loc, "ui.home_profile.required_abbr"));
    format!(
        r#"<div class="profile-account-panel profile-delete-account-card" id="profile-delete-entity-block" hidden>
    <h2 class="profile-block-heading">{delete_entity_heading}</h2>
    <p class="sub">{delete_entity_lead}</p>
    <ul class="profile-delete-account-list">
      <li>{delete_entity_bullet_connections}</li>
      <li>{delete_entity_bullet_cases}</li>
      <li>{delete_entity_bullet_posts}</li>
    </ul>
    <label class="profile-field-label profile-field-label--required" for="profile-delete-entity-password"><span class="profile-field-label__text">{current_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
    <input id="profile-delete-entity-password" type="password" autocomplete="current-password" minlength="{password_min}">
    <label class="profile-field-label profile-field-label--required" for="profile-delete-entity-password-confirm"><span class="profile-field-label__text">{confirm_current_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
    <input id="profile-delete-entity-password-confirm" type="password" autocomplete="current-password" minlength="{password_min}">
    <label class="profile-delete-account-ack" for="profile-delete-entity-ack">
      <input id="profile-delete-entity-ack" type="checkbox" name="delete_ack">
      <span class="profile-delete-account-ack-text">{delete_entity_ack_label}</span>
    </label>
    <div class="profile-human-edit-actions">
      <button type="button" class="auth-text-btn btn-cta profile-account-delete-btn profile-connect-cta-btn" id="profile-delete-entity-submit">{delete_entity_confirm_label}</button>
      <button type="button" class="auth-text-btn profile-connect-cta-btn" id="profile-delete-entity-cancel">{cancel_label}</button>
    </div>
    <p class="err" id="profile-delete-entity-msg" role="alert" hidden></p>
  </div>"#,
        delete_entity_heading = escape_html(&entity_delete_heading(type_key, loc, c)),
        delete_entity_lead = escape_html(&c.t(loc, "ui.entity_private.delete_entity_lead")),
        delete_entity_bullet_connections = escape_html(&c.t(loc, "ui.entity_private.delete_entity_bullet_connections")),
        delete_entity_bullet_cases = escape_html(&c.t(loc, "ui.entity_private.delete_entity_bullet_cases")),
        delete_entity_bullet_posts = escape_html(&c.t(loc, "ui.entity_private.delete_entity_bullet_posts")),
        current_password_label = escape_html(&c.t(loc, "ui.entity_private.account_current_password_label")),
        confirm_current_password_label = escape_html(&c.t(loc, "ui.entity_private.account_confirm_password_label")),
        required_abbr = required_abbr,
        password_min = MIN_PASSWORD_LEN,
        delete_entity_ack_label = escape_html(&c.t(loc, "ui.entity_private.delete_entity_ack")),
        delete_entity_confirm_label = escape_html(&entity_delete_confirm_label(type_key, loc, c)),
        cancel_label = escape_html(&c.t(loc, "ui.shared.cancel")),
    )
}
