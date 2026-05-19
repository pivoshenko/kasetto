use crate::error::{err, Result};

/// A parsed Markdown-with-YAML-frontmatter command source.
#[derive(Debug, Clone)]
pub(crate) struct Parsed {
    /// Frontmatter YAML text (between the `---` fences), without the fences.
    /// `None` if the source had no frontmatter.
    pub frontmatter: Option<String>,
    /// Body content after the closing `---` fence (or the whole file if none).
    pub body: String,
}

impl Parsed {
    pub(crate) fn description(&self) -> Option<String> {
        let fm = self.frontmatter.as_deref()?;
        let value: serde_yaml::Value = serde_yaml::from_str(fm).ok()?;
        value
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }
}

/// Split a Markdown file into (frontmatter, body).
///
/// Frontmatter is recognized only when the file starts with `---` on its own
/// line and a matching closing `---` line is present.
pub(crate) fn parse(text: &str) -> Result<Parsed> {
    let normalized = text.replace("\r\n", "\n");
    let stripped = normalized.strip_prefix("---\n");
    let Some(rest) = stripped else {
        return Ok(Parsed {
            frontmatter: None,
            body: normalized,
        });
    };
    // Find a line that is exactly "---" or "---\n" inside rest.
    let mut idx = 0usize;
    let bytes = rest.as_bytes();
    let mut found: Option<(usize, usize)> = None;
    while idx < bytes.len() {
        let line_end = rest[idx..]
            .find('\n')
            .map(|n| idx + n)
            .unwrap_or(bytes.len());
        let line = &rest[idx..line_end];
        if line == "---" {
            // Frontmatter ends at idx; body starts after line_end + 1 (skip newline).
            let body_start = (line_end + 1).min(bytes.len());
            found = Some((idx, body_start));
            break;
        }
        idx = line_end + 1;
    }
    let Some((fm_end, body_start)) = found else {
        return Err(err(
            "command source has an opening `---` but no closing `---` for the frontmatter",
        ));
    };
    let frontmatter = rest[..fm_end].trim_end_matches('\n').to_string();
    let body = rest[body_start..].to_string();
    Ok(Parsed {
        frontmatter: Some(frontmatter),
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let text = "---\ndescription: hi\nargument-hint: <n>\n---\nBody here.\n";
        let p = parse(text).unwrap();
        assert!(p
            .frontmatter
            .as_deref()
            .unwrap()
            .contains("description: hi"));
        assert_eq!(p.body, "Body here.\n");
        assert_eq!(p.description().as_deref(), Some("hi"));
    }

    #[test]
    fn no_frontmatter_means_whole_body() {
        let p = parse("just markdown\n").unwrap();
        assert!(p.frontmatter.is_none());
        assert_eq!(p.body, "just markdown\n");
    }

    #[test]
    fn missing_closing_fence_is_error() {
        let text = "---\ndescription: nope\nBody never closed.\n";
        assert!(parse(text).is_err());
    }
}
