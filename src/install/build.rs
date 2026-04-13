use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

use owo_colors::OwoColorize;

use crate::error::{RenkeiError, Result};
use crate::manifest::Manifest;

use super::messages::confirm_block;

const PLAIN_WHITELIST: &[&str] = &[
    "PATH", "HOME", "USER", "LOGNAME", "LANG", "TMPDIR", "SHELL", "TERM",
];

const PROXY_VARS: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"];

const CERT_VARS: &[&str] = &[
    "NODE_EXTRA_CA_CERTS",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "REQUESTS_CA_BUNDLE",
];

const TOOL_PREFIXES: &[&str] = &[
    "npm_config_",
    "PIP_",
    "BUN_",
    "CARGO_",
    "UV_",
    "POETRY_",
];

const EXCLUDE_PREFIXES: &[&str] = &[
    "AWS_",
    "GITHUB_",
    "GITLAB_",
    "ANTHROPIC_",
    "OPENAI_",
];

/// Decide whether a given env var name should be exposed to package build
/// scripts. Exclusion rules win over inclusion rules: a `_TOKEN` named
/// `npm_config_TOKEN` would still be filtered out.
fn should_keep(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();

    if EXCLUDE_PREFIXES.iter().any(|p| upper.starts_with(p)) {
        return false;
    }
    if upper.contains("PASSWORD")
        || upper.ends_with("_TOKEN")
        || upper.contains("_TOKEN_")
        || upper.ends_with("_KEY")
        || upper.contains("_KEY_")
        || upper.ends_with("_SECRET")
        || upper.contains("_SECRET_")
    {
        return false;
    }

    if PLAIN_WHITELIST.contains(&name) {
        return true;
    }
    if name.starts_with("LC_") {
        return true;
    }
    if PROXY_VARS.iter().any(|p| upper == *p) {
        return true;
    }
    if CERT_VARS.contains(&name) {
        return true;
    }
    if TOOL_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return true;
    }

    false
}

/// Build the minimal env that local-MCP build commands run with. Whitelist
/// + tooling prefixes minus secret-shaped names. Reads the current process
/// env once.
pub fn build_env() -> HashMap<String, String> {
    std::env::vars().filter(|(k, _)| should_keep(k)).collect()
}

pub struct BuildStep {
    pub argv: Vec<String>,
}

/// Run a sequence of build steps under `cwd` with the filtered env. No
/// shell — argv goes straight to the kernel via `execve`. Streams stdout
/// and stderr live so the user sees progress; first non-zero exit aborts
/// and surfaces `BuildFailed`.
pub fn run_build(steps: &[BuildStep], cwd: &Path) -> Result<()> {
    for step in steps {
        if step.argv.is_empty() {
            return Err(RenkeiError::BuildFailed {
                step: String::new(),
                exit_code: None,
            });
        }
        let status = Command::new(&step.argv[0])
            .args(&step.argv[1..])
            .current_dir(cwd)
            .env_clear()
            .envs(build_env())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| RenkeiError::BuildFailed {
                step: step.argv.join(" "),
                exit_code: e.raw_os_error(),
            })?;
        if !status.success() {
            return Err(RenkeiError::BuildFailed {
                step: step.argv.join(" "),
                exit_code: status.code(),
            });
        }
    }
    Ok(())
}

/// One local-MCP build declared by a manifest in the current install batch.
/// Collected upfront so the user sees every build they're about to run in a
/// single consolidated prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildNotice {
    pub full_name: String,
    pub mcp_name: String,
    pub steps: Vec<Vec<String>>,
}

