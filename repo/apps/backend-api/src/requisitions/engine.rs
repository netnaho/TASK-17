use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;

/// Snapshot of a requisition the engine evaluates against.
#[derive(Debug, Clone)]
pub struct ReqSnapshot {
    pub total_amount_cents: i64,
    pub contains_controlled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub kind: String,
    pub threshold_cents: Option<i64>,
}

impl Condition {
    pub fn matches(&self, snap: &ReqSnapshot) -> bool {
        match self.kind.as_str() {
            "always" => true,
            "amount_gt_cents" => self
                .threshold_cents
                .map(|t| snap.total_amount_cents > t)
                .unwrap_or(false),
            "contains_controlled" => snap.contains_controlled,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedNode {
    pub node_id: Uuid,
    pub sequence: i32,
    pub label: String,
    pub required_role: String,
}

/// Pure rule evaluator. Walks the workflow's nodes in sequence; a node
/// is included when ANY of its conditions match (OR semantics). The
/// resulting list IS the approval route for this requisition.
pub async fn resolve_route(
    pool: &PgPool,
    workflow_id: Uuid,
    snap: &ReqSnapshot,
) -> Result<Vec<ResolvedNode>, ApiAppError> {
    let nodes: Vec<(Uuid, i32, String, String)> = sqlx::query_as(
        "SELECT id, sequence, label, required_role
         FROM approval_nodes WHERE workflow_id = $1 ORDER BY sequence",
    )
    .bind(workflow_id)
    .fetch_all(pool)
    .await?;

    let mut resolved = Vec::new();
    for (id, seq, label, role) in nodes {
        let conds: Vec<(String, Option<i64>)> = sqlx::query_as(
            "SELECT condition_kind, threshold_cents FROM approval_rule_conditions WHERE node_id = $1",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;

        let included = conds.iter().any(|(k, t)| {
            Condition { kind: k.clone(), threshold_cents: *t }.matches(snap)
        });
        if included {
            resolved.push(ResolvedNode {
                node_id: id,
                sequence: seq,
                label,
                required_role: role,
            });
        }
    }

    if resolved.is_empty() {
        return Err(ApiAppError::Internal(
            "approval workflow resolved to zero nodes".into(),
        ));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn condition_matchers() {
        let snap = ReqSnapshot { total_amount_cents: 300_000, contains_controlled: false };
        assert!(Condition { kind: "always".into(), threshold_cents: None }.matches(&snap));
        assert!(Condition { kind: "amount_gt_cents".into(), threshold_cents: Some(250_000) }.matches(&snap));
        assert!(!Condition { kind: "amount_gt_cents".into(), threshold_cents: Some(500_000) }.matches(&snap));
        assert!(!Condition { kind: "contains_controlled".into(), threshold_cents: None }.matches(&snap));
        let snap2 = ReqSnapshot { total_amount_cents: 100, contains_controlled: true };
        assert!(Condition { kind: "contains_controlled".into(), threshold_cents: None }.matches(&snap2));
    }

    #[test]
    fn amount_gt_requires_threshold_set() {
        // amount_gt_cents with no threshold_cents → false (safety: unknown threshold)
        let snap = ReqSnapshot { total_amount_cents: 999_999, contains_controlled: false };
        let cond = Condition { kind: "amount_gt_cents".into(), threshold_cents: None };
        assert!(!cond.matches(&snap), "amount_gt_cents with no threshold should be false");
    }

    #[test]
    fn unknown_condition_kind_does_not_match() {
        let snap = ReqSnapshot { total_amount_cents: 1_000, contains_controlled: true };
        let cond = Condition { kind: "future_condition".into(), threshold_cents: None };
        assert!(!cond.matches(&snap));
    }

    #[test]
    fn amount_exactly_at_threshold_does_not_match() {
        // "amount_gt_cents" is strictly greater-than, not >=
        let snap = ReqSnapshot { total_amount_cents: 250_000, contains_controlled: false };
        let cond = Condition { kind: "amount_gt_cents".into(), threshold_cents: Some(250_000) };
        assert!(!cond.matches(&snap), "equal to threshold should NOT match (strictly greater-than)");
    }

    #[test]
    fn controlled_flag_independent_of_amount() {
        let snap_low_ctl = ReqSnapshot { total_amount_cents: 100, contains_controlled: true };
        let snap_high_no_ctl = ReqSnapshot { total_amount_cents: 999_999, contains_controlled: false };

        let ctl_cond = Condition { kind: "contains_controlled".into(), threshold_cents: None };
        assert!(ctl_cond.matches(&snap_low_ctl));
        assert!(!ctl_cond.matches(&snap_high_no_ctl));
    }
}
