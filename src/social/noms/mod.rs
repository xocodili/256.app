//! Public-facing **Noms** balance, stored in `social_human.ai_tokens` for now.
//!
//! Pricing for any Nom-debiting service lives in [`crate::pricing`]. Handlers
//! should never inline a `SET ai_tokens = ai_tokens - N` query: route the spend
//! through [`spend_for_service`] (or [`spend_for_service_tx`] when the spend
//! must be part of a wider transaction) so the schedule remains the single
//! source of truth. Successful debits also set `social_entity.modified_by`
//! for the spending human (wallet rows live on `social_human`).
//!
//! All balance changes that should appear in economics analysis are recorded via
//! `ledger::append_entry_tx` inside spend/credit paths here.

pub mod ledger;
pub mod purchase;

use chrono::NaiveDate;
use sqlx::MySqlPool;

use crate::pricing::BillableService;
use crate::social::audit;

pub use ledger::LedgerReason;

/// Maximum Noms stored in `social_human.ai_tokens`.
pub const NOMS_MAX: u16 = 256;
/// Noms credited when a new human profile (`social_human`) is created at signup onboarding.
pub const SIGNUP_NOM_GRANT: u16 = 32;
/// Noms credited on the user's first successful sign-in each UTC calendar day.
pub const DAILY_LOGIN_NOM_GRANT: u16 = 1;
/// Noms credited when a juror votes and the case resolves (once per invite).
pub const JURY_DUTY_NOM_GRANT: u16 = 3;

/// Result of attempting to charge a user for a [`BillableService`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendOutcome {
    /// The service was non-free, the wallet had enough, and the cost was
    /// deducted. `balance` is the post-spend balance.
    Spent { cost: u16, balance: u16 },
    /// The service is free (cost of zero). No wallet write happened.
    /// `balance` is the current balance.
    NoCharge { balance: u16 },
    /// The service is non-free and the wallet did not have enough. No
    /// wallet write happened.
    InsufficientBalance { needed: u16, balance: u16 },
}

impl SpendOutcome {
    /// `true` for `Spent` and `NoCharge`. `false` for `InsufficientBalance`.
    #[must_use]
    pub const fn is_paid(self) -> bool {
        matches!(
            self,
            SpendOutcome::Spent { .. } | SpendOutcome::NoCharge { .. }
        )
    }

}

/// Charges the human entity for the given service inside an existing
/// transaction. The caller is responsible for committing or rolling back.
///
/// Returns a [`SpendOutcome`] reflecting what happened — callers should match
/// on it to decide whether to continue with the action (`Spent` or `NoCharge`)
/// or surface an insufficient-balance error (`InsufficientBalance`).
pub async fn spend_for_service_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    entity_id: u64,
    item: BillableService,
    modified_by_user_id: u64,
    reference_subject: Option<&str>,
) -> Result<SpendOutcome, sqlx::Error> {
    let cost = item.nom_cost();

    let current: Option<u16> =
        sqlx::query_scalar("SELECT ai_tokens FROM social_human WHERE social_entity_id = ? LIMIT 1")
            .bind(entity_id)
            .fetch_optional(&mut **tx)
            .await?;
    let balance = current.unwrap_or(0);

    if cost == 0 {
        return Ok(SpendOutcome::NoCharge { balance });
    }
    if balance < cost {
        return Ok(SpendOutcome::InsufficientBalance {
            needed: cost,
            balance,
        });
    }

    let res = sqlx::query(
        r#"
        UPDATE social_human
        SET ai_tokens = ai_tokens - ?
        WHERE social_entity_id = ?
          AND ai_tokens >= ?
        "#,
    )
    .bind(cost)
    .bind(entity_id)
    .bind(cost)
    .execute(&mut **tx)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(SpendOutcome::InsufficientBalance {
            needed: cost,
            balance,
        });
    }

    let balance_after = balance - cost;
    audit::touch_social_entity_modified_by_tx(tx, entity_id, modified_by_user_id).await?;

    ledger::append_entry_tx(
        tx,
        entity_id,
        -(cost as i16),
        balance_after,
        ledger::LedgerReason::SpendBillableService,
        Some(item),
        reference_subject,
        Some(modified_by_user_id),
    )
    .await?;

    Ok(SpendOutcome::Spent {
        cost,
        balance: balance_after,
    })
}

