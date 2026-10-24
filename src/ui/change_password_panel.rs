use crate::auth::password::MIN_PASSWORD_LEN;
use crate::html_escape::escape_html;
use crate::i18n::Catalog;

#[derive(Debug, Clone, Copy)]
pub struct ChangePasswordPanelOpts {
    /// When true the panel is visible immediately (standalone reset page).
    pub standalone: bool,
    /// When true skip the current-password verification step (forgot-password recovery).
    pub skip_verify_step: bool,
}

impl Default for ChangePasswordPanelOpts {
    fn default() -> Self {
        Self {
            standalone: false,
            skip_verify_step: false,
        }
    }
}

#[must_use]
pub fn change_password_panel_html(loc: &str, c: &Catalog, opts: ChangePasswordPanelOpts) -> String {
    let required_abbr = escape_html(&c.t(loc, "ui.shared.required_abbr"));
    let hidden_attr = if opts.standalone { "" } else { " hidden" };
    let skip_verify = if opts.skip_verify_step { "1" } else { "0" };
    let verify_hidden = if opts.skip_verify_step { " hidden" } else { "" };
    let new_hidden = if opts.skip_verify_step { "" } else { " hidden" };
    let cancel_btn = if opts.standalone {
        String::new()
    } else {
        format!(
            r#"<button type="button" class="auth-text-btn profile-connect-cta-btn" id="profile-change-password-cancel">{}</button>"#,
            escape_html(&c.t(loc, "ui.entity_private.change_password_back"))
        )
    };
    let reset_lead = if opts.skip_verify_step {
        escape_html(&c.t(loc, "ui.login.reset_password_new_lead"))
    } else {
        escape_html(&c.t(loc, "ui.entity_private.change_password_new_lead"))
    };

    format!(
        r#"<div class="profile-account-panel" id="profile-change-password-block"{hidden_attr} data-skip-verify="{skip_verify}" data-password-min-len="{password_min}">
    <h2 class="profile-block-heading">{change_password_heading}</h2>
    <div id="profile-change-password-step-verify"{verify_hidden}>
      <p class="sub">{change_password_verify_lead}</p>
      <label class="profile-field-label profile-field-label--required" for="profile-current-password"><span class="profile-field-label__text">{current_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
      <input id="profile-current-password" type="password" autocomplete="current-password" minlength="{password_min}">
      <label class="profile-field-label profile-field-label--required" for="profile-current-password-confirm"><span class="profile-field-label__text">{confirm_current_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
      <input id="profile-current-password-confirm" type="password" autocomplete="current-password" minlength="{password_min}">
      <div class="profile-human-edit-actions">
        <button type="button" class="auth-text-btn btn-cta profile-connect-cta-btn" id="profile-change-password-continue">{continue_label}</button>
        {cancel_btn}
      </div>
    </div>
    <div id="profile-change-password-step-new"{new_hidden}>
      <p class="sub">{reset_lead}</p>
      <label class="profile-field-label profile-field-label--required" for="profile-new-password"><span class="profile-field-label__text">{new_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
      <input id="profile-new-password" type="password" autocomplete="new-password" minlength="{password_min}">
      <p class="hint">{password_hint}</p>
      <label class="profile-field-label profile-field-label--required" for="profile-new-password-confirm"><span class="profile-field-label__text">{confirm_new_password_label}</span><abbr class="profile-req-mark" title="{required_abbr}">*</abbr></label>
      <input id="profile-new-password-confirm" type="password" autocomplete="new-password" minlength="{password_min}">
      <div class="profile-human-edit-actions">
        <button type="button" class="auth-text-btn btn-cta profile-connect-cta-btn" id="profile-change-password-save">{change_password_save_label}</button>
        <button type="button" class="auth-text-btn profile-connect-cta-btn" id="profile-change-password-back">{back_label}</button>
      </div>
    </div>
    <p class="err" id="profile-change-password-msg" role="alert" hidden></p>
  </div>"#,
        hidden_attr = hidden_attr,
        skip_verify = skip_verify,
        password_min = MIN_PASSWORD_LEN,
        change_password_heading = escape_html(&c.t(loc, "ui.entity_private.change_password_heading")),
        verify_hidden = verify_hidden,
        change_password_verify_lead =
            escape_html(&c.t(loc, "ui.entity_private.change_password_verify_lead")),
        current_password_label =
            escape_html(&c.t(loc, "ui.entity_private.account_current_password_label")),
        confirm_current_password_label =
            escape_html(&c.t(loc, "ui.entity_private.account_confirm_password_label")),
        required_abbr = required_abbr,
        continue_label = escape_html(&c.t(loc, "ui.entity_private.change_password_continue")),
        cancel_btn = cancel_btn,
        new_hidden = new_hidden,
        reset_lead = reset_lead,
        new_password_label = escape_html(&c.t(loc, "ui.entity_private.account_new_password_label")),
        password_hint = escape_html(&c.t(loc, "ui.register.password_hint")),
        confirm_new_password_label =
            escape_html(&c.t(loc, "ui.entity_private.account_confirm_new_password_label")),
        change_password_save_label =
            escape_html(&c.t(loc, "ui.entity_private.change_password_save")),
        back_label = escape_html(&c.t(loc, "ui.entity_private.change_password_back")),
    )
}
