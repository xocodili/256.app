//! Noms currency glyph — `{amount}` + icon; icon tooltip reads "Noms".
//! Sprite: `static/icons/post-actions.svg#icon-nom`.

use crate::html_escape::escape_html;
use crate::i18n::Catalog;

const SPRITE_HREF: &str = "/static/icons/post-actions.svg?v=20260812-chrome-nav";
const ICON_VIEW: &str = "0 0 24 24";

#[must_use]
pub fn currency_tooltip(c: &Catalog, loc: &str) -> String {
    c.t(loc, "noms.currency_tooltip")
}

fn icon_svg(class: &str, size: u16, tooltip: &str) -> String {
    let tip = escape_html(tooltip);
    format!(
        r#"<svg class="{class}" viewBox="{view}" width="{size}" height="{size}" role="img" xmlns="http://www.w3.org/2000/svg"><title>{tip}</title><use href="{href}#icon-nom"/></svg>"#,
        class = class,
        view = ICON_VIEW,
        size = size,
        tip = tip,
        href = SPRITE_HREF,
    )
}

/// Inline Noms glyph at `--icon-size-sm` (16px).
#[must_use]
pub fn nom_icon_sm(tooltip: &str) -> String {
    icon_svg("nom-icon nom-icon--sm system-icon--sm", 16, tooltip)
}

/// Visible price: amount + icon (no "Noms" word).
#[must_use]
pub fn nom_amount_html(amount: u16, tooltip: &str) -> String {
    format!(
        r#"<span class="nom-amount"><span class="nom-amount-count mono">{amount}</span>{icon}</span>"#,
        amount = amount,
        icon = nom_icon_sm(tooltip),
    )
}

/// Parenthetical price: `(1 {icon})`.
#[must_use]
pub fn nom_paren_html(amount: u16, tooltip: &str) -> String {
    format!(
        r#"(<span class="nom-amount"><span class="nom-amount-count mono">{amount}</span>{icon}</span>)"#,
        amount = amount,
        icon = nom_icon_sm(tooltip),
    )
}

/// "Cost: {amount} {icon}" line for judicial / service cards.
#[must_use]
pub fn nom_cost_line_html(c: &Catalog, loc: &str, amount: u16) -> String {
    let tooltip = currency_tooltip(c, loc);
    let prefix = escape_html(&c.t(loc, "noms.cost_prefix"));
    format!(
        r#"<span class="nom-cost-line"><span class="nom-cost-prefix">{prefix}</span> {price}</span>"#,
        price = nom_amount_html(amount, &tooltip),
    )
}

/// Substitute `{nom_price}` in a locale string with amount + icon HTML.
#[must_use]
pub fn locale_with_nom_price(c: &Catalog, loc: &str, key: &str, amount: u16) -> String {
    let tooltip = currency_tooltip(c, loc);
    c.tf(loc, key, &[("nom_price", &nom_amount_html(amount, &tooltip))])
}

/// Substitute `{nom_price}` with parenthetical amount + icon HTML.
#[must_use]
pub fn locale_with_nom_price_paren(c: &Catalog, loc: &str, key: &str, amount: u16) -> String {
    let tooltip = currency_tooltip(c, loc);
    c.tf(loc, key, &[("nom_price", &nom_paren_html(amount, &tooltip))])
}

/// Top bar wallet: clickable `{balance}{icon}` (opens Nom purchase when Stripe is configured).
#[must_use]
pub fn topbar_wallet_html(balance: u16, tooltip: &str, aria_label: &str) -> String {
    format!(
        r#"<button type="button" class="app-topbar-ai-tokens" id="app-topbar-noms-btn" aria-haspopup="dialog" aria-label="{aria}">{amount}</button>"#,
        aria = escape_html(aria_label),
        amount = nom_amount_html(balance, tooltip),
    )
}

/// Plain-text price for JSON errors and native dialogs (no HTML).
#[must_use]
pub fn nom_amount_plain(amount: u16) -> String {
    if amount == 1 {
        "1 Nom".to_string()
    } else {
        format!("{amount} Noms")
    }
}

/// Insufficient-balance message as plain text (`{nom_price}` = "1 Nom" / "32 Noms").
#[must_use]
pub fn insufficient_noms_plain(
    c: &Catalog,
    loc: &str,
    key: &str,
    needed: u16,
    balance: Option<u16>,
) -> String {
    let nom_price = nom_amount_plain(needed);
    match balance {
        Some(b) => c.tf(
            loc,
            key,
            &[
                ("nom_price", &nom_price),
                ("balance", &b.to_string()),
            ],
        ),
        None => c.tf(loc, key, &[("nom_price", &nom_price)]),
    }
}

/// Insufficient-balance copy with `{nom_price}` (+ optional `{balance}`) as HTML.
#[must_use]
pub fn insufficient_noms_html(
    c: &Catalog,
    loc: &str,
    key: &str,
    needed: u16,
    balance: Option<u16>,
) -> String {
    let tooltip = currency_tooltip(c, loc);
    let nom_price = nom_amount_html(needed, &tooltip);
    match balance {
        Some(b) => c.tf(
            loc,
            key,
            &[
                ("nom_price", &nom_price),
                ("balance", &b.to_string()),
            ],
        ),
        None => c.tf(loc, key, &[("nom_price", &nom_price)]),
    }
}

/// Flash alert paragraph; trusted nom-price HTML is passed through unescaped.
#[must_use]
pub fn flash_alert_html(message: &str) -> String {
    if message.contains("nom-amount") {
        format!(r#"<p class="sub" role="alert">{message}</p>"#)
    } else {
        format!(
            r#"<p class="sub" role="alert">{}</p>"#,
            escape_html(message)
        )
    }
}

/// Flash status paragraph (success notices).
#[must_use]
pub fn flash_status_html(message: &str) -> String {
    format!(
        r#"<p class="sub" role="status">{}</p>"#,
        escape_html(message)
    )
}

/// Status or alert flash — nom-price HTML uses alert role.
#[must_use]
pub fn flash_message_html(message: &str) -> String {
    if message.contains("nom-amount") {
        flash_alert_html(message)
    } else {
        flash_status_html(message)
    }
}
