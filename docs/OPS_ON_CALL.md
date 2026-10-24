# Operator on-call — closed beta

**Status:** Shipped (2026-08-12). Single-operator closed beta; multi-operator rotation remains backlog.

---

## Primary on-call (v1)

| Role | Handle | Notes |
|------|--------|--------|
| **Primary on-call** | Human **@sphinx** | Seed Panoply operator; verified `@panoply` clearance |
| **Console** | `GET /home/ops` | Emergency filing, civic filing, vacate, eject, session kill, domain block |
| **Rotation** | Single operator | @sphinx until roster expands — no UI rotation in v1 |

---

## Response expectations

| Channel | Target (closed beta) |
|---------|----------------------|
| **In-app first** | Emergency safety filing (`emergency_safety`) for immediate harm; Panoply console for operator override |
| **Platform safety email** | `PLATFORM_SAFETY_CONTACT_EMAIL` (default `safety@example.com`) — **24h best-effort** on business days; **not a legal SLA** |
| **Digest** | `panoply_daily_status` email to verified `@panoply` operators |

Public Safety legal card surfaces `{safety_contact_email}` from config — do not expose `@panoply` roster or operator handles in guest-facing copy.

---

## Escalation (manual v1)

- **Legal counsel / law enforcement / NCMEC CyberTipline** — manual; preserve logs; never view Class A/B raw media in ops workflows.
- **CyberTipline API automation** — backlog (not shipped).
- **PhotoDNA / Thorn** — backlog; Class A/B gates apply to **avatar/logo ingress only**, not link-card previews.

---

## Related

- [`docs/TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) — tiered response model
- [`docs/PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Panoply operator console, § Outbound links & domain policy
- [`AGENTS.md`](../AGENTS.md) — `@panoply` operator capabilities
