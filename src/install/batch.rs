//! Batch coordinator helpers for install paths that handle more than one
//! package per invocation (workspace, lockfile replay) and the single-package
//! path that still needs a unified preinstall confirmation.
//!
//! The coordinator pattern is:
//! 1. Collect every relevant `Manifest` upfront (single, all workspace
//!    members, or all lockfile entries — git sources cloned, archives
//!    extracted before this step).
//! 2. Run [`confirm_batch`] once. If it returns `Ok(false)`, the caller
//!    should exit 0 with no side effects.
//! 3. Run the actual installs, gathering each member's optional
//!    `messages.postinstall` string.
//! 4. Render every gathered postinstall via [`print_postinstall_summary`]
//!    at the end of the batch (labelled with the package or member name).

use crate::error::Result;
use crate::manifest::Manifest;

use super::messages::{collect_preinstall, confirm_preinstall};
use super::print_postinstall_block;

/// Render the consolidated preinstall block (if any) and prompt the user.
///
/// - `Ok(true)` → proceed with installs.
/// - `Ok(false)` → user declined; caller should exit 0 without side effects.
/// - `Err(PreinstallRequiresConfirmation)` → non-TTY environment without `--yes`.
pub(crate) fn confirm_batch(manifests: &[&Manifest], yes: bool) -> Result<bool> {
    let notices = collect_preinstall(manifests);
    confirm_preinstall(&notices, yes)
}

/// Print one labelled postinstall block per `(label, message)` pair, in order.
/// Skips empty messages defensively (callers should already have filtered).
pub(crate) fn print_postinstall_summary(items: &[(String, String)]) {
    for (label, msg) in items {
        if !msg.is_empty() {
            print_postinstall_block(msg, Some(label));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_summary_with_no_items_is_noop() {
        // Compiles and doesn't panic with empty input.
        print_postinstall_summary(&[]);
    }

    #[test]
    fn confirm_batch_with_no_messages_returns_true() {
        let m = Manifest {
            name: "@x/y".into(),
            version: "1.0.0".into(),
            description: "x".into(),
            author: "a".into(),
            license: "MIT".into(),
            backends: vec!["claude".into()],
            keywords: vec![],
            scope: Default::default(),
            mcp: None,
            required_env: None,
            messages: None,
        };
        assert!(confirm_batch(&[&m], false).unwrap());
    }
}
