//! Package that contains subagent parsing and per-agent transforms.
//!
//! Named `subagents` rather than `agents` so it does not read as the
//! `model::Agent` tool registry.

mod transform;

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::model::AgentTarget;

pub(crate) use transform::destination_path;

/// Read a Markdown subagent at `source`, transform it for `target.format`, and
/// write it under `target.path`.
///
/// Returns the written path and the source fields or tools the format could not
/// carry, so the caller can warn about each one.
pub(crate) fn apply_agent(
    source: &Path,
    target: &AgentTarget,
    name: &str,
) -> Result<(PathBuf, Vec<String>)> {
    let text = fs::read_to_string(source).map_err(|e| {
        err(format!(
            "failed to read agent source {}: {e}",
            source.display()
        ))
    })?;
    let rendered = transform::render(&text, name, target.format)?;
    let dest = destination_path(target, name);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&dest, rendered.text)
        .map_err(|e| err(format!("failed to write agent {}: {e}", dest.display())))?;
    Ok((dest, rendered.dropped))
}

/// Check that a subagent source parses and carries what every format needs,
/// without writing anything. `sync --dry-run` uses it to report a broken agent.
pub(crate) fn validate_agent(source: &Path, name: &str) -> Result<()> {
    let text = fs::read_to_string(source).map_err(|e| {
        err(format!(
            "failed to read agent source {}: {e}",
            source.display()
        ))
    })?;
    transform::parse_source(&text, name).map(|_| ())
}

/// The source fields or tools `target.format` would drop, without writing.
/// `sync --dry-run` uses it so the preview warns like the real run.
pub(crate) fn dropped_fields(
    source: &Path,
    target: &AgentTarget,
    name: &str,
) -> Result<Vec<String>> {
    let text = fs::read_to_string(source).map_err(|e| {
        err(format!(
            "failed to read agent source {}: {e}",
            source.display()
        ))
    })?;
    Ok(transform::render(&text, name, target.format)?.dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::AgentFormat;

    #[test]
    fn apply_agent_writes_format_filename_and_reports_drops() {
        let src_dir = temp_dir("kasetto-sa-src");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("reviewer.md");
        fs::write(
            &src,
            "---\nname: reviewer\ndescription: Reviews\ncolor: blue\n---\nReview it.\n",
        )
        .unwrap();

        let dst_dir = temp_dir("kasetto-sa-dst");
        let target = AgentTarget {
            path: dst_dir.clone(),
            format: AgentFormat::Copilot,
        };
        let (out, dropped) = apply_agent(&src, &target, "reviewer").unwrap();
        assert!(out.ends_with("reviewer.agent.md"));
        assert_eq!(dropped, vec!["color".to_string()]);
        assert!(fs::read_to_string(&out).unwrap().contains("Review it."));

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
    }
}
