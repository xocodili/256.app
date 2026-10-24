#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::super::inactivity_batch::{
        decrement_day_for_idle, estimated_deletion_at_from_last_login,
        expected_decrement_days_applied, format_estimated_deletion_date,
        inactivity_decrement_days_to_apply, multiplier_for_decrement_day,
        penalty_for_decrement_day, INACTIVITY_DECREMENT_START_IDLE_DAYS,
        INACTIVITY_PHASE_1_DECREMENT_DAYS, INACTIVITY_WARNING_IDLE_DAYS,
    };
    use crate::social::judicial::jury_civic_rating_delta;

    #[test]
    fn decrement_days_start_at_idle_28() {
        assert_eq!(decrement_day_for_idle(INACTIVITY_WARNING_IDLE_DAYS), None);
        assert_eq!(
            decrement_day_for_idle(INACTIVITY_DECREMENT_START_IDLE_DAYS),
            Some(1)
        );
        assert_eq!(expected_decrement_days_applied(30), 3);
    }

    #[test]
    fn phased_multipliers_follow_decrement_day() {
        assert_eq!(multiplier_for_decrement_day(1), 1);
        assert_eq!(multiplier_for_decrement_day(INACTIVITY_PHASE_1_DECREMENT_DAYS), 1);
        use super::super::inactivity_batch::INACTIVITY_PHASE_2_DECREMENT_DAYS;
        assert_eq!(
            multiplier_for_decrement_day(INACTIVITY_PHASE_1_DECREMENT_DAYS + 1),
            3
        );
        assert_eq!(
            multiplier_for_decrement_day(
                INACTIVITY_PHASE_1_DECREMENT_DAYS + INACTIVITY_PHASE_2_DECREMENT_DAYS + 1
            ),
            6
        );
    }

    #[test]
    fn penalty_magnitude_uses_basal_delta() {
        let d = jury_civic_rating_delta();
        assert_eq!(penalty_for_decrement_day(1), -d);
        assert_eq!(penalty_for_decrement_day(22), -(d * Decimal::from(3i64)));
    }

    #[test]
    fn owed_decrement_days_respect_applied_count() {
        assert_eq!(inactivity_decrement_days_to_apply(35, 0), 8);
        assert_eq!(inactivity_decrement_days_to_apply(35, 7), 1);
        assert_eq!(inactivity_decrement_days_to_apply(20, 0), 0);
    }

    #[test]
    fn deletion_estimate_uses_phased_daily_decrements() {
        let login = chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let rating = Decimal::new(210, 2);
        let at = estimated_deletion_at_from_last_login(login, rating);
        assert!(at.is_some());
        assert_eq!(
            format_estimated_deletion_date(at.unwrap()),
            at.unwrap().format("%Y-%m-%d").to_string()
        );
    }
}
