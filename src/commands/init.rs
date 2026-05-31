use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use crate::banner::print_banner;
use crate::colors::{ACCENT, ATTENTION, RESET, SECONDARY, SUCCESS};
use crate::error::{err, Result};
use crate::fsops::dirs_kasetto_config;
use crate::{DEFAULT_CONFIG_FILENAME, DEFAULT_GLOBAL_CONFIG_FILENAME};

const TEMPLATE: &str = r#"# Kasetto - https://github.com/pivoshenko/kasetto

# Scope: global (default) or project (install into current project)
# scope: project

# Target agent (see README for supported values)
# agent: claude-code

# Or set a custom skills directory
# destination: ~/.claude/skills

# skills:
#   - source: https://github.com/example/skill-pack
#     skills: "*"
#   - source: https://github.com/example/skill-pack
#     ref: v2.0            # pin to a git tag, commit SHA, or any ref
#     skills: "*"
#   - source: https://github.com/example/skill-pack
#     branch: develop       # track a specific branch
#     skills: "*"

# mcps:
#   - source: https://github.com/example/mcp-pack
#     mcps: "*"
#   - source: https://github.com/example/monorepo
#     ref: v1.0
#     mcps:
#       - github         # → mcps/github.json
#       - linear         # → mcps/linear.json
#   - source: https://github.com/example/other
#     mcps:
#       - name: my-server
#         path: tools    # → tools/my-server.json

# commands:
#   - source: https://github.com/example/commands
#     commands: "*"
#   - source: https://github.com/example/commands
#     ref: v1.0
#     sub-dir: commands
#     commands:
#       - review-pr
#       - name: deploy
#         path: ops
"#;

pub(crate) fn run(force: bool, global: bool) -> Result<()> {
    print_banner();
    println!();
    let path = init_config_path(global)?;

    if path.exists() && !force {
        println!(
            "{ATTENTION}\x1b[1mwarning:{RESET} {} already exists",
            path.display()
        );
        if io::stdin().is_terminal() {
            print!("{ACCENT}Overwrite?{RESET} [y/N] ");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            if !matches!(buf.trim(), "y" | "Y" | "yes") {
                println!("{SECONDARY}Cancelled.{RESET}");
                return Ok(());
            }
        } else {
            return Err(err(format!(
                "{} already exists (use --force to overwrite)",
                path.display()
            )));
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, TEMPLATE)?;

    println!(
        "{SUCCESS}\x1b[1mCreated{RESET} {ACCENT}{}{RESET}",
        path.display()
    );
    println!();
    println!("{ACCENT}Next steps{RESET}");
    println!("  1. Edit {ACCENT}{}{RESET} to add your sources and target agent", path.display());
    println!(
        "  2. For private repos set {ACCENT}GITHUB_TOKEN{RESET} / {ACCENT}GH_TOKEN{RESET} / {ACCENT}GITLAB_TOKEN{RESET}",
    );
    println!("  3. Run {ACCENT}kasetto sync{RESET} to install");

    Ok(())
}

fn init_config_path(global: bool) -> Result<PathBuf> {
    if global {
        return Ok(dirs_kasetto_config()?.join(DEFAULT_GLOBAL_CONFIG_FILENAME));
    }
    Ok(PathBuf::from(DEFAULT_CONFIG_FILENAME))
}

#[cfg(test)]
mod tests {
    use super::init_config_path;

    #[test]
    fn init_path_defaults_to_local_config() {
        let path = init_config_path(false).expect("local path");
        assert_eq!(path, std::path::PathBuf::from("kasetto.yaml"));
    }

    #[test]
    fn init_path_global_uses_kasetto_config_dir() {
        let path = init_config_path(true).expect("global path");
        assert!(path.ends_with("kasetto/kasetto.yaml"));
    }
}
