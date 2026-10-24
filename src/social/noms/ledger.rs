//! Append-only Nom ledger (`social_human_ai_token_ledger`) for economics analysis.

use sqlx::{MySql, Transaction};

use crate::pricing::BillableService;
use crate::ref_key_values;
use crate::ref_keys;

/// Reason slug in `ref_key_values` group [`ref_keys::GROUP_NOM_LEDGER_REASON`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerReason {
    CreditSignup,
    CreditDailyLogin,
    CreditJuryDuty,
    CreditPurchase,
    SpendBillableService,
    AdminAdjustment,
}

impl LedgerReason {
    #[must_use]
    pub const fn ref_key(self) -> &'static str {
        match self {
            Self::CreditSignup => ref_keys::KEY_NOM_LEDGER_CREDIT_SIGNUP,
            Self::CreditDailyLogin => ref_keys::KEY_NOM_LEDGER_CREDIT_DAILY_LOGIN,
            Self::CreditJuryDuty => ref_keys::KEY_NOM_LEDGER_CREDIT_JURY_DUTY,
            Self::CreditPurchase => ref_keys::KEY_NOM_LEDGER_CREDIT_PURCHASE,
            Self::SpendBillableService => ref_keys::KEY_NOM_LEDGER_SPEND_BILLABLE_SERVICE,
            Self::AdminAdjustment => ref_keys::KEY_NOM_LEDGER_ADMIN_ADJUSTMENT,
        }
    }
}

/// Record a ledger row inside an existing transaction after the wallet row was updated.
pub async fn append_entry_tx(
    tx: &mut Transaction<'_, MySql>,
    social_entity_id: u64,
    delta: i16,
    balance_after: u16,
    reason: LedgerReason,
    service: Option<BillableService>,
    reference_subject: Option<&str>,
    created_by_user_id: Option<u64>,
) -> Result<(), sqlx::Error> {
    if delta == 0 {
        return Ok(());
    }

    let reason_id = ref_key_values::lookup_active_id_conn(
        tx,
        ref_keys::GROUP_NOM_LEDGER_REASON,
        reason.ref_key(),
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Protocol(format!(
            "nom ledger reason not configured: {}",
            reason.ref_key()
        ))
    })?;

    let service_key = service.map(BillableService::ledger_key);
    let metadata_json = service.map(|s| {
        serde_json::json!({
            "nom_cost": s.nom_cost(),
        })
    });

    sqlx::query(
        r#"
        INSERT INTO social_human_ai_token_ledger (
            social_entity_id,
            delta,
            balance_after,
            reason_id,
            service_key,
            reference_subject,
            metadata_json,
            created_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(social_entity_id)
    .bind(delta)
    .bind(balance_after)
    .bind(reason_id)
    .bind(service_key)
    .bind(reference_subject)
    .bind(metadata_json.map(sqlx::types::Json))
    .bind(created_by_user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
