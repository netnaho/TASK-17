/// Shared display helpers for formatting values from API responses.

// ── Date / datetime formatting ─────────────────────────────────────────────

/// Safely extracts the date portion from an ISO 8601 datetime string.
///
/// Returns the first 10 bytes ("YYYY-MM-DD") when the string is long enough.
/// Falls back to returning the full string for short-but-non-empty values, or
/// "—" for an empty string.  Never panics on short or otherwise unexpected
/// input.
///
/// # Examples
/// ```
/// # use frontend_yew::components::utils::fmt_date;
/// assert_eq!(fmt_date("2024-03-15T10:30:00Z"), "2024-03-15");
/// assert_eq!(fmt_date("2024-03-15"),           "2024-03-15");
/// assert_eq!(fmt_date("2024-03"),              "2024-03");   // too short → full value
/// assert_eq!(fmt_date(""),                     "—");
/// ```
pub fn fmt_date(s: &str) -> &str {
    if s.len() >= 10 {
        // All ISO 8601 date characters are single-byte ASCII, so the 10-byte
        // boundary is always a valid char boundary — no need for is_char_boundary.
        &s[..10]
    } else if s.is_empty() {
        "—"
    } else {
        s
    }
}

/// Safely extracts the date-and-time portion from an ISO 8601 datetime string.
///
/// Returns the first 16 bytes ("YYYY-MM-DDTHH:MM") when the string is long
/// enough.  Falls back to returning the full string for short-but-non-empty
/// values, or "—" for an empty string.  Never panics on short or otherwise
/// unexpected input.
///
/// # Examples
/// ```
/// # use frontend_yew::components::utils::fmt_datetime;
/// assert_eq!(fmt_datetime("2024-03-15T10:30:00Z"), "2024-03-15T10:30");
/// assert_eq!(fmt_datetime("2024-03-15T10:30"),     "2024-03-15T10:30");
/// assert_eq!(fmt_datetime("2024-03-15"),           "2024-03-15");  // too short → full
/// assert_eq!(fmt_datetime(""),                     "—");
/// ```
pub fn fmt_datetime(s: &str) -> &str {
    if s.len() >= 16 {
        &s[..16]
    } else if s.is_empty() {
        "—"
    } else {
        s
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── fmt_date ──────────────────────────────────────────────────────────

    #[test]
    fn fmt_date_full_iso_datetime_truncates_to_date() {
        assert_eq!(fmt_date("2024-03-15T10:30:00Z"), "2024-03-15");
    }

    #[test]
    fn fmt_date_full_iso_datetime_with_microseconds() {
        assert_eq!(fmt_date("2024-03-15T10:30:00.000000Z"), "2024-03-15");
    }

    #[test]
    fn fmt_date_exactly_10_chars_returned_unchanged() {
        assert_eq!(fmt_date("2024-03-15"), "2024-03-15");
    }

    #[test]
    fn fmt_date_longer_than_10_but_no_time_part() {
        // e.g. an ISO string with no time — just 11 chars including trailing Z
        assert_eq!(fmt_date("2024-03-15Z"), "2024-03-15");
    }

    #[test]
    fn fmt_date_short_string_returns_full_value_not_empty_marker() {
        // Partial dates should show as-is — better than "—"
        assert_eq!(fmt_date("2024-03"), "2024-03");
        assert_eq!(fmt_date("2024"), "2024");
    }

    #[test]
    fn fmt_date_empty_string_returns_dash() {
        assert_eq!(fmt_date(""), "—");
    }

    #[test]
    fn fmt_date_single_char_returns_full_value() {
        assert_eq!(fmt_date("x"), "x");
    }

    // ── fmt_datetime ──────────────────────────────────────────────────────

    #[test]
    fn fmt_datetime_full_iso_datetime_truncates_to_minutes() {
        assert_eq!(fmt_datetime("2024-03-15T10:30:00Z"), "2024-03-15T10:30");
    }

    #[test]
    fn fmt_datetime_full_iso_datetime_with_microseconds() {
        assert_eq!(fmt_datetime("2024-03-15T10:30:00.123456Z"), "2024-03-15T10:30");
    }

    #[test]
    fn fmt_datetime_exactly_16_chars_returned_unchanged() {
        assert_eq!(fmt_datetime("2024-03-15T10:30"), "2024-03-15T10:30");
    }

    #[test]
    fn fmt_datetime_10_char_date_only_returns_full_value() {
        // Only a date — not enough for the datetime slice, return as-is.
        assert_eq!(fmt_datetime("2024-03-15"), "2024-03-15");
    }

    #[test]
    fn fmt_datetime_short_string_returns_full_value() {
        assert_eq!(fmt_datetime("2024-03"), "2024-03");
    }

    #[test]
    fn fmt_datetime_empty_string_returns_dash() {
        assert_eq!(fmt_datetime(""), "—");
    }

    // ── shared invariants ─────────────────────────────────────────────────

    #[test]
    fn neither_helper_panics_on_single_byte_string() {
        // Regression guard: ensure [..10] / [..16] guards are correct
        let tiny = "a";
        let _ = fmt_date(tiny);
        let _ = fmt_datetime(tiny);
    }

    #[test]
    fn fmt_date_result_is_always_at_most_10_chars() {
        let samples = [
            "2024-03-15T10:30:00Z",
            "2024-03-15",
            "2024",
            "",
            "abc",
        ];
        for s in samples {
            assert!(
                fmt_date(s).chars().count() <= 10 || fmt_date(s) == s,
                "fmt_date(\"{s}\") returned more than 10 chars without falling back"
            );
        }
    }

    #[test]
    fn fmt_datetime_result_is_always_at_most_16_chars() {
        let samples = [
            "2024-03-15T10:30:00Z",
            "2024-03-15T10:30",
            "2024-03-15",
            "",
            "abc",
        ];
        for s in samples {
            assert!(
                fmt_datetime(s).chars().count() <= 16 || fmt_datetime(s) == s,
                "fmt_datetime(\"{s}\") returned more than 16 chars without falling back"
            );
        }
    }
}
