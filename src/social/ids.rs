//! Parse unsigned id strings from JSON (`"123"`) into `u64` with consistent i18n errors.

use crate::error::{AppError, AppResult};
use crate::i18n;

/// `invalid_id_i18n_key` must be a catalog key whose value includes `{field}` (e.g. `social_entity.invalid_id`).
pub fn parse_u64(raw: &str, field: &'static str, invalid_id_i18n_key: &str) -> AppResult<u64> {
    let t = raw.trim();
    let loc = i18n::request_locale();
    if t.is_empty() {
        return Err(AppError::BadRequest(i18n::catalog().tf(
            &loc,
            invalid_id_i18n_key,
            &[("field", field)],
        )));
    }
    t.parse::<u64>().map_err(|_| {
        AppError::BadRequest(i18n::catalog().tf(&loc, invalid_id_i18n_key, &[("field", field)]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_u64_trims() {
        let _ = i18n::catalog();
        let v = parse_u64(" 42 ", "x", "social_entity.invalid_id").unwrap();
        assert_eq!(v, 42);
    }
}
