//! Human login-inactivity penalty phases (communities do not time out via post idle).

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::social::judicial::{civic_rating_delta_times, jury_civic_rating_delta};

/// Days without sign-in before the warning email (no rating change).
pub const INACTIVITY_WARNING_IDLE_DAYS: i64 = 21;
/// Days without sign-in before daily decrements begin (7-day buffer after warning).
pub const INACTIVITY_DECREMENT_START_IDLE_DAYS: i64 = 28;
/// Decrement-period days at 1×δ per day.
pub const INACTIVITY_PHASE_1_DECREMENT_DAYS: u32 = 21;
/// Decrement-period days at 3×δ per day (after phase 1).
pub const INACTIVITY_PHASE_2_DECREMENT_DAYS: u32 = 21;
/// Max decrement days applied per scheduler tick per user.
pub const INACTIVITY_MAX_PER_TICK: u32 = 8;

/// Back-compat alias used by profile notice gating.
pub const INACTIVITY_FIRST_PENALTY_DAYS: i64 = INACTIVITY_WARNING_IDLE_DAYS;

/// Decrement-period day number (1-based) once idle ≥ [`INACTIVITY_DECREMENT_START_IDLE_DAYS`].
#[must_use]
pub fn decrement_day_for_idle(idle_days: i64) -> Option<u32> {
    if idle_days < INACTIVITY_DECREMENT_START_IDLE_DAYS {
        None
    } else {
        Some(
            (idle_days - INACTIVITY_DECREMENT_START_IDLE_DAYS + 1)
                .try_into()
                .unwrap_or(u32::MAX),
        )
    }
}

/// Integer multiplier (1, 3, or 6) for a 1-based decrement-period day.
#[must_use]
pub fn multiplier_for_decrement_day(decrement_day: u32) -> i64 {
    if decrement_day <= INACTIVITY_PHASE_1_DECREMENT_DAYS {
        1
    } else if decrement_day <= INACTIVITY_PHASE_1_DECREMENT_DAYS + INACTIVITY_PHASE_2_DECREMENT_DAYS {
        3
    } else {
        6
    }
}

/// Penalty magnitude for a single decrement-period day.
#[must_use]
pub fn penalty_for_decrement_day(decrement_day: u32) -> Decimal {
    -civic_rating_delta_times(multiplier_for_decrement_day(decrement_day))
}

/// Total decrement-period days owed for current idle time.
#[must_use]
pub fn expected_decrement_days_applied(idle_days: i64) -> u32 {
    decrement_day_for_idle(idle_days).unwrap_or(0)
}

/// How many decrement days are still owed given idle time and prior applications.
#[must_use]
pub fn inactivity_decrement_days_to_apply(idle_days: i64, decrement_days_applied: u32) -> u32 {
    let expected = expected_decrement_days_applied(idle_days);
    if expected <= decrement_days_applied {
        0
    } else {
        (expected - decrement_days_applied).min(INACTIVITY_MAX_PER_TICK)
    }
}

/// Simulate remaining decrement days until `rating` reaches zero from a decrement start.
#[must_use]
pub fn decrement_days_until_rating_zero(from_rating: Decimal, starting_decrement_day: u32) -> u32 {
    if from_rating <= Decimal::ZERO {
        return 0;
    }
    let mut rating = from_rating;
    let mut day = starting_decrement_day;
    let mut steps = 0u32;
    while rating > Decimal::ZERO && steps < 10_000 {
        let penalty = penalty_for_decrement_day(day);
        rating += penalty;
        day = day.saturating_add(1);
        steps += 1;
    }
    steps
}

/// Wall-clock date when phased decrements would drive `rating` to zero (UTC calendar date).
#[must_use]
pub fn estimated_deletion_at_from_last_login(
    last_login_at: chrono::NaiveDateTime,
    rating: Decimal,
) -> Option<chrono::NaiveDateTime> {
    if rating <= Decimal::ZERO {
        return None;
    }
    let start_day = expected_decrement_days_applied(INACTIVITY_DECREMENT_START_IDLE_DAYS);
    if start_day == 0 {
        return None;
    }
    let steps = decrement_days_until_rating_zero(rating, 1);
    if steps == 0 {
        return None;
    }
    let days = INACTIVITY_DECREMENT_START_IDLE_DAYS + i64::from(steps.saturating_sub(1));
    Some(last_login_at + chrono::Duration::days(days))
}

/// Format a deletion estimate for email and profile copy (UTC calendar date).
#[must_use]
pub fn format_estimated_deletion_date(at: chrono::NaiveDateTime) -> String {
    at.format("%Y-%m-%d").to_string()
}

/// Legacy helper: weekly steps at basal δ (used only where phased logic is not applicable).
#[must_use]
pub fn penalties_until_rating_zero(rating: Decimal) -> u32 {
    if rating <= Decimal::ZERO {
        return 0;
    }
    let delta = jury_civic_rating_delta();
    let steps = (rating / delta).ceil();
    steps.to_u32().unwrap_or(u32::MAX).max(1)
}
