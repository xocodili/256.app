//! Fiat Nom pack purchases (Stripe PaymentIntent → wallet credit).

use sqlx::{MySql, MySqlPool, Transaction};

use crate::nom_packs::NomPack;
use crate::social::human_onboarding;
use crate::social::noms::{self, ledger::LedgerReason, NOMS_MAX};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseStatus {
    Pending,
    Completed,
    Failed,
}

impl PurchaseStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    fn from_db(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PurchaseRow {
    pub id: u64,
    pub user_id: u64,
    pub social_entity_id: u64,
    pub sku: String,
    pub noms_granted: u16,
    pub amount_cents: u32,
    pub currency: String,
    pub stripe_payment_intent_id: String,
    pub status: PurchaseStatus,
    pub balance_after: Option<u16>,
}

#[derive(Debug)]
pub enum PurchaseError {
    PackNotFound,
    WalletFull { balance: u16, max_exclusive: u16 },
    NoHumanEntity,
    Db(sqlx::Error),
    AlreadyCompleted,
    NotPending,
    CapExceeded { balance: u16, grant: u16 },
    PendingPurchaseExists,
}

/// Outcome of webhook / sync fulfillment (distinct from checkout validation errors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FulfillOutcome {
    Credited(u16),
    AlreadyCompleted(u16),
    NoMatchingPurchase,
    RejectedAfterPayment(PurchaseRejectReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurchaseRejectReason {
    WalletFull { balance: u16, max_exclusive: u16 },
    CapExceeded { balance: u16, grant: u16 },
}

impl std::fmt::Display for PurchaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PackNotFound => write!(f, "unknown nom pack sku"),
            Self::WalletFull { balance, max_exclusive } => {
                write!(f, "wallet balance {balance} must be below {max_exclusive} to buy")
            }
            Self::NoHumanEntity => write!(f, "human entity not found"),
            Self::Db(e) => write!(f, "{e}"),
            Self::AlreadyCompleted => write!(f, "purchase already completed"),
            Self::NotPending => write!(f, "purchase not pending"),
            Self::CapExceeded { balance, grant } => {
                write!(f, "credit {grant} would exceed cap from balance {balance}")
            }
            Self::PendingPurchaseExists => {
                write!(f, "a Nom purchase checkout is already in progress")
            }
        }
    }
}

impl From<sqlx::Error> for PurchaseError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

