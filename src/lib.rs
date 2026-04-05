/// Default config file in the current directory when `--config` is omitted.
pub(crate) const DEFAULT_CONFIG_FILENAME: &str = "kasetto.yaml";

mod app;
mod banner;
mod cli;
mod colors;
mod commands;
mod error;
mod fsops;
mod home;
mod list;
mod lock;
mod mcps;
mod model;
mod profile;
mod source;
mod tui;
mod ui;

pub use app::run;
pub use error::Result;
