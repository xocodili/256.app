//! OpenID Connect (Google v1).

pub mod google;
pub mod pending;
pub mod store;

pub use google::{
    build_google_authorize_request, complete_password_link, handle_google_callback,
    GoogleAuthRequest, GoogleCallbackOutcome, RECAPTCHA_ACTION_OIDC_LINK,
    RECAPTCHA_ACTION_OIDC_START,
};
pub use pending::{
    build_oidc_link_cookie, build_oidc_state_cookie, clear_cookie, parse_oidc_link_cookie,
    parse_oidc_state_cookie, OidcLinkPendingPayload, OidcStatePayload, OIDC_LINK_COOKIE,
    OIDC_STATE_COOKIE,
};
pub use store::{find_user_by_provider_subject, user_has_provider, PROVIDER_GOOGLE};
