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
mod issue;
mod project;

pub use issue::IssueLinks;

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