pub async fn user_has_pending_purchase(pool: &MySqlPool, user_id: u64) -> Result<bool, PurchaseError> {
    let row: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id FROM nom_purchase
        WHERE user_id = ? AND status = ?
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(PurchaseStatus::Pending.as_str())
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// Mark every in-progress checkout for this user as failed (pack switch / fresh intent).
/// Returns Stripe PaymentIntent ids so the caller can cancel them at Stripe.
pub async fn abandon_all_pending_purchases_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Vec<String>, PurchaseError> {
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

    let pi_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT stripe_payment_intent_id
        FROM nom_purchase
        WHERE user_id = ? AND status = ?
        "#,
    )
    .bind(user_id)
    .bind(PurchaseStatus::Pending.as_str())
    .fetch_all(&mut *tx)
    .await?;

    if !pi_ids.is_empty() {
        sqlx::query(
            r#"
            UPDATE nom_purchase
            SET status = ?
            WHERE user_id = ? AND status = ?
            "#,
        )
        .bind(PurchaseStatus::Failed.as_str())
        .bind(user_id)
        .bind(PurchaseStatus::Pending.as_str())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(pi_ids)
}

pub async fn insert_pending_purchase(
    pool: &MySqlPool,
    user_id: u64,
    social_entity_id: u64,
    pack: NomPack,
    stripe_payment_intent_id: &str,
) -> Result<u64, PurchaseError> {
    let mut tx = pool.begin().await?;

    // Serialize concurrent checkout attempts per user.
    sqlx::query("SELECT id FROM users WHERE id = ? LIMIT 1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

    let pending_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id FROM nom_purchase
        WHERE user_id = ? AND status = ?
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(PurchaseStatus::Pending.as_str())
    .fetch_optional(&mut *tx)
    .await?;
    if pending_id.is_some() {
        tx.rollback().await?;
        return Err(PurchaseError::PendingPurchaseExists);
    }

    let balance: u16 = sqlx::query_scalar(
        "SELECT ai_tokens FROM social_human WHERE social_entity_id = ? LIMIT 1",
    )
    .bind(social_entity_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0);

    if !pack.can_purchase_at_balance(balance) {
        tx.rollback().await?;
        return Err(PurchaseError::WalletFull {
            balance,
            max_exclusive: pack.max_balance_exclusive,
        });
    }

    let res = sqlx::query(
        r#"
        INSERT INTO nom_purchase (
            user_id, social_entity_id, sku, noms_granted, amount_cents, currency,
            stripe_payment_intent_id, status
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(social_entity_id)
    .bind(pack.sku)
    .bind(pack.noms)
    .bind(pack.amount_cents)
    .bind(pack.currency)
    .bind(stripe_payment_intent_id)
    .bind(PurchaseStatus::Pending.as_str())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(res.last_insert_id())
}

pub async fn load_purchase_by_id(
    pool: &MySqlPool,
    purchase_id: u64,
    user_id: u64,
) -> Result<Option<PurchaseRow>, PurchaseError> {
    let row = sqlx::query_as::<_, (
        u64,
        u64,
        u64,
        String,
        u16,
        u32,
        String,
        String,
        String,
        Option<u16>,
    )>(
        r#"
        SELECT
            id, user_id, social_entity_id, sku, noms_granted, amount_cents, currency,
            stripe_payment_intent_id, status, balance_after
        FROM nom_purchase
        WHERE id = ? AND user_id = ?
        LIMIT 1
        "#,
    )
    .bind(purchase_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_purchase_row))
}

pub async fn load_purchase_by_payment_intent(
    pool: &MySqlPool,
    stripe_payment_intent_id: &str,
) -> Result<Option<PurchaseRow>, PurchaseError> {
    let row = sqlx::query_as::<_, (
        u64,
        u64,
        u64,
        String,
        u16,
        u32,
        String,
        String,
        String,
        Option<u16>,
    )>(
        r#"
        SELECT
            id, user_id, social_entity_id, sku, noms_granted, amount_cents, currency,
            stripe_payment_intent_id, status, balance_after
        FROM nom_purchase
        WHERE stripe_payment_intent_id = ?
        LIMIT 1
        "#,
    )
    .bind(stripe_payment_intent_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_purchase_row))
}

fn map_purchase_row(
    row: (
        u64,
        u64,
        u64,
        String,
        u16,
        u32,
        String,
        String,
        String,
        Option<u16>,
    ),
) -> PurchaseRow {
    let status = PurchaseStatus::from_db(&row.8).unwrap_or(PurchaseStatus::Pending);
    PurchaseRow {
        id: row.0,
        user_id: row.1,
        social_entity_id: row.2,
        sku: row.3,
        noms_granted: row.4,
        amount_cents: row.5,
        currency: row.6,
        stripe_payment_intent_id: row.7,
        status,
        balance_after: row.9,
    }
}

/// Abandon a pending checkout owned by `user_id`. Returns the Stripe PI id when a row was
/// transitioned from `pending` → `failed` (caller may cancel the PI at Stripe).
pub async fn cancel_pending_purchase_by_id(
    pool: &MySqlPool,
    purchase_id: u64,
    user_id: u64,
) -> Result<Option<String>, PurchaseError> {
    let Some(row) = load_purchase_by_id(pool, purchase_id, user_id).await? else {
        return Ok(None);
    };
    if row.status != PurchaseStatus::Pending {
        return Ok(None);
    }
    mark_purchase_failed_by_payment_intent(pool, &row.stripe_payment_intent_id).await?;
    Ok(Some(row.stripe_payment_intent_id))
}

pub async fn mark_purchase_failed_by_payment_intent(
    pool: &MySqlPool,
    stripe_payment_intent_id: &str,
) -> Result<(), PurchaseError> {
    sqlx::query(
        r#"
        UPDATE nom_purchase
        SET status = ?
        WHERE stripe_payment_intent_id = ?
          AND status = ?
        "#,
    )
    .bind(PurchaseStatus::Failed.as_str())
    .bind(stripe_payment_intent_id)
    .bind(PurchaseStatus::Pending.as_str())
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_purchase_failed_tx(
    tx: &mut Transaction<'_, MySql>,
    purchase_id: u64,
) -> Result<(), PurchaseError> {
    sqlx::query(
        r#"
        UPDATE nom_purchase
        SET status = ?
        WHERE id = ?
          AND status = ?
        "#,
    )
    .bind(PurchaseStatus::Failed.as_str())
    .bind(purchase_id)
    .bind(PurchaseStatus::Pending.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Credit Noms when Stripe reports `payment_intent.succeeded`. Idempotent per PI id.
pub async fn fulfill_purchase_by_payment_intent(
    pool: &MySqlPool,
    stripe_payment_intent_id: &str,
) -> Result<FulfillOutcome, PurchaseError> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query_as::<_, (
        u64,
        u64,
        u64,
        u16,
        String,
        String,
    )>(
        r#"
        SELECT id, user_id, social_entity_id, noms_granted, status, sku
        FROM nom_purchase
        WHERE stripe_payment_intent_id = ?
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(stripe_payment_intent_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((purchase_id, user_id, entity_id, noms_granted, status, sku)) = row else {
        tx.commit().await?;
        return Ok(FulfillOutcome::NoMatchingPurchase);
    };

    if status == PurchaseStatus::Completed.as_str() {
        let balance: u16 = sqlx::query_scalar(
            "SELECT balance_after FROM nom_purchase WHERE id = ? LIMIT 1",
        )
        .bind(purchase_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
        tx.commit().await?;
        return Ok(FulfillOutcome::AlreadyCompleted(balance));
    }

    if status != PurchaseStatus::Pending.as_str() {
        tx.commit().await?;
        return Err(PurchaseError::NotPending);
    }

    let balance: u16 = sqlx::query_scalar(
        "SELECT ai_tokens FROM social_human WHERE social_entity_id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(entity_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0);

    let pack = NomPack::find(&sku).ok_or(PurchaseError::PackNotFound)?;
    if !pack.can_purchase_at_balance(balance) {
        mark_purchase_failed_tx(&mut tx, purchase_id).await?;
        tx.commit().await?;
        tracing::error!(
            purchase_id,
            user_id,
            entity_id,
            balance,
            stripe_payment_intent_id,
            "nom purchase paid but wallet no longer eligible — manual Stripe refund required"
        );
        return Ok(FulfillOutcome::RejectedAfterPayment(
            PurchaseRejectReason::WalletFull {
                balance,
                max_exclusive: pack.max_balance_exclusive,
            },
        ));
    }

    let new_balance = balance.saturating_add(noms_granted);
    if new_balance > NOMS_MAX {
        mark_purchase_failed_tx(&mut tx, purchase_id).await?;
        tx.commit().await?;
        tracing::error!(
            purchase_id,
            user_id,
            entity_id,
            balance,
            noms_granted,
            stripe_payment_intent_id,
            "nom purchase paid but would exceed wallet cap — manual Stripe refund required"
        );
        return Ok(FulfillOutcome::RejectedAfterPayment(
            PurchaseRejectReason::CapExceeded {
                balance,
                grant: noms_granted,
            },
        ));
    }

    sqlx::query(
        "UPDATE social_human SET ai_tokens = ? WHERE social_entity_id = ?",
    )
    .bind(new_balance)
    .bind(entity_id)
    .execute(&mut *tx)
    .await?;

    let reference = format!("stripe:{stripe_payment_intent_id}");
    credit_purchase_ledger_tx(
        &mut tx,
        entity_id,
        noms_granted,
        new_balance,
        &reference,
        user_id,
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE nom_purchase
        SET status = ?, balance_after = ?, completed_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(PurchaseStatus::Completed.as_str())
    .bind(new_balance)
    .bind(purchase_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        purchase_id,
        user_id,
        entity_id,
        noms_granted,
        balance = new_balance,
        stripe_payment_intent_id,
        "nom purchase fulfilled"
    );

    Ok(FulfillOutcome::Credited(new_balance))
}

async fn credit_purchase_ledger_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_id: u64,
    credited: u16,
    balance_after: u16,
    reference_subject: &str,
    user_id: u64,
) -> Result<(), PurchaseError> {
    crate::social::noms::ledger::append_entry_tx(
        tx,
        entity_id,
        credited as i16,
        balance_after,
        LedgerReason::CreditPurchase,
        None,
        Some(reference_subject),
        Some(user_id),
    )
    .await?;
    Ok(())
}

pub async fn purchase_config_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<(u16, bool), PurchaseError> {
    let entity_id = human_onboarding::primary_human_entity_id(pool, user_id)
        .await?
        .ok_or(PurchaseError::NoHumanEntity)?;
    let balance = noms::balance_for_human_entity(pool, entity_id).await?;
    let pending_checkout = user_has_pending_purchase(pool, user_id).await?;
    Ok((balance, pending_checkout))
}
