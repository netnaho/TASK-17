use serde::{Deserialize, Serialize};

use crate::error::ApiAppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReqStatus {
    Draft,
    PendingApproval,
    SentBack,
    ApprovedFinal,
    Rejected,
    Withdrawn,
    Issued,
}

impl ReqStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::PendingApproval => "pending_approval",
            Self::SentBack => "sent_back",
            Self::ApprovedFinal => "approved_final",
            Self::Rejected => "rejected",
            Self::Withdrawn => "withdrawn",
            Self::Issued => "issued",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "draft" => Self::Draft,
            "pending_approval" => Self::PendingApproval,
            "sent_back" => Self::SentBack,
            "approved_final" => Self::ApprovedFinal,
            "rejected" => Self::Rejected,
            "withdrawn" => Self::Withdrawn,
            "issued" => Self::Issued,
            _ => return None,
        })
    }
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Rejected | Self::Withdrawn | Self::Issued)
    }
    pub fn is_editable(&self) -> bool {
        matches!(self, Self::Draft | Self::SentBack)
    }
    pub fn is_withdrawable(&self) -> bool {
        matches!(self, Self::Draft | Self::SentBack | Self::PendingApproval)
    }
}

/// Centralized transition validator. Every status change MUST go through
/// here — services call `Transition::check(from, to)?` before any DB write.
/// This is the only place where the requisition lifecycle is encoded.
pub struct Transition;

impl Transition {
    pub fn check(from: ReqStatus, to: ReqStatus) -> Result<(), ApiAppError> {
        let allowed = matches!(
            (from, to),
            (ReqStatus::Draft,            ReqStatus::PendingApproval) |
            (ReqStatus::Draft,            ReqStatus::Withdrawn)       |
            (ReqStatus::SentBack,         ReqStatus::PendingApproval) |
            (ReqStatus::SentBack,         ReqStatus::Withdrawn)       |
            (ReqStatus::PendingApproval,  ReqStatus::SentBack)        |
            (ReqStatus::PendingApproval,  ReqStatus::Rejected)        |
            (ReqStatus::PendingApproval,  ReqStatus::ApprovedFinal)   |
            (ReqStatus::PendingApproval,  ReqStatus::Withdrawn)       |
            (ReqStatus::ApprovedFinal,    ReqStatus::Issued)
        );
        if allowed {
            Ok(())
        } else {
            Err(ApiAppError::BadRequest(format!(
                "invalid transition: {} → {}",
                from.as_str(),
                to.as_str()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn happy_path_transitions() {
        Transition::check(ReqStatus::Draft, ReqStatus::PendingApproval).unwrap();
        Transition::check(ReqStatus::PendingApproval, ReqStatus::ApprovedFinal).unwrap();
        Transition::check(ReqStatus::ApprovedFinal, ReqStatus::Issued).unwrap();
    }
    #[test]
    fn invalid_transitions_blocked() {
        assert!(Transition::check(ReqStatus::Issued, ReqStatus::PendingApproval).is_err());
        assert!(Transition::check(ReqStatus::Rejected, ReqStatus::ApprovedFinal).is_err());
        assert!(Transition::check(ReqStatus::Draft, ReqStatus::Issued).is_err());
        assert!(Transition::check(ReqStatus::Withdrawn, ReqStatus::PendingApproval).is_err());
    }
    #[test]
    fn withdraw_only_while_pending_or_editable() {
        assert!(ReqStatus::Draft.is_withdrawable());
        assert!(ReqStatus::SentBack.is_withdrawable());
        assert!(ReqStatus::PendingApproval.is_withdrawable());
        assert!(!ReqStatus::ApprovedFinal.is_withdrawable());
        assert!(!ReqStatus::Issued.is_withdrawable());
    }

    #[test]
    fn sent_back_can_be_resubmitted() {
        Transition::check(ReqStatus::PendingApproval, ReqStatus::SentBack).unwrap();
        Transition::check(ReqStatus::SentBack, ReqStatus::PendingApproval).unwrap();
    }

    #[test]
    fn terminal_states_are_not_withdrawable_or_editable() {
        for s in [ReqStatus::Issued, ReqStatus::Rejected, ReqStatus::Withdrawn] {
            assert!(!s.is_editable(), "{:?} should not be editable", s);
            assert!(!s.is_withdrawable(), "{:?} should not be withdrawable", s);
            assert!(s.is_terminal(), "{:?} should be terminal", s);
        }
    }

    #[test]
    fn draft_and_sent_back_are_editable() {
        assert!(ReqStatus::Draft.is_editable());
        assert!(ReqStatus::SentBack.is_editable());
        assert!(!ReqStatus::PendingApproval.is_editable());
        assert!(!ReqStatus::ApprovedFinal.is_editable());
    }

    #[test]
    fn all_statuses_round_trip_through_parse() {
        let statuses = [
            "draft", "pending_approval", "sent_back", "approved_final",
            "rejected", "withdrawn", "issued",
        ];
        for s in statuses {
            let parsed = ReqStatus::parse(s).unwrap_or_else(|| panic!("failed to parse '{s}'"));
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn unknown_status_parses_to_none() {
        assert!(ReqStatus::parse("not_a_status").is_none());
        assert!(ReqStatus::parse("").is_none());
        assert!(ReqStatus::parse("DRAFT").is_none()); // case-sensitive
    }

    #[test]
    fn approved_final_cannot_be_withdrawn() {
        assert!(Transition::check(ReqStatus::ApprovedFinal, ReqStatus::Withdrawn).is_err());
    }
}
