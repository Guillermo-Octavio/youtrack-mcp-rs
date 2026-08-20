//! Human-readable text renderers for YouTrack domain types.
//!
//! Every renderer here implements the [`Render`] trait (from
//! [`crate::payload`]) so [`ToolPayload::from_value`](crate::payload::ToolPayload::from_value)
//! can pair a domain value's text rendering with its structured JSON form
//! for a tool's dual text/JSON output.
//!
//! [`Page<T>`](crate::client::page::Page) gets one generic [`Render`]
//! implementation here, covering every paginated list tool: it renders
//! each item and appends a pagination footer when more results exist.

mod agile;
mod article;
mod attachment;
mod command;
mod comment;
mod issue;
mod project;
mod user;
mod work_item;

pub use article::DeleteArticleResult;
pub use attachment::DownloadAttachmentResult;
pub use command::CommandResult;
pub use issue::{DeleteIssueResult, IssueLinks};
pub use user::Watchers;
pub use work_item::DeleteWorkItemResult;

use crate::client::page::Page;
use crate::payload::Render;

impl<T: Render> Render for Page<T> {
    fn render(&self) -> String {
        if self.items.is_empty() {
            return "No results.".to_string();
        }

        let mut rendered = self
            .items
            .iter()
            .map(Render::render)
            .collect::<Vec<_>>()
            .join("\n\n");

        if self.has_more {
            rendered.push_str("\n\n(more results available; pass the returned cursor to continue)");
        }

        rendered
    }
}

/// Formats an epoch-milliseconds timestamp as `YYYY-MM-DD`, or `—` when
/// absent. Shared by every renderer that displays a raw millisecond
/// timestamp (work items, activity log entries, and similar).
pub(crate) fn format_epoch_millis(ms: Option<i64>) -> String {
    match ms {
        None => "—".to_string(),
        Some(ms) => {
            let days = ms.div_euclid(86_400_000);
            let (year, month, day) = civil_from_days(days);
            format!("{year:04}-{month:02}-{day:02}")
        }
    }
}

/// Converts a day count since the Unix epoch (1970-01-01) into a
/// `(year, month, day)` civil date. Howard Hinnant's `civil_from_days`
/// algorithm (<http://howardhinnant.github.io/date_algorithms.html>),
/// correct for the proleptic Gregorian calendar over the full `i64`
/// range without any external date/time dependency.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod format_epoch_millis_tests {
    use super::format_epoch_millis;

    #[test]
    fn renders_a_dash_when_absent() {
        assert_eq!(format_epoch_millis(None), "—");
    }

    #[test]
    fn renders_year_month_day_for_the_epoch() {
        assert_eq!(format_epoch_millis(Some(0)), "1970-01-01");
    }

    #[test]
    fn renders_a_known_recent_date() {
        // 2024-01-01 is 19723 days after the Unix epoch.
        assert_eq!(format_epoch_millis(Some(19_723 * 86_400_000)), "2024-01-01");
    }
}
