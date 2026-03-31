use owo_colors::OwoColorize;

pub struct MissingEnvVar {
    pub name: String,
    pub description: String,
}

pub fn check_required_env(required_env: &serde_json::Value) -> Vec<MissingEnvVar> {
    let obj = match required_env.as_object() {
        Some(o) => o,
        None => return Vec::new(),
    };

    let mut missing = Vec::new();
    for (name, desc) in obj {
        if std::env::var(name).is_err() {
            missing.push(MissingEnvVar {
                name: name.clone(),
                description: desc.as_str().unwrap_or("").to_string(),
            });
        }
    }
    missing
}

pub fn print_env_warnings(missing: &[MissingEnvVar]) {
    println!(
        "\n{}",
        "Missing environment variables:".yellow().bold()
    );
    for var in missing {
        println!(
            "  {} {}: {}",
            "Warning:".yellow().bold(),
            var.name.bold(),
            var.description
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_present() {
        unsafe { std::env::set_var("RK_TEST_ENV_A1", "value") };
        let env = serde_json::json!({ "RK_TEST_ENV_A1": "desc" });
        let missing = check_required_env(&env);
        assert!(missing.is_empty());
        unsafe { std::env::remove_var("RK_TEST_ENV_A1") };
    }

    #[test]
    fn test_one_missing() {
        std::env::remove_var("RK_TEST_ENV_B1");
        let env = serde_json::json!({ "RK_TEST_ENV_B1": "API key for testing" });
        let missing = check_required_env(&env);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].name, "RK_TEST_ENV_B1");
        assert_eq!(missing[0].description, "API key for testing");
    }

    #[test]
    fn test_mixed_present_and_missing() {
        unsafe { std::env::set_var("RK_TEST_ENV_C1", "value") };
        std::env::remove_var("RK_TEST_ENV_C2");
        let env = serde_json::json!({
            "RK_TEST_ENV_C1": "present var",
            "RK_TEST_ENV_C2": "missing var"
        });
        let missing = check_required_env(&env);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].name, "RK_TEST_ENV_C2");
        unsafe { std::env::remove_var("RK_TEST_ENV_C1") };
    }

    #[test]
    fn test_empty_required_env() {
        let env = serde_json::json!({});
        let missing = check_required_env(&env);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_invalid_required_env_not_object() {
        let env = serde_json::json!("not an object");
        let missing = check_required_env(&env);
        assert!(missing.is_empty());
    }
}
