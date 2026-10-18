//! `dot restore` delegates to backup_cmd::run_restore.

use anyhow::Result;

pub(crate) fn run(backup: Option<String>, force: bool, list: bool) -> Result<()> {
    crate::backup_cmd::run_restore(backup, force, list)
}
