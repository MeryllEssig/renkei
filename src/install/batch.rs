//! Batch coordinator helpers for install paths that handle more than one
//! package per invocation (workspace, lockfile replay) and the single-package
//! path that still needs a unified preinstall confirmation.
//!
//! The coordinator pattern is:
//! 1. Collect every relevant `Manifest` upfront (single, all workspace
//!    members, or all lockfile entries — git sources cloned, archives
//!    extracted before this step).
//! 2. Run [`confirm_batch`] once. If it returns [`BatchDecision::Declined`],
//!    the caller should exit 0 with no side effects. The [`BatchDecision::Proceed`]
//!    variant carries the effective `allow_build` consent (prompt acceptance
//!    upgrades the CLI flag).
//! 3. Run the actual installs, gathering each member's optional
//!    `messages.postinstall` string.
//! 4. Render every gathered postinstall via [`print_postinstall_summary`]
//!    at the end of the batch (labelled with the package or member name).

use crate::error::Result;
use crate::manifest::Manifest;

use super::build::{collect_build_notices, confirm_builds};
use super::messages::{collect_preinstall, confirm_preinstall};
use super::print_postinstall_block;

/// Outcome of the consolidated preinstall+build confirmation pass.
///
/// `Proceed { allow_build }` carries the *effective* build consent: when the
/// user accepts the interactive build prompt, `allow_build` is `true` even if
/// the CLI flag was not set, so downstream install stages can run builds
/// without tripping the defensive guard in `build_into_staging`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchDecision {
    Proceed { allow_build: bool },
    Declined,
}

impl BatchDecision {
    /// `Some(allow_build)` when the batch should proceed, `None` when the
    /// user declined (caller should exit 0 with no side effects).
    pub(crate) fn proceed(self) -> Option<bool> {
        match self {
            BatchDecision::Proceed { allow_build } => Some(allow_build),
            BatchDecision::Declined => None,
        }
    }
}

/// Render the consolidated preinstall block (if any) and prompt the user.
///
/// - `Ok(Proceed { allow_build })` → proceed with installs; `allow_build`
///   reflects the effective consent after the build prompt.
/// - `Ok(Declined)` → user declined; caller should exit 0 without side effects.
/// - `Err(PreinstallRequiresConfirmation | BuildRequiresConfirmation)` →
///   non-TTY environment without the matching `--yes` / `--allow-build` flag.
pub(crate) fn confirm_batch(
    manifests: &[&Manifest],
    yes: bool,
    allow_build: bool,
    link_mode: bool,
) -> Result<BatchDecision> {
    let preinstall = collect_preinstall(manifests);
    if !confirm_preinstall(&preinstall, yes)? {
        return Ok(BatchDecision::Declined);
    }
    if link_mode {
        // Linked installs never run builds — sources are live, the user
        // owns the build lifecycle in their workspace.
        return Ok(BatchDecision::Proceed { allow_build });
    }
    let builds = collect_build_notices(manifests);
    let had_notices = !builds.is_empty();
    if !confirm_builds(&builds, allow_build)? {
        return Ok(BatchDecision::Declined);
    }
    // Accepting the build prompt is equivalent to passing `--allow-build`
    // for the remainder of this batch.
    let effective_allow_build = allow_build || had_notices;
    Ok(BatchDecision::Proceed {
        allow_build: effective_allow_build,
    })
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

    fn bare_manifest(name: &str) -> Manifest {
        Manifest {
            name: name.into(),
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
        }
    }

    #[test]
    fn confirm_batch_with_no_messages_proceeds_without_build_consent() {
        let m = bare_manifest("@x/y");
        assert_eq!(
            confirm_batch(&[&m], false, false, false).unwrap(),
            BatchDecision::Proceed { allow_build: false }
        );
    }

    #[test]
    fn confirm_batch_link_mode_passes_through_allow_build() {
        let m = bare_manifest("@x/y");
        assert_eq!(
            confirm_batch(&[&m], true, true, true).unwrap(),
            BatchDecision::Proceed { allow_build: true }
        );
        assert_eq!(
            confirm_batch(&[&m], true, false, true).unwrap(),
            BatchDecision::Proceed { allow_build: false }
        );
    }

    #[test]
    fn confirm_batch_allow_build_flag_is_preserved_when_no_builds() {
        let m = bare_manifest("@x/y");
        assert_eq!(
            confirm_batch(&[&m], true, true, false).unwrap(),
            BatchDecision::Proceed { allow_build: true }
        );
    }

    #[test]
    fn confirm_batch_with_build_notices_and_allow_build_flag_proceeds_with_consent() {
        use crate::manifest::McpServer;
        use std::collections::HashMap;

        let mut mcps = HashMap::new();
        mcps.insert(
            "srv".to_string(),
            McpServer {
                entrypoint: Some("dist/index.js".into()),
                build: Some(vec![vec!["bun".into(), "install".into()]]),
                extra: serde_json::Map::new(),
            },
        );
        let mut m = bare_manifest("@x/build-me");
        m.mcp = Some(mcps);

        // allow_build flag already set → no prompt, effective stays true.
        assert_eq!(
            confirm_batch(&[&m], true, true, false).unwrap(),
            BatchDecision::Proceed { allow_build: true }
        );
    }
}
