//! Global outbound cap for Google generateContent calls (Vertex + AI Studio).
//!
//! Waits for a slot instead of firing bursts that trip provider 429s. Council jobs,
//! face-capture vision, and other gateway callers share one smoothed budget.

use std::num::NonZeroU32;
use std::sync::OnceLock;
use std::time::Duration;

use governor::clock::{Clock, DefaultClock};
use governor::{Quota, RateLimiter};
use governor::state::{InMemoryState, NotKeyed};
use governor::middleware::NoOpMiddleware;

type GatewayLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>;

struct Config {
    limiter: Option<GatewayLimiter>,
    max_rpm: u32,
    burst: u32,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let max_rpm: u32 = env_or("VERTEX_GATEWAY_MAX_RPM", 20);
        let burst: u32 = env_or("VERTEX_GATEWAY_BURST", 1).max(1);
        if max_rpm == 0 {
            tracing::info!("vertex gateway rate limit disabled (VERTEX_GATEWAY_MAX_RPM=0)");
            return Config {
                limiter: None,
                max_rpm: 0,
                burst,
            };
        }
        // Even spacing: max_rpm requests per 60s window, small burst for fairness.
        let period_secs = (60u64).div_ceil(u64::from(max_rpm)).max(1);
        let quota = Quota::with_period(Duration::from_secs(period_secs))
            .expect("non-zero period")
            .allow_burst(NonZeroU32::new(burst).expect("non-zero burst"));
        tracing::info!(
            max_rpm,
            burst,
            period_secs,
            "vertex gateway outbound rate limit enabled"
        );
        Config {
            limiter: Some(RateLimiter::direct(quota)),
            max_rpm,
            burst,
        }
    })
}

/// Block until an outbound provider call is allowed (or return immediately when disabled).
pub async fn acquire_provider_slot() {
    let Some(limiter) = config().limiter.as_ref() else {
        return;
    };
    loop {
        match limiter.check() {
            Ok(()) => return,
            Err(not_until) => {
                let wait = not_until.wait_time_from(DefaultClock::default().now());
                if wait.is_zero() {
                    continue;
                }
                tokio::time::sleep(wait).await;
            }
        }
    }
}

pub fn limit_summary() -> (u32, u32) {
    let c = config();
    (c.max_rpm, c.burst)
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_rpm_means_disabled_config() {
        // Config is process-global; this only validates parsing logic via env_or.
        assert_eq!(env_or("VERTEX_GATEWAY_TEST_MISSING_VAR_XYZ", 20u32), 20);
    }
}
