//! Pagination helpers.
//!
//! [`Page`] is the shape returned by every list-style tool. Callers
//! fetch `limit + 1` items from the YouTrack API so this module can
//! tell, without a second request, whether more results exist:
//! [`paginate`] truncates the fetched batch back down to `limit` and
//! produces an opaque [`Cursor`] for the next page when needed.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use thiserror::Error;

/// A page of results returned by a list-style tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// An opaque pagination cursor encoding a `skip` offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub skip: usize,
}

/// A pagination cursor could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid pagination cursor")]
pub struct InvalidCursor;

impl Cursor {
    /// Encodes this cursor as an opaque, URL-safe string.
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!("skip:{}", self.skip))
    }

    /// Decodes a cursor previously produced by [`Cursor::encode`].
    pub fn decode(value: &str) -> Result<Self, InvalidCursor> {
        let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| InvalidCursor)?;
        let text = String::from_utf8(bytes).map_err(|_| InvalidCursor)?;
        let skip_str = text.strip_prefix("skip:").ok_or(InvalidCursor)?;
        let skip = skip_str.parse::<usize>().map_err(|_| InvalidCursor)?;
        Ok(Cursor { skip })
    }
}

/// Builds a [`Page`] from a fetched `batch` of up to `limit + 1` items.
///
/// `skip` is the offset that produced this batch, used to compute the
/// next page's cursor. When `batch` holds more than `limit` items, the
/// extra item is dropped and `has_more` is set; otherwise the whole
/// batch is returned with no next cursor.
pub fn paginate<T>(mut batch: Vec<T>, limit: usize, skip: usize) -> Page<T> {
    let has_more = batch.len() > limit;
    if has_more {
        batch.truncate(limit);
    }
    let next_cursor = has_more.then(|| Cursor { skip: skip + limit }.encode());

    Page {
        items: batch,
        has_more,
        next_cursor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_smaller_than_limit_has_no_more_pages() {
        let batch = vec!["a", "b"];

        let page = paginate(batch, 5, 0);

        assert_eq!(page.items, vec!["a", "b"]);
        assert!(!page.has_more);
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn batch_of_limit_plus_one_has_more_pages_with_valid_cursor() {
        let batch = vec![1, 2, 3, 4, 5, 6];

        let page = paginate(batch, 5, 0);

        assert_eq!(page.items, vec![1, 2, 3, 4, 5]);
        assert!(page.has_more);
        let cursor = page.next_cursor.expect("cursor must be present");
        assert_eq!(Cursor::decode(&cursor).unwrap().skip, 5);
    }

    #[test]
    fn batch_exactly_at_limit_has_no_more_pages() {
        let batch = vec![1, 2, 3];

        let page = paginate(batch, 3, 0);

        assert_eq!(page.items, vec![1, 2, 3]);
        assert!(!page.has_more);
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn cursor_next_offset_accounts_for_existing_skip() {
        let batch = vec![1, 2, 3, 4];

        let page = paginate(batch, 3, 10);

        let cursor = page.next_cursor.expect("cursor must be present");
        assert_eq!(Cursor::decode(&cursor).unwrap().skip, 13);
    }

    #[test]
    fn cursor_round_trips_through_encode_and_decode() {
        let cursor = Cursor { skip: 42 };

        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();

        assert_eq!(decoded.skip, 42);
    }

    #[test]
    fn cursor_decode_rejects_garbage_input() {
        let result = Cursor::decode("not-a-valid-cursor!!!");

        assert!(result.is_err());
    }
}
