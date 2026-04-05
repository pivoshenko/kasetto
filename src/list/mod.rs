//! Interactive skill browser (`kasetto list` in a TTY).

mod browse;
mod render;
mod session;
mod tab;
mod types;

pub(crate) use browse::browse;
pub(crate) use types::{mcp_asset_entries, BrowseInput};
