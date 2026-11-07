//! Tatami ownership ("klaim tatami").
//!
//! Only one control panel may drive a given tatami at a time. Each control panel
//! window generates a random `owner_id`, claims its tatami on open, and repeats the
//! claim as a heartbeat every few seconds. A claim that has not been refreshed within
//! `CLAIM_TTL` is considered abandoned (window closed, laptop disconnected) and can be
//! taken by anyone. A live claim can only be taken over with `force = true`; the
//! previous owner learns about it on its next heartbeat and stops broadcasting.
//!
//! Claims live in memory on the host process only. Host windows reach this module via
//! Tauri IPC; operator-tambahan laptops and browser devices reach the same state via
//! the LAN RPC dispatcher, so everyone sees one consistent table.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const CLAIM_TTL: Duration = Duration::from_secs(10);

struct Claim {
    owner_id: String,
    owner_label: String,
    claimed_at_ms: u64,
    last_seen: Instant,
}

static CLAIMS: Mutex<Option<HashMap<i64, Claim>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct ClaimResult {
    pub granted: bool,
    pub tatami: i64,
    pub owner_id: Option<String>,
    pub owner_label: Option<String>,
    pub claimed_at_ms: Option<u64>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Claims (or refreshes the claim on) `tatami` for `owner_id`.
/// Returns `granted = false` with the current owner when another live owner holds it
/// and `force` is false.
pub fn claim(tatami: i64, owner_id: &str, owner_label: &str, force: bool) -> ClaimResult {
    claim_at(tatami, owner_id, owner_label, force, Instant::now())
}

fn claim_at(tatami: i64, owner_id: &str, owner_label: &str, force: bool, now: Instant) -> ClaimResult {
    let mut guard = CLAIMS.lock().unwrap_or_else(|p| p.into_inner());
    let claims = guard.get_or_insert_with(HashMap::new);

    if let Some(existing) = claims.get_mut(&tatami) {
        let expired = now.saturating_duration_since(existing.last_seen) > CLAIM_TTL;
        if existing.owner_id == owner_id {
            existing.last_seen = now;
            if !owner_label.is_empty() {
                existing.owner_label = owner_label.to_string();
            }
            return ClaimResult {
                granted: true,
                tatami,
                owner_id: Some(existing.owner_id.clone()),
                owner_label: Some(existing.owner_label.clone()),
                claimed_at_ms: Some(existing.claimed_at_ms),
            };
        }
        if !expired && !force {
            return ClaimResult {
                granted: false,
                tatami,
                owner_id: Some(existing.owner_id.clone()),
                owner_label: Some(existing.owner_label.clone()),
                claimed_at_ms: Some(existing.claimed_at_ms),
            };
        }
    }

    let claimed_at_ms = now_ms();
    claims.insert(
        tatami,
        Claim {
            owner_id: owner_id.to_string(),
            owner_label: owner_label.to_string(),
            claimed_at_ms,
            last_seen: now,
        },
    );
    ClaimResult {
        granted: true,
        tatami,
        owner_id: Some(owner_id.to_string()),
        owner_label: Some(owner_label.to_string()),
        claimed_at_ms: Some(claimed_at_ms),
    }
}

/// Releases `tatami` if (and only if) `owner_id` currently holds it.
pub fn release(tatami: i64, owner_id: &str) -> bool {
    let mut guard = CLAIMS.lock().unwrap_or_else(|p| p.into_inner());
    let Some(claims) = guard.as_mut() else {
        return false;
    };
    if claims.get(&tatami).map(|c| c.owner_id == owner_id).unwrap_or(false) {
        claims.remove(&tatami);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each test uses its own tatami numbers because CLAIMS is process-global.

    #[test]
    fn first_claim_is_granted_and_heartbeat_refreshes() {
        let t0 = Instant::now();
        assert!(claim_at(9101, "a", "Laptop host", false, t0).granted);
        assert!(claim_at(9101, "a", "Laptop host", false, t0 + Duration::from_secs(8)).granted);
        // Still owned by "a" 8s after the refresh, so "b" is refused.
        let r = claim_at(9101, "b", "Operator tambahan", false, t0 + Duration::from_secs(16));
        assert!(!r.granted);
        assert_eq!(r.owner_label.as_deref(), Some("Laptop host"));
    }

    #[test]
    fn live_claim_needs_force_and_expired_claim_does_not() {
        let t0 = Instant::now();
        assert!(claim_at(9102, "a", "A", false, t0).granted);
        assert!(!claim_at(9102, "b", "B", false, t0 + Duration::from_secs(2)).granted);
        assert!(claim_at(9102, "b", "B", true, t0 + Duration::from_secs(2)).granted);
        // "a" lost it: its heartbeat is refused now.
        assert!(!claim_at(9102, "a", "A", false, t0 + Duration::from_secs(3)).granted);
        // After "b" goes silent past the TTL, "a" can take it back without force.
        assert!(claim_at(9102, "a", "A", false, t0 + Duration::from_secs(3) + CLAIM_TTL + Duration::from_secs(1)).granted);
    }

    #[test]
    fn release_only_by_owner() {
        let t0 = Instant::now();
        assert!(claim_at(9103, "a", "A", false, t0).granted);
        assert!(!release(9103, "b"));
        assert!(!claim_at(9103, "b", "B", false, t0).granted);
        assert!(release(9103, "a"));
        assert!(claim_at(9103, "b", "B", false, t0).granted);
    }
}
