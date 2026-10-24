//! Fiat Nom pack catalog (single source of truth for purchase SKUs).

use crate::social::noms::NOMS_MAX;

/// SKU: 32 Noms for A$3.00 via Stripe (Civic pack — always available subject to wallet cap).
pub const PACK_32_SKU: &str = "nom_pack_32_aud_v1";
pub const PACK_32_NOMS: u16 = 32;
pub const PACK_32_AMOUNT_CENTS: u32 = 300;
pub const PACK_32_CURRENCY: &str = "aud";
/// No balance-tier gate — only the 256 Nom wallet cap applies.
pub const PACK_32_MAX_BALANCE_EXCLUSIVE: u16 = NOMS_MAX;

/// SKU: 64 Noms for A$5.00 via Stripe (Standard pack).
pub const PACK_64_SKU: &str = "nom_pack_64_aud_v1";
pub const PACK_64_NOMS: u16 = 64;
pub const PACK_64_AMOUNT_CENTS: u32 = 500;
pub const PACK_64_CURRENCY: &str = "aud";
/// No balance-tier gate — only the 256 Nom wallet cap applies.
pub const PACK_64_MAX_BALANCE_EXCLUSIVE: u16 = NOMS_MAX;

/// SKU: 128 Noms for A$8.00 via Stripe (Steward pack).
pub const PACK_128_SKU: &str = "nom_pack_128_aud_v1";
pub const PACK_128_NOMS: u16 = 128;
pub const PACK_128_AMOUNT_CENTS: u32 = 800;
pub const PACK_128_CURRENCY: &str = "aud";
/// Steward tier gate: blocked when balance is already **≥** 128 (wallet cap also applies).
pub const PACK_128_MAX_BALANCE_EXCLUSIVE: u16 = PACK_128_NOMS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NomPack {
    pub sku: &'static str,
    pub noms: u16,
    pub amount_cents: u32,
    pub currency: &'static str,
    pub max_balance_exclusive: u16,
}

const ALL_PACKS: [NomPack; 3] = [
    NomPack {
        sku: PACK_32_SKU,
        noms: PACK_32_NOMS,
        amount_cents: PACK_32_AMOUNT_CENTS,
        currency: PACK_32_CURRENCY,
        max_balance_exclusive: PACK_32_MAX_BALANCE_EXCLUSIVE,
    },
    NomPack {
        sku: PACK_64_SKU,
        noms: PACK_64_NOMS,
        amount_cents: PACK_64_AMOUNT_CENTS,
        currency: PACK_64_CURRENCY,
        max_balance_exclusive: PACK_64_MAX_BALANCE_EXCLUSIVE,
    },
    NomPack {
        sku: PACK_128_SKU,
        noms: PACK_128_NOMS,
        amount_cents: PACK_128_AMOUNT_CENTS,
        currency: PACK_128_CURRENCY,
        max_balance_exclusive: PACK_128_MAX_BALANCE_EXCLUSIVE,
    },
];

impl NomPack {
    #[must_use]
    pub fn all() -> &'static [Self] {
        &ALL_PACKS
    }

    #[must_use]
    pub const fn pack_civic_aud() -> Self {
        Self::pack_32_aud()
    }

    /// Civic pack (32 Noms / A$3).
    #[must_use]
    pub const fn pack_32_aud() -> Self {
        Self {
            sku: PACK_32_SKU,
            noms: PACK_32_NOMS,
            amount_cents: PACK_32_AMOUNT_CENTS,
            currency: PACK_32_CURRENCY,
            max_balance_exclusive: PACK_32_MAX_BALANCE_EXCLUSIVE,
        }
    }

    #[must_use]
    pub const fn pack_64_aud() -> Self {
        Self {
            sku: PACK_64_SKU,
            noms: PACK_64_NOMS,
            amount_cents: PACK_64_AMOUNT_CENTS,
            currency: PACK_64_CURRENCY,
            max_balance_exclusive: PACK_64_MAX_BALANCE_EXCLUSIVE,
        }
    }

    #[must_use]
    pub const fn pack_128_aud() -> Self {
        Self {
            sku: PACK_128_SKU,
            noms: PACK_128_NOMS,
            amount_cents: PACK_128_AMOUNT_CENTS,
            currency: PACK_128_CURRENCY,
            max_balance_exclusive: PACK_128_MAX_BALANCE_EXCLUSIVE,
        }
    }

    #[must_use]
    pub fn find(sku: &str) -> Option<Self> {
        Self::all().iter().copied().find(|p| p.sku == sku)
    }

    /// Why checkout is blocked at this balance (when [`Self::can_purchase_at_balance`] is false).
    #[must_use]
    pub const fn purchase_block_reason(self, balance: u16) -> Option<PurchaseBlockReason> {
        if balance.saturating_add(self.noms) > NOMS_MAX {
            return Some(PurchaseBlockReason::WalletCap);
        }
        if self.noms == PACK_128_NOMS && balance >= PACK_128_NOMS {
            return Some(PurchaseBlockReason::StewardTier);
        }
        None
    }

    /// `true` when the user may start checkout for this pack at the given balance.
    #[must_use]
    pub const fn can_purchase_at_balance(self, balance: u16) -> bool {
        self.purchase_block_reason(balance).is_none()
    }
}

/// Nom pack checkout blocked — surfaced in purchase modal copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseBlockReason {
    /// `balance + pack_noms` would exceed [`NOMS_MAX`].
    WalletCap,
    /// Steward (128) pack not offered once the wallet already holds a full steward tier.
    StewardTier,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_civic_always_available_subject_to_wallet_cap() {
        let pack = NomPack::pack_civic_aud();
        assert!(pack.can_purchase_at_balance(0));
        assert!(pack.can_purchase_at_balance(32));
        assert!(pack.can_purchase_at_balance(224));
        assert!(!pack.can_purchase_at_balance(225));
    }

    #[test]
    fn pack_64_available_until_wallet_cap() {
        let pack = NomPack::pack_64_aud();
        assert!(pack.can_purchase_at_balance(63));
        assert!(pack.can_purchase_at_balance(64));
        assert!(pack.can_purchase_at_balance(128));
        assert!(pack.can_purchase_at_balance(192));
        assert!(!pack.can_purchase_at_balance(193));
        assert_eq!(
            pack.purchase_block_reason(193),
            Some(PurchaseBlockReason::WalletCap)
        );
    }

    #[test]
    fn pack_64_fits_wallet_cap() {
        let pack = NomPack::pack_64_aud();
        assert!(192 + pack.noms <= NOMS_MAX);
    }

    #[test]
    fn pack_128_blocked_at_128_noms() {
        let pack = NomPack::pack_128_aud();
        assert!(pack.can_purchase_at_balance(127));
        assert!(!pack.can_purchase_at_balance(128));
        assert_eq!(
            pack.purchase_block_reason(128),
            Some(PurchaseBlockReason::StewardTier)
        );
    }

    #[test]
    fn pack_128_fits_wallet_cap() {
        let pack = NomPack::pack_128_aud();
        let balance = pack.max_balance_exclusive - 1;
        assert!(balance + pack.noms <= NOMS_MAX);
    }

    #[test]
    fn find_returns_all_skus() {
        assert_eq!(NomPack::find(PACK_32_SKU), Some(NomPack::pack_32_aud()));
        assert_eq!(NomPack::find(PACK_64_SKU), Some(NomPack::pack_64_aud()));
        assert_eq!(NomPack::find(PACK_128_SKU), Some(NomPack::pack_128_aud()));
        assert_eq!(NomPack::find("unknown"), None);
    }
}
