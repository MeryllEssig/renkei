#[derive(Debug, Clone, PartialEq)]
pub enum PackageSource {
    Local(String),
    GitSsh(String),
    GitUrl(String),
}

pub fn parse_source(source: &str) -> PackageSource {
    if source.starts_with("git@") {
        PackageSource::GitSsh(source.to_string())
    } else if source.starts_with("https://") || source.starts_with("file://") {
        PackageSource::GitUrl(source.to_string())
    } else {
        PackageSource::Local(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_relative_path() {
        assert_eq!(
            parse_source("./local-path/"),
            PackageSource::Local("./local-path/".to_string())
        );
    }

    #[test]
    fn test_local_absolute_path() {
        assert_eq!(
            parse_source("/absolute/path"),
            PackageSource::Local("/absolute/path".to_string())
        );
    }

    #[test]
    fn test_local_plain_dir() {
        assert_eq!(
            parse_source("relative-dir"),
            PackageSource::Local("relative-dir".to_string())
        );
    }

    #[test]
    fn test_git_ssh() {
        assert_eq!(
            parse_source("git@github.com:user/repo"),
            PackageSource::GitSsh("git@github.com:user/repo".to_string())
        );
    }

    #[test]
    fn test_git_ssh_with_dot_git() {
        assert_eq!(
            parse_source("git@github.com:user/repo.git"),
            PackageSource::GitSsh("git@github.com:user/repo.git".to_string())
        );
    }

    #[test]
    fn test_git_https() {
        assert_eq!(
            parse_source("https://github.com/user/repo"),
            PackageSource::GitUrl("https://github.com/user/repo".to_string())
        );
    }

    #[test]
    fn test_git_https_with_dot_git() {
        assert_eq!(
            parse_source("https://github.com/user/repo.git"),
            PackageSource::GitUrl("https://github.com/user/repo.git".to_string())
        );
    }

    #[test]
    fn test_file_url_treated_as_git() {
        assert_eq!(
            parse_source("file:///tmp/bare-repo"),
            PackageSource::GitUrl("file:///tmp/bare-repo".to_string())
        );
    }

    #[test]
    fn test_http_not_supported_as_git() {
        assert_eq!(
            parse_source("http://example.com/repo"),
            PackageSource::Local("http://example.com/repo".to_string())
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(parse_source(""), PackageSource::Local("".to_string()));
    }
}
