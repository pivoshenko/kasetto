use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use crate::banner::print_banner;
use crate::colors::{ACCENT, RESET, SECONDARY, SUCCESS, WARNING};
use crate::error::{err, Result};
use crate::ui::{SYM_FAIL, SYM_OK};
use crate::DEFAULT_CONFIG_FILENAME;

const TEMPLATE: &str = r#"# Kasetto — https://github.com/pivoshenko/kasetto

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
#   - source: https://github.com/example/mcp-pack
#     ref: v1.0
#   - source: https://github.com/example/repo
#     path: .mcp.json         # explicit path to MCP JSON within the repo
"#;

pub(crate) fn run(force: bool) -> Result<()> {
    print_banner();
    println!();
    let path = Path::new(DEFAULT_CONFIG_FILENAME);

    if path.exists() && !force {
        println!(
            "{WARNING}{SYM_FAIL}{RESET} {} already exists",
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

    fs::write(path, TEMPLATE)?;

    println!(
        "{SUCCESS}{SYM_OK}{RESET} Created {ACCENT}{}{RESET}",
        path.display()
    );
    println!();
    println!("{ACCENT}Next steps:{RESET}");
    println!(
        "  1. Edit {ACCENT}{}{RESET} for your sources and agent",
        path.display()
    );
    println!("  2. For private GitHub / GHE use {ACCENT}GITHUB_TOKEN{RESET} or {ACCENT}GH_TOKEN{RESET}; for GitLab use {ACCENT}GITLAB_TOKEN{RESET}");
    println!("  3. Run {ACCENT}kasetto sync{RESET} to install skills");

    Ok(())
}
