//! Signing-mode enum controlling which HMAC canonical-string contract the
//! `SignedAuth` middleware enforces.
//!
//! ## Modes
//!
//! | Mode     | Canonical string | Accepts legacy? | Notes |
//! |----------|------------------|-----------------|-------|
//! | `compat` | 4-field          | yes             | Default. Backward-compatible with all existing clients. |
//! | `dual`   | 6-field primary, 4-field fallback | yes (with warning) | Recommended for rolling out strict mode. Clients that already send `x-silveroak-body-hash` are validated strictly; older clients fall back silently with a WARN log. |
//! | `strict` | 6-field only     | no              | Requires all clients to send `x-silveroak-body-hash`. Rejects requests that omit the header with 401. Enable only after dual-mode burn-in. |
//!
//! ## Rollout playbook
//!
//! ```text
//! Phase 1: compat (current default)
//!   → All clients work. No body hash required.
//!
//! Phase 2: dual
//!   → Update clients to send x-silveroak-body-hash.
//!   → Monitor WARN logs for "dual-mode compat fallback" entries.
//!   → Once WARN rate reaches zero, all clients are strict-capable.
//!
//! Phase 3: strict
//!   → Enable APP__SIGNING_MODE=strict.
//!   → Any client that forgot the header gets 401 "missing body hash header".
//!   → Rollback: set APP__SIGNING_MODE=dual (no deployment required).
//! ```

use serde::Deserialize;

/// Active signing mode.  Parsed from `AppConfig::signing_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SigningMode {
    /// Legacy 4-field canonical. Compatible with all existing clients.
    Compat,
    /// Try 6-field strict first; fall back to 4-field compat with a WARN log.
    /// Recommended transition mode.
    Dual,
    /// 6-field strict only. Requires `x-silveroak-body-hash` header.
    Strict,
}

impl Default for SigningMode {
    fn default() -> Self {
        Self::Compat
    }
}

impl SigningMode {
    /// Parse from a config string (case-insensitive).
    /// Unknown values fall back to `Compat` and log a warning.
    pub fn from_config(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "strict" => Self::Strict,
            "dual" => Self::Dual,
            "compat" => Self::Compat,
            other => {
                tracing::warn!(
                    value = other,
                    "unknown signing_mode value; falling back to compat"
                );
                Self::Compat
            }
        }
    }

    /// Human-readable label for structured logs.
    pub fn label(self) -> &'static str {
        match self {
            Self::Compat => "compat",
            Self::Dual => "dual",
            Self::Strict => "strict",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_config_parses_all_variants() {
        assert_eq!(SigningMode::from_config("compat"), SigningMode::Compat);
        assert_eq!(SigningMode::from_config("dual"), SigningMode::Dual);
        assert_eq!(SigningMode::from_config("strict"), SigningMode::Strict);
    }

    #[test]
    fn from_config_is_case_insensitive() {
        assert_eq!(SigningMode::from_config("STRICT"), SigningMode::Strict);
        assert_eq!(SigningMode::from_config("Dual"), SigningMode::Dual);
        assert_eq!(SigningMode::from_config("COMPAT"), SigningMode::Compat);
    }

    #[test]
    fn from_config_unknown_falls_back_to_compat() {
        assert_eq!(SigningMode::from_config("unknown"), SigningMode::Compat);
        assert_eq!(SigningMode::from_config(""), SigningMode::Compat);
    }

    #[test]
    fn default_is_compat() {
        assert_eq!(SigningMode::default(), SigningMode::Compat);
    }

    #[test]
    fn labels_are_distinct() {
        let labels: Vec<_> = [SigningMode::Compat, SigningMode::Dual, SigningMode::Strict]
            .iter()
            .map(|m| m.label())
            .collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(labels.len(), unique.len(), "each mode must have a unique label");
    }
}
