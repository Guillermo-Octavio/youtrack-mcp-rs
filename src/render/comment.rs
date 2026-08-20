use crate::payload::Render;
use crate::youtrack::comment::Comment;

impl Render for Comment {
    fn render(&self) -> String {
        let who = self
            .author
            .as_ref()
            .and_then(|author| author.full_name.clone().or_else(|| author.login.clone()))
            .unwrap_or_else(|| "?".to_string());
        let when = format_created(self.created);
        let text = self.text.as_deref().unwrap_or("").trim();

        format!("[{when}] {who}:\n  {}", text.replace('\n', "\n  "))
    }
}

/// Formats a comment's `created` timestamp (milliseconds since the Unix
/// epoch) as `YYYY-MM-DD`, or `—` when absent.
fn format_created(created_ms: Option<i64>) -> String {
    match created_ms {
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
mod tests {
    use super::*;
    use crate::youtrack::issue::UserRef;

    #[test]
    fn civil_from_days_converts_the_epoch_to_1970_01_01() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_converts_a_known_recent_date() {
        // 2024-01-01 is 19723 days after the Unix epoch.
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn format_created_renders_a_dash_when_absent() {
        assert_eq!(format_created(None), "—");
    }

    #[test]
    fn format_created_renders_year_month_day() {
        assert_eq!(format_created(Some(0)), "1970-01-01");
    }

    #[test]
    fn render_prefers_full_name_over_login_and_indents_the_text() {
        let comment = Comment {
            id: Some("1".to_string()),
            text: Some("Looks good\nShip it".to_string()),
            created: Some(0),
            author: Some(UserRef {
                login: Some("jdoe".to_string()),
                full_name: Some("Jane Doe".to_string()),
            }),
        };

        assert_eq!(
            comment.render(),
            "[1970-01-01] Jane Doe:\n  Looks good\n  Ship it"
        );
    }

    #[test]
    fn render_falls_back_to_login_and_then_a_placeholder() {
        let with_login = Comment {
            author: Some(UserRef {
                login: Some("jdoe".to_string()),
                full_name: None,
            }),
            ..Default::default()
        };
        let with_no_author = Comment::default();

        assert!(with_login.render().contains("jdoe"));
        assert!(with_no_author.render().contains('?'));
    }
}