/// Walk the batch and collect one `BuildNotice` per `(manifest, mcp_name)`
/// pair whose manifest declares a non-empty `build` array. MCPs with only an
/// `entrypoint` (prebuilt) produce no notice because there's nothing to run.
pub fn collect_build_notices(manifests: &[&Manifest]) -> Vec<BuildNotice> {
    let mut out = Vec::new();
    for m in manifests {
        let Some(ref mcps) = m.mcp else { continue };
        let mut names: Vec<&String> = mcps.keys().collect();
        names.sort();
        for name in names {
            let server = &mcps[name];
            if let Some(ref build) = server.build {
                if !build.is_empty() {
                    out.push(BuildNotice {
                        full_name: m.name.clone(),
                        mcp_name: name.clone(),
                        steps: build.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Render the consolidated build block to a string. Public for
/// snapshot-style testing; the production path writes straight to stdout.
pub fn render_build_block(notices: &[BuildNotice]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        "Build notice: the following commands will execute with a minimal environment:"
            .yellow()
            .bold()
    ));
    for n in notices {
        let joined = n
            .steps
            .iter()
            .map(|s| s.join(" "))
            .collect::<Vec<_>>()
            .join(" && ");
        out.push_str(&format!(
            "  {} → {}: {}\n",
            n.full_name.bold(),
            n.mcp_name,
            joined
        ));
    }
    out.push_str("  (no shell: each step runs via execve with a filtered env)\n");
    out
}

/// Prompt (or auto-accept) before running the builds listed in `notices`.
///
/// Returns:
/// - `Ok(true)`  → no notices, OR `allow_build == true`, OR user accepted at the prompt.
/// - `Ok(false)` → user declined at the prompt (caller should exit 0).
/// - `Err(BuildRequiresConfirmation)` → there are notices but no TTY and `allow_build == false`.
pub fn confirm_builds(notices: &[BuildNotice], allow_build: bool) -> Result<bool> {
    confirm_block(
        notices.is_empty(),
        allow_build,
        RenkeiError::BuildRequiresConfirmation,
        || render_build_block(notices),
        "Run all builds?",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Tests in this module mutate process env; serialize them so they
    /// don't see each other's writes.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    use crate::manifest::{ManifestScope, McpServer};

    fn manifest_with_mcps(name: &str, mcps: Vec<(&str, McpServer)>) -> Manifest {
        let map: HashMap<String, McpServer> =
            mcps.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Manifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "x".to_string(),
            author: "a".to_string(),
            license: "MIT".to_string(),
            backends: vec!["claude".to_string()],
            keywords: vec![],
            scope: ManifestScope::default(),
            mcp: if map.is_empty() { None } else { Some(map) },
            required_env: None,
            messages: None,
        }
    }

    fn local_mcp(build: Option<Vec<Vec<&str>>>, entrypoint: Option<&str>) -> McpServer {
        McpServer {
            entrypoint: entrypoint.map(String::from),
            build: build.map(|b| {
                b.into_iter()
                    .map(|s| s.into_iter().map(String::from).collect())
                    .collect()
            }),
            extra: serde_json::Map::new(),
        }
    }

    #[test]
    fn collect_build_notices_skips_external_mcps() {
        let m = manifest_with_mcps(
            "@x/a",
            vec![(
                "external",
                McpServer {
                    entrypoint: None,
                    build: None,
                    extra: serde_json::Map::new(),
                },
            )],
        );
        assert!(collect_build_notices(&[&m]).is_empty());
    }

    #[test]
    fn collect_build_notices_skips_prebuilt_local_mcps() {
        // entrypoint set but no build → prebuilt, nothing to run
        let m = manifest_with_mcps(
            "@x/a",
            vec![("prebuilt", local_mcp(None, Some("dist/index.js")))],
        );
        assert!(collect_build_notices(&[&m]).is_empty());
    }

    #[test]
    fn collect_build_notices_emits_one_per_local_mcp_with_build() {
        let m = manifest_with_mcps(
            "@x/a",
            vec![
                (
                    "srv-1",
                    local_mcp(Some(vec![vec!["bun", "install"]]), Some("dist/index.js")),
                ),
                (
                    "srv-2",
                    local_mcp(
                        Some(vec![vec!["cargo", "build", "--release"]]),
                        Some("target/release/srv2"),
                    ),
                ),
            ],
        );
        let notices = collect_build_notices(&[&m]);
        assert_eq!(notices.len(), 2);
        // sorted by mcp name
        assert_eq!(notices[0].mcp_name, "srv-1");
        assert_eq!(notices[1].mcp_name, "srv-2");
        assert_eq!(notices[0].full_name, "@x/a");
        assert_eq!(
            notices[0].steps,
            vec![vec!["bun".to_string(), "install".to_string()]]
        );
    }

    #[test]
    fn collect_build_notices_preserves_manifest_order() {
        let a = manifest_with_mcps(
            "@x/a",
            vec![("foo", local_mcp(Some(vec![vec!["true"]]), Some("a.js")))],
        );
        let b = manifest_with_mcps(
            "@x/b",
            vec![("bar", local_mcp(Some(vec![vec!["true"]]), Some("b.js")))],
        );
        let notices = collect_build_notices(&[&a, &b]);
        assert_eq!(notices[0].full_name, "@x/a");
        assert_eq!(notices[1].full_name, "@x/b");
    }

    #[test]
    fn collect_build_notices_skips_empty_build_array() {
        let m = manifest_with_mcps(
            "@x/a",
            vec![("srv", local_mcp(Some(vec![]), Some("dist/index.js")))],
        );
        assert!(collect_build_notices(&[&m]).is_empty());
    }

    #[test]
    fn confirm_builds_with_no_notices_returns_true_without_prompt() {
        assert!(confirm_builds(&[], false).unwrap());
    }

    #[test]
    fn confirm_builds_with_allow_flag_returns_true_without_prompt() {
        let n = vec![BuildNotice {
            full_name: "@x/a".into(),
            mcp_name: "srv".into(),
            steps: vec![vec!["bun".into(), "install".into()]],
        }];
        assert!(confirm_builds(&n, true).unwrap());
    }

    #[test]
    fn confirm_builds_in_non_tty_without_allow_errors() {
        // cargo test stdin is not a TTY.
        let n = vec![BuildNotice {
            full_name: "@x/a".into(),
            mcp_name: "srv".into(),
            steps: vec![vec!["bun".into(), "install".into()]],
        }];
        let err = confirm_builds(&n, false).unwrap_err();
        assert!(matches!(err, RenkeiError::BuildRequiresConfirmation));
    }

    #[test]
    fn render_build_block_lists_each_notice() {
        let n = vec![
            BuildNotice {
                full_name: "@x/a".into(),
                mcp_name: "srv-1".into(),
                steps: vec![vec!["bun".into(), "install".into()], vec!["bun".into(), "run".into(), "build".into()]],
            },
            BuildNotice {
                full_name: "@x/b".into(),
                mcp_name: "srv-2".into(),
                steps: vec![vec!["cargo".into(), "build".into()]],
            },
        ];
        let out = render_build_block(&n);
        assert!(out.contains("Build notice"));
        assert!(out.contains("@x/a"));
        assert!(out.contains("srv-1"));
        assert!(out.contains("bun install && bun run build"));
        assert!(out.contains("@x/b"));
        assert!(out.contains("srv-2"));
        assert!(out.contains("cargo build"));
        assert!(out.contains("no shell"));
    }

    #[test]
    fn build_requires_confirmation_message_mentions_allow_build() {
        let msg = RenkeiError::BuildRequiresConfirmation.to_string();
        assert!(msg.contains("--allow-build"));
        assert!(msg.contains("non-interactive"));
    }

    #[test]
    fn test_should_keep_plain_whitelist() {
        assert!(should_keep("PATH"));
        assert!(should_keep("HOME"));
        assert!(should_keep("LANG"));
    }

    #[test]
    fn test_should_keep_lc_prefix() {
        assert!(should_keep("LC_ALL"));
        assert!(should_keep("LC_CTYPE"));
    }

    #[test]
    fn test_should_keep_proxy_case_insensitive() {
        assert!(should_keep("HTTP_PROXY"));
        assert!(should_keep("https_proxy"));
        assert!(should_keep("no_proxy"));
    }

    #[test]
    fn test_should_keep_certs() {
        assert!(should_keep("NODE_EXTRA_CA_CERTS"));
        assert!(should_keep("SSL_CERT_FILE"));
    }

    #[test]
    fn test_should_keep_tool_prefixes() {
        assert!(should_keep("npm_config_registry"));
        assert!(should_keep("CARGO_HOME"));
        assert!(should_keep("BUN_INSTALL"));
        assert!(should_keep("PIP_INDEX_URL"));
    }

    #[test]
    fn test_should_drop_secrets() {
        assert!(!should_keep("AWS_ACCESS_KEY_ID"));
        assert!(!should_keep("AWS_SECRET_ACCESS_KEY"));
        assert!(!should_keep("GITHUB_TOKEN"));
        assert!(!should_keep("GITLAB_PRIVATE_TOKEN"));
        assert!(!should_keep("ANTHROPIC_API_KEY"));
        assert!(!should_keep("OPENAI_API_KEY"));
        assert!(!should_keep("FOO_TOKEN"));
        assert!(!should_keep("MY_API_KEY"));
        assert!(!should_keep("DB_PASSWORD"));
        assert!(!should_keep("CLIENT_SECRET"));
    }

    #[test]
    fn test_should_drop_unknown_vars() {
        assert!(!should_keep("EDITOR"));
        assert!(!should_keep("RANDOM_VAR"));
    }

    #[test]
    fn test_build_env_filters_secrets() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "leak-1");
        std::env::set_var("FOO_TOKEN", "leak-2");
        std::env::set_var("npm_config_registry", "https://example.com");
        std::env::set_var("NODE_EXTRA_CA_CERTS", "/tmp/cert.pem");

        let env = build_env();

        assert!(!env.contains_key("AWS_SECRET_ACCESS_KEY"));
        assert!(!env.contains_key("FOO_TOKEN"));
        assert_eq!(env.get("npm_config_registry").map(String::as_str), Some("https://example.com"));
        assert_eq!(env.get("NODE_EXTRA_CA_CERTS").map(String::as_str), Some("/tmp/cert.pem"));

        std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        std::env::remove_var("FOO_TOKEN");
        std::env::remove_var("npm_config_registry");
        std::env::remove_var("NODE_EXTRA_CA_CERTS");
    }

    #[test]
    fn test_build_env_keeps_path() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("PATH", "/usr/bin:/bin");
        let env = build_env();
        assert!(env.contains_key("PATH"));
    }

    #[cfg(unix)]
    mod unix_runner {
        use super::*;
        use tempfile::tempdir;

        #[test]
        fn test_run_build_single_true_step_ok() {
            let dir = tempdir().unwrap();
            let steps = vec![BuildStep {
                argv: vec!["true".to_string()],
            }];
            run_build(&steps, dir.path()).unwrap();
        }

        #[test]
        fn test_run_build_false_step_fails() {
            let dir = tempdir().unwrap();
            let steps = vec![BuildStep {
                argv: vec!["false".to_string()],
            }];
            let err = run_build(&steps, dir.path()).unwrap_err();
            match err {
                RenkeiError::BuildFailed { step, exit_code } => {
                    assert_eq!(step, "false");
                    assert_eq!(exit_code, Some(1));
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }

        #[test]
        fn test_run_build_stops_at_first_failure() {
            let dir = tempdir().unwrap();
            let steps = vec![
                BuildStep {
                    argv: vec!["true".to_string()],
                },
                BuildStep {
                    argv: vec!["false".to_string()],
                },
                BuildStep {
                    argv: vec!["never-runs-binary-doesnotexist-9999".to_string()],
                },
            ];
            let err = run_build(&steps, dir.path()).unwrap_err();
            match err {
                RenkeiError::BuildFailed { step, .. } => assert_eq!(step, "false"),
                other => panic!("unexpected error: {other:?}"),
            }
        }

        #[test]
        fn test_run_build_respects_cwd() {
            let dir = tempdir().unwrap();
            let steps = vec![BuildStep {
                argv: vec!["sh".into(), "-c".into(), "pwd > out.txt".into()],
            }];
            run_build(&steps, dir.path()).unwrap();

            let pwd_out = std::fs::read_to_string(dir.path().join("out.txt")).unwrap();
            // Some platforms canonicalise /tmp via /private; compare suffix.
            let dir_str = dir.path().to_string_lossy().to_string();
            assert!(
                pwd_out.trim().ends_with(dir_str.trim_start_matches("/private")),
                "pwd={pwd_out:?} dir={dir_str:?}"
            );
        }

        #[test]
        fn test_run_build_env_isolation() {
            let _g = ENV_LOCK.lock().unwrap();
            std::env::set_var("AWS_SECRET_ACCESS_KEY", "should-not-leak");
            let dir = tempdir().unwrap();
            let steps = vec![BuildStep {
                argv: vec![
                    "sh".into(),
                    "-c".into(),
                    "printenv AWS_SECRET_ACCESS_KEY > leak.txt; printenv PATH > path.txt; true".into(),
                ],
            }];
            run_build(&steps, dir.path()).unwrap();

            let leak = std::fs::read_to_string(dir.path().join("leak.txt")).unwrap();
            assert!(leak.is_empty(), "secret leaked: {leak:?}");
            let path = std::fs::read_to_string(dir.path().join("path.txt")).unwrap();
            assert!(!path.is_empty(), "PATH must be inherited");

            std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        }

        #[test]
        fn test_run_build_unknown_binary_yields_build_failed() {
            let dir = tempdir().unwrap();
            let steps = vec![BuildStep {
                argv: vec!["this-binary-does-not-exist-rk-9999".into()],
            }];
            let err = run_build(&steps, dir.path()).unwrap_err();
            assert!(matches!(err, RenkeiError::BuildFailed { .. }));
        }
    }
}
