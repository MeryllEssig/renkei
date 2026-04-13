//! Preinstall and postinstall message rendering for the install pipeline.
//!
//! Authors declare optional `messages.preinstall` / `messages.postinstall` strings in
//! `renkei.json`. Preinstall messages are gathered across the whole install batch,
//! rendered in a single yellow/bold framed block, and gated by a `[y/N]` prompt.
//! Postinstall messages render passively after each successful install.

use std::io::IsTerminal;

use owo_colors::OwoColorize;

use crate::error::{RenkeiError, Result};
use crate::manifest::Manifest;

/// One entry in the consolidated preinstall block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreinstallNotice {
    pub full_name: String,
    pub message: String,
}

/// Walk the batch and pull out every package that declares a non-empty
/// `messages.preinstall`. Order is preserved so the rendered block matches the
/// install order.
pub fn collect_preinstall(manifests: &[&Manifest]) -> Vec<PreinstallNotice> {
    manifests
        .iter()
        .filter_map(|m| {
            let text = m.messages.as_ref()?.preinstall.as_ref()?;
            if text.is_empty() {
                None
            } else {
                Some(PreinstallNotice {
                    full_name: m.name.clone(),
                    message: text.clone(),
                })
            }
        })
        .collect()
}

/// Render the preinstall block to a string. Public for snapshot-style testing.
pub fn render_preinstall_block(notices: &[PreinstallNotice]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", "Preinstall notice:".yellow().bold()));
    for n in notices {
        let mut lines = n.message.lines();
        if let Some(first) = lines.next() {
            out.push_str(&format!("  {}: {}\n", n.full_name.bold(), first));
        }
        for line in lines {
            out.push_str(&format!("  {}\n", line));
        }
    }
    out
}

/// Resolve the user's intent for a batch with `notices`.
///
/// Returns:
/// - `Ok(true)`  → no notices, OR `yes == true`, OR user accepted at the prompt.
/// - `Ok(false)` → user declined at the prompt (caller should exit 0).
/// - `Err(PreinstallRequiresConfirmation)` → there are notices but no TTY and `yes == false`.
pub fn confirm_preinstall(notices: &[PreinstallNotice], yes: bool) -> Result<bool> {
    if notices.is_empty() {
        return Ok(true);
    }
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        return Err(RenkeiError::PreinstallRequiresConfirmation);
    }

    print!("{}", render_preinstall_block(notices));

    let answer = inquire::Confirm::new("Install all?")
        .with_default(false)
        .prompt()
        .map_err(|e| {
            RenkeiError::DeploymentFailed(format!("Preinstall confirmation failed: {e}"))
        })?;

    if !answer {
        println!("{}", "Installation cancelled.".yellow());
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Messages;

    fn manifest_with(name: &str, preinstall: Option<&str>) -> Manifest {
        Manifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "x".to_string(),
            author: "a".to_string(),
            license: "MIT".to_string(),
            backends: vec!["claude".to_string()],
            keywords: vec![],
            scope: Default::default(),
            mcp: None,
            required_env: None,
            messages: preinstall.map(|p| Messages {
                preinstall: Some(p.to_string()),
                postinstall: None,
            }),
        }
    }

    #[test]
    fn collect_skips_packages_without_messages() {
        let a = manifest_with("@x/a", None);
        let b = manifest_with("@x/b", Some("hello"));
        let c = manifest_with("@x/c", None);
        let got = collect_preinstall(&[&a, &b, &c]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].full_name, "@x/b");
        assert_eq!(got[0].message, "hello");
    }

    #[test]
    fn collect_skips_empty_string_messages() {
        let a = manifest_with("@x/a", Some(""));
        assert!(collect_preinstall(&[&a]).is_empty());
    }

    #[test]
    fn collect_preserves_order() {
        let a = manifest_with("@x/a", Some("first"));
        let b = manifest_with("@x/b", Some("second"));
        let got = collect_preinstall(&[&a, &b]);
        assert_eq!(got[0].full_name, "@x/a");
        assert_eq!(got[1].full_name, "@x/b");
    }

    #[test]
    fn confirm_with_no_notices_returns_true_without_prompt() {
        assert!(confirm_preinstall(&[], false).unwrap());
    }

    #[test]
    fn confirm_with_yes_flag_returns_true_without_prompt() {
        let notices = vec![PreinstallNotice {
            full_name: "@x/a".into(),
            message: "do the thing".into(),
        }];
        assert!(confirm_preinstall(&notices, true).unwrap());
    }

    #[test]
    fn confirm_in_non_tty_without_yes_errors() {
        // Cargo test stdin is not a TTY.
        let notices = vec![PreinstallNotice {
            full_name: "@x/a".into(),
            message: "do the thing".into(),
        }];
        let err = confirm_preinstall(&notices, false).unwrap_err();
        assert!(matches!(err, RenkeiError::PreinstallRequiresConfirmation));
    }

    #[test]
    fn render_block_contains_title_and_each_notice() {
        let notices = vec![
            PreinstallNotice {
                full_name: "@x/a".into(),
                message: "first".into(),
            },
            PreinstallNotice {
                full_name: "@x/b".into(),
                message: "second".into(),
            },
        ];
        let out = render_preinstall_block(&notices);
        assert!(out.contains("Preinstall notice:"));
        assert!(out.contains("@x/a"));
        assert!(out.contains("first"));
        assert!(out.contains("@x/b"));
        assert!(out.contains("second"));
    }

    #[test]
    fn render_block_indents_multiline_messages() {
        let notices = vec![PreinstallNotice {
            full_name: "@x/a".into(),
            message: "line 1\nline 2".into(),
        }];
        let out = render_preinstall_block(&notices);
        assert!(out.contains("@x/a"));
        assert!(out.contains("line 1"));
        // Continuation line indented by two spaces (no name prefix).
        assert!(out.contains("\n  line 2\n"));
    }

    #[test]
    fn preinstall_requires_confirmation_message() {
        let msg = RenkeiError::PreinstallRequiresConfirmation.to_string();
        assert!(msg.contains("--yes"));
        assert!(msg.contains("non-interactive"));
    }
}
