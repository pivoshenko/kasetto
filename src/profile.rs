use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use crate::fsops::now_unix;

pub(crate) fn read_skill_profile(destination: &str, fallback_name: &str) -> (String, String) {
    read_skill_profile_from_dir(Path::new(destination), fallback_name)
}

pub(crate) fn read_skill_profile_from_dir(
    skill_dir: &Path,
    fallback_name: &str,
) -> (String, String) {
    let skill_md = skill_dir.join("SKILL.md");
    let body = match fs::read_to_string(skill_md) {
        Ok(v) => v,
        Err(_) => return (fallback_name.to_string(), "No description.".to_string()),
    };

    let lines: Vec<&str> = body.lines().collect();
    let mut content_start = 0usize;
    let mut front_name: Option<String> = None;
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;

    if lines.first().map(|line| line.trim()) == Some("---") {
        for (idx, line) in lines.iter().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed == "---" {
                content_start = idx + 1;
                break;
            }
            if front_name.is_none() {
                if let Some(raw) = trimmed.strip_prefix("name:") {
                    let value = raw.trim();
                    if !value.is_empty() {
                        front_name = Some(value.to_string());
                    }
                }
            }
            if description.is_none() {
                if let Some(raw) = trimmed.strip_prefix("description:") {
                    let value = raw.trim();
                    if !value.is_empty() {
                        description = Some(value.to_string());
                    }
                }
            }
        }
    }

    let mut in_code = false;
    for line in lines.iter().skip(content_start) {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || trimmed.is_empty() {
            continue;
        }
        if title.is_none() && trimmed.starts_with('#') {
            title = Some(trimmed.trim_start_matches('#').trim().to_string());
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if description.is_none() {
            let candidate = trimmed
                .trim_start_matches('-')
                .trim_start_matches('*')
                .trim();
            if !candidate.is_empty() {
                description = Some(candidate.to_string());
            }
        }
        if title.is_some() && description.is_some() {
            break;
        }
    }

    (
        title
            .or(front_name)
            .unwrap_or_else(|| fallback_name.to_string()),
        description.unwrap_or_else(|| "No description.".to_string()),
    )
}

pub(crate) fn format_updated_ago(updated_at: &str) -> String {
    let ts = match updated_at.parse::<u64>() {
        Ok(v) => v,
        Err(_) => return "unknown".to_string(),
    };
    let now = now_unix();
    if ts > now {
        let d = ts - now;
        return format!("in {}s", d);
    }
    let d = now - ts;
    if d < 60 {
        format!("{}s ago", d)
    } else if d < 3600 {
        format!("{}m ago", d / 60)
    } else if d < 86_400 {
        format!("{}h ago", d / 3600)
    } else {
        format!("{}d ago", d / 86_400)
    }
}

pub(crate) fn list_color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn profile_prefers_heading_and_frontmatter_description() {
        let dir = temp_dir("kasetto-profile");
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: slug-name\ndescription: from-front-matter\n---\n\n# Human Title\n\nBody line.\n",
        )
        .expect("write skill");

        let (name, description) = read_skill_profile_from_dir(&dir, "fallback");
        assert_eq!(name, "Human Title");
        assert_eq!(description, "from-front-matter");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn profile_falls_back_when_file_missing() {
        let dir = temp_dir("kasetto-profile-missing");
        fs::create_dir_all(&dir).expect("create temp dir");

        let (name, description) = read_skill_profile_from_dir(&dir, "fallback-name");
        assert_eq!(name, "fallback-name");
        assert_eq!(description, "No description.");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_updated_ago_returns_unknown_for_invalid_input() {
        assert_eq!(format_updated_ago("not-a-timestamp"), "unknown");
    }
}
