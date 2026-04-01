use crate::error::{RenkeiError, Result};

/// Replace the `name:` field value in a markdown frontmatter block.
///
/// Expects content with YAML frontmatter delimited by `---`.
/// Returns the full content with the name field updated.
pub fn replace_frontmatter_name(content: &str, new_name: &str) -> Result<String> {
    let Some((before, fm, after)) = split_frontmatter(content) else {
        return Err(RenkeiError::InvalidManifest(
            "No frontmatter found in artifact".into(),
        ));
    };

    let mut found = false;
    let mut new_lines: Vec<String> = Vec::new();
    for line in fm.lines() {
        if line.trim_start().starts_with("name:") {
            new_lines.push(format!("name: {new_name}"));
            found = true;
        } else {
            new_lines.push(line.to_string());
        }
    }

    if !found {
        return Err(RenkeiError::InvalidManifest(
            "No 'name' field found in frontmatter".into(),
        ));
    }

    Ok(format!("{before}---\n{}\n---{after}", new_lines.join("\n")))
}

/// Split content into (before_first_delimiter, frontmatter_body, after_second_delimiter).
/// Returns None if no valid frontmatter block is found.
fn split_frontmatter(content: &str) -> Option<(&str, &str, &str)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let offset = content.len() - trimmed.len();
    let before = &content[..offset];
    let rest = &trimmed[3..]; // skip first "---"

    // Skip the newline after first ---
    let rest = rest.strip_prefix('\n').unwrap_or(rest);

    let end_pos = rest.find("\n---")?;
    let fm = &rest[..end_pos];
    let after = &rest[end_pos + 4..]; // skip "\n---"

    Some((before, fm, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_name_basic() {
        let content = "---\nname: review\ndescription: Review code\n---\nBody content here.";
        let result = replace_frontmatter_name(content, "review-v2").unwrap();
        assert!(result.contains("name: review-v2"));
        assert!(result.contains("description: Review code"));
        assert!(result.contains("Body content here."));
    }

    #[test]
    fn test_replace_name_preserves_body() {
        let content = "---\nname: old\n---\nLine 1\nLine 2\nLine 3";
        let result = replace_frontmatter_name(content, "new").unwrap();
        assert!(result.contains("name: new"));
        assert!(result.contains("Line 1\nLine 2\nLine 3"));
    }

    #[test]
    fn test_replace_name_preserves_other_fields() {
        let content = "---\nname: deploy\ndescription: Deploy app\nauthor: test\n---\nContent";
        let result = replace_frontmatter_name(content, "deploy-2").unwrap();
        assert!(result.contains("name: deploy-2"));
        assert!(result.contains("description: Deploy app"));
        assert!(result.contains("author: test"));
    }

    #[test]
    fn test_no_frontmatter_returns_error() {
        let content = "Just some text without frontmatter.";
        let result = replace_frontmatter_name(content, "new");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No frontmatter"));
    }

    #[test]
    fn test_no_name_field_returns_error() {
        let content = "---\ndescription: No name here\n---\nBody";
        let result = replace_frontmatter_name(content, "new");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No 'name' field"));
    }

    #[test]
    fn test_replace_name_empty_body() {
        let content = "---\nname: review\n---\n";
        let result = replace_frontmatter_name(content, "lint").unwrap();
        assert!(result.contains("name: lint"));
    }

    #[test]
    fn test_split_frontmatter_valid() {
        let content = "---\nname: test\n---\nbody";
        let (before, fm, after) = split_frontmatter(content).unwrap();
        assert_eq!(before, "");
        assert_eq!(fm, "name: test");
        assert_eq!(after, "\nbody");
    }

    #[test]
    fn test_split_frontmatter_no_delimiters() {
        assert!(split_frontmatter("no frontmatter").is_none());
    }

    #[test]
    fn test_split_frontmatter_only_one_delimiter() {
        assert!(split_frontmatter("---\nname: test").is_none());
    }
}