/// Current Nom wallet balance for a human `social_entity.id`.
pub async fn balance_for_human_entity(
    pool: &MySqlPool,
    human_entity_id: u64,
) -> Result<u16, sqlx::Error> {
    sqlx::query_scalar(
        r#"SELECT ai_tokens FROM social_human WHERE social_entity_id = ? LIMIT 1"#,
    )
    .bind(human_entity_id)
    .fetch_optional(pool)
    .await
    .map(|v| v.unwrap_or(0))
}

/// Outcome of a non-purchase Nom accrual attempt (signup uses a dedicated path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccrualOutcome {
    Credited { balance: u16 },
    AlreadyClaimed,
    WalletFull { balance: u16 },
}

/// Credits up to `amount` Noms without exceeding [`NOMS_MAX`].
async fn credit_capped_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    human_entity_id: u64,
    amount: u16,
    reason: LedgerReason,
    reference_subject: Option<&str>,
    created_by_user_id: Option<u64>,
) -> Result<AccrualOutcome, sqlx::Error> {
    let balance: Option<u16> =
        sqlx::query_scalar("SELECT ai_tokens FROM social_human WHERE social_entity_id = ? LIMIT 1")
            .bind(human_entity_id)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(balance) = balance else {
        return Ok(AccrualOutcome::WalletFull { balance: 0 });
    };

    let headroom = NOMS_MAX.saturating_sub(balance);
    if headroom == 0 {
        return Ok(AccrualOutcome::WalletFull { balance });
    }
    let credit = amount.min(headroom);

    let res = sqlx::query(
        r#"
        UPDATE social_human
        SET ai_tokens = ai_tokens + ?
        WHERE social_entity_id = ?
          AND ai_tokens + ? <= ?
        "#,
    )
    .bind(credit)
    .bind(human_entity_id)
    .bind(credit)
    .bind(NOMS_MAX)
    .execute(&mut **tx)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(AccrualOutcome::WalletFull { balance });
    }

    let balance_after = balance + credit;
    if let Some(uid) = created_by_user_id {
        audit::touch_social_entity_modified_by_tx(tx, human_entity_id, uid).await?;
    }

    ledger::append_entry_tx(
        tx,
        human_entity_id,
        credit as i16,
        balance_after,
        reason,
        None,
        reference_subject,
        created_by_user_id,
    )
    .await?;

    Ok(AccrualOutcome::Credited {
        balance: balance_after,
    })
}

/// First sign-in each UTC day: +1 Nom (deduped on `users.nom_daily_credit_utc_date`).
pub async fn try_credit_daily_login_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    user_id: u64,
    human_entity_id: u64,
    utc_date: NaiveDate,
) -> Result<AccrualOutcome, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE users
        SET nom_daily_credit_utc_date = ?
        WHERE id = ?
          AND (nom_daily_credit_utc_date IS NULL OR nom_daily_credit_utc_date < ?)
        "#,
    )
    .bind(utc_date)
    .bind(user_id)
    .bind(utc_date)
    .execute(&mut **tx)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(AccrualOutcome::AlreadyClaimed);
    }

    let reference = format!("daily_login:{utc_date}");
    credit_capped_tx(
        tx,
        human_entity_id,
        DAILY_LOGIN_NOM_GRANT,
        LedgerReason::CreditDailyLogin,
        Some(&reference),
        Some(user_id),
    )
    .await
}

/// Juror voted and case resolved: +3 Noms once per invite (`nom_duty_credited_at`).
pub async fn try_credit_jury_duty_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    invite_id: u64,
    human_entity_id: u64,
    case_id: u64,
    credited_by_user_id: Option<u64>,
) -> Result<AccrualOutcome, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite
        SET nom_duty_credited_at = UTC_TIMESTAMP(6)
        WHERE id = ?
          AND nom_duty_credited_at IS NULL
        "#,
    )
    .bind(invite_id)
    .execute(&mut **tx)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(AccrualOutcome::AlreadyClaimed);
    }

    let reference = format!("judicial_case:{case_id}");
    credit_capped_tx(
        tx,
        human_entity_id,
        JURY_DUTY_NOM_GRANT,
        LedgerReason::CreditJuryDuty,
        Some(&reference),
        credited_by_user_id,
    )
    .await
}
