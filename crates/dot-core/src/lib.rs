//! dot core: config, facts, modules, render, plan, diff, state.

pub mod backup;
pub mod config;
pub mod diff;
pub mod error;
pub mod facts;
pub mod module;
pub mod plan;
pub mod render;
pub mod state;

pub use backup::{prune_backups, restore, snapshot, BackupId};
pub use config::{Config, UserConfig};
pub use diff::unified_hunks;
pub use error::{DotError, Result};
pub use facts::{detect, HostFacts};
pub use module::{Module, MODULES};
pub use plan::{plan_write, WriteAction, WritePlan};
pub use render::{render_module, RenderedFile};
pub use state::{ManagedFile, Manifest};
