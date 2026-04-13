use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{RenkeiError, Result};

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
#[allow(dead_code)]
pub fn build_env() -> HashMap<String, String> {
    std::env::vars().filter(|(k, _)| should_keep(k)).collect()
}

#[allow(dead_code)]
pub struct BuildStep {
    pub argv: Vec<String>,
}

/// Run a sequence of build steps under `cwd` with the filtered env. No
/// shell — argv goes straight to the kernel via `execve`. Streams stdout
/// and stderr live so the user sees progress; first non-zero exit aborts
/// and surfaces `BuildFailed`.
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Tests in this module mutate process env; serialize them so they
    /// don't see each other's writes.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

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
