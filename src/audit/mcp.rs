//! Module that contains the offline checks kasetto runs on an installed MCP
//! server definition.
//!
//! The definition is read back from the agent config, where secret placeholders
//! are already resolved, so no verdict summary ever echoes an env value, a
//! header, a URL path or query, or a shell script: rules name the problem and at
//! most a host or package name.

use std::path::Path;

use super::{RiskLevel, Verdict};

/// Provider name on every verdict these rules produce.
pub(super) const PROVIDER: &str = "kasetto";

/// The launch-relevant fields of a server definition, normalized across the
/// agent-native shapes kasetto writes (`mcpServers`, VS Code, OpenCode's
/// command array and `environment`, Codex TOML, Gemini's `httpUrl`).
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ServerSpec {
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub env: Vec<(String, String)>,
}

impl ServerSpec {
    pub(super) fn from_definition(def: &serde_json::Value) -> ServerSpec {
        let text = |v: &serde_json::Value| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let mut spec = ServerSpec::default();
        match def.get("command") {
            Some(serde_json::Value::String(c)) => spec.command = Some(c.clone()),
            Some(serde_json::Value::Array(parts)) => {
                let mut parts = parts.iter().map(text);
                spec.command = parts.next();
                spec.args.extend(parts);
            }
            _ => {}
        }
        if let Some(args) = def.get("args").and_then(|a| a.as_array()) {
            spec.args.extend(args.iter().map(text));
        }
        spec.url = ["url", "serverUrl", "httpUrl"]
            .iter()
            .find_map(|k| def.get(*k).and_then(|u| u.as_str()))
            .map(str::to_string);
        for key in ["env", "environment"] {
            if let Some(env) = def.get(key).and_then(|e| e.as_object()) {
                spec.env
                    .extend(env.iter().map(|(k, v)| (k.clone(), text(v))));
            }
        }
        spec
    }

    /// The command's file name without a Windows `.exe` / `.cmd` suffix, so
    /// `/usr/local/bin/npx` and `npx.cmd` both read as `npx`.
    fn program(&self) -> Option<String> {
        let cmd = self.command.as_deref()?;
        let base = Path::new(cmd).file_name()?.to_string_lossy().to_lowercase();
        Some(
            base.strip_suffix(".exe")
                .or_else(|| base.strip_suffix(".cmd"))
                .unwrap_or(&base)
                .to_string(),
        )
    }
}

/// A package registry the launched server is pulled from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ecosystem {
    Npm,
    PyPI,
}

impl Ecosystem {
    /// The ecosystem name as OSV spells it.
    pub(super) fn osv_name(self) -> &'static str {
        match self {
            Ecosystem::Npm => "npm",
            Ecosystem::PyPI => "PyPI",
        }
    }
}

/// The package a server launches through a package runner (`npx`, `uvx`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Package {
    pub ecosystem: Ecosystem,
    pub name: String,
    /// The exact version the definition pins, `None` when it floats (no
    /// version, a dist-tag like `latest`, or a range).
    pub version: Option<String>,
}

/// Run every rule over `spec`.
pub(super) fn check(spec: &ServerSpec) -> Vec<Verdict> {
    let mut out = Vec::new();
    check_transport(spec, &mut out);
    check_tls(spec, &mut out);
    check_shell(spec, &mut out);
    check_container(spec, &mut out);
    if let Some(pkg) = launched_package(spec) {
        if pkg.version.is_none() {
            out.push(finding(
                RiskLevel::Low,
                "unpinned-package",
                format!(
                    "runs {} without an exact version, so each launch may fetch new code",
                    pkg.name
                ),
            ));
        }
    }
    out
}

fn finding(level: RiskLevel, rule: &str, summary: String) -> Verdict {
    Verdict {
        provider: PROVIDER.to_string(),
        level,
        summary,
        audited_at: None,
        categories: vec![rule.to_string()],
    }
}

/// A remote server reached over plain `http://` sends its auth headers in the
/// clear. Loopback is exempt: that traffic never leaves the machine.
fn check_transport(spec: &ServerSpec, out: &mut Vec<Verdict>) {
    let Some(rest) = spec.url.as_deref().and_then(|u| u.strip_prefix("http://")) else {
        return;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host.starts_with("127.")
        || host == "::1"
        || host == "0.0.0.0";
    if !loopback {
        out.push(finding(
            RiskLevel::High,
            "insecure-transport",
            format!("connects to {host} over plain http"),
        ));
    }
}

fn check_tls(spec: &ServerSpec, out: &mut Vec<Verdict>) {
    let disabled = spec.env.iter().any(|(k, v)| {
        matches!(
            (k.as_str(), v.trim()),
            ("NODE_TLS_REJECT_UNAUTHORIZED", "0") | ("PYTHONHTTPSVERIFY", "0")
        )
    });
    if disabled {
        out.push(finding(
            RiskLevel::High,
            "tls-verification-disabled",
            "disables TLS certificate verification through its environment".to_string(),
        ));
    }
}

/// A server launched as an inline shell script runs whatever the script says;
/// one that downloads a script and pipes it into a shell runs whatever the
/// remote host serves on that launch.
fn check_shell(spec: &ServerSpec, out: &mut Vec<Verdict>) {
    let Some(program) = spec.program() else {
        return;
    };
    let is_shell = matches!(
        program.as_str(),
        "sh" | "bash" | "zsh" | "dash" | "fish" | "pwsh" | "powershell" | "cmd"
    );
    let inline = spec.args.iter().any(|a| {
        matches!(
            a.to_ascii_lowercase().as_str(),
            "-c" | "-lc" | "/c" | "-command" | "-encodedcommand"
        )
    });
    if !is_shell || !inline {
        return;
    }
    let script = spec.args.join(" ").to_ascii_lowercase();
    let fetches = ["curl ", "wget ", "invoke-webrequest", "iwr "]
        .iter()
        .any(|f| script.contains(f));
    let pipes_to_shell = [
        "| sh", "|sh", "| bash", "|bash", "| iex", "|iex", "| zsh", "|zsh",
    ]
    .iter()
    .any(|p| script.contains(p));
    if fetches && pipes_to_shell {
        out.push(finding(
            RiskLevel::Critical,
            "pipe-to-shell",
            format!("downloads a script and pipes it into {program} on every launch"),
        ));
    } else {
        out.push(finding(
            RiskLevel::Medium,
            "inline-shell",
            format!("launches through an inline {program} script"),
        ));
    }
}

/// `docker run` / `podman run` options that hand the container the host.
fn check_container(spec: &ServerSpec, out: &mut Vec<Verdict>) {
    let Some(program) = spec.program() else {
        return;
    };
    if !matches!(program.as_str(), "docker" | "podman") {
        return;
    }
    let Some(run_at) = spec.args.iter().position(|a| a == "run") else {
        return;
    };
    let args = &spec.args[run_at + 1..];
    let mut image = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let (flag, inline_value) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with('-') => (f, Some(v)),
            _ => (arg, None),
        };
        if !flag.starts_with('-') {
            image = Some(arg);
            break;
        }
        let takes_value = DOCKER_VALUE_FLAGS.contains(&flag);
        let value = match (inline_value, takes_value) {
            (Some(v), _) => Some(v),
            (None, true) => {
                i += 1;
                args.get(i).map(String::as_str)
            }
            (None, false) => None,
        };
        container_flag(flag, value, out);
        i += 1;
    }
    if let Some(image) = image {
        let tag = image.rsplit('/').next().unwrap_or(image).split_once(':');
        let pinned = image.contains("@sha256:") || matches!(tag, Some((_, t)) if t != "latest");
        if !pinned {
            out.push(finding(
                RiskLevel::Low,
                "unpinned-image",
                format!(
                    "runs image {} without a fixed tag or digest",
                    image.split('@').next().unwrap_or(image)
                ),
            ));
        }
    }
}

/// `docker run` flags that consume the next argument, so it is not mistaken
/// for the image.
const DOCKER_VALUE_FLAGS: &[&str] = &[
    "-e",
    "--env",
    "--env-file",
    "-v",
    "--volume",
    "--mount",
    "--name",
    "--network",
    "--net",
    "-p",
    "--publish",
    "-w",
    "--workdir",
    "-u",
    "--user",
    "--entrypoint",
    "--platform",
    "--pid",
    "--ipc",
    "--cap-add",
    "--cap-drop",
    "-l",
    "--label",
    "-h",
    "--hostname",
    "--add-host",
    "--security-opt",
    "--memory",
    "-m",
    "--cpus",
    "--pull",
    "--restart",
    "--device",
];

fn container_flag(flag: &str, value: Option<&str>, out: &mut Vec<Verdict>) {
    let value = value.unwrap_or("");
    match flag {
        "--privileged" => out.push(finding(
            RiskLevel::High,
            "privileged-container",
            "runs a privileged container with full access to the host".to_string(),
        )),
        "--network" | "--net" | "--pid" | "--ipc" if value == "host" => out.push(finding(
            RiskLevel::High,
            "host-namespace",
            format!("shares the host {} namespace", flag.trim_start_matches('-')),
        )),
        "--cap-add" if matches!(value.to_ascii_uppercase().as_str(), "ALL" | "SYS_ADMIN") => out
            .push(finding(
                RiskLevel::High,
                "added-capability",
                format!("grants the container {}", value.to_ascii_uppercase()),
            )),
        "-v" | "--volume" | "--mount" => {
            let source = if flag == "--mount" {
                value
                    .split(',')
                    .find_map(|kv| {
                        kv.strip_prefix("source=")
                            .or_else(|| kv.strip_prefix("src="))
                    })
                    .unwrap_or("")
            } else {
                value.split(':').next().unwrap_or("")
            };
            if source.ends_with("docker.sock") {
                out.push(finding(
                    RiskLevel::Critical,
                    "docker-socket",
                    "mounts the Docker socket, which controls the host".to_string(),
                ));
            } else if source == "/" {
                out.push(finding(
                    RiskLevel::Critical,
                    "host-root-mount",
                    "mounts the host's root filesystem".to_string(),
                ));
            }
        }
        _ => {}
    }
}

/// The registry package a server is launched from, when its command is a
/// package runner. Local paths, git URLs, and tarballs are not packages.
pub(super) fn launched_package(spec: &ServerSpec) -> Option<Package> {
    let program = spec.program()?;
    let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
    let (ecosystem, rest) = match (program.as_str(), args.as_slice()) {
        ("npx" | "bunx", rest) => (Ecosystem::Npm, rest),
        ("pnpm" | "yarn" | "bun", ["dlx" | "x", rest @ ..]) => (Ecosystem::Npm, rest),
        ("npm", ["exec" | "x", rest @ ..]) => (Ecosystem::Npm, rest),
        ("uvx", rest) => (Ecosystem::PyPI, rest),
        ("uv", ["tool", "run", rest @ ..]) => (Ecosystem::PyPI, rest),
        ("pipx", ["run", rest @ ..]) => (Ecosystem::PyPI, rest),
        _ => return None,
    };
    let spec_arg = package_arg(ecosystem, rest)?;
    match ecosystem {
        Ecosystem::Npm => parse_npm_spec(spec_arg),
        Ecosystem::PyPI => parse_pypi_spec(spec_arg),
    }
}

/// The package spec among a runner's arguments: an explicit `--package` /
/// `--from` / `--spec` value wins, else the first positional argument.
fn package_arg<'a>(ecosystem: Ecosystem, args: &[&'a str]) -> Option<&'a str> {
    let (explicit, value_flags): (&[&str], &[&str]) = match ecosystem {
        Ecosystem::Npm => (
            &["-p", "--package"],
            &["-c", "--call", "--registry", "--cache"],
        ),
        Ecosystem::PyPI => (
            &["--from", "--spec"],
            &[
                "--with",
                "--python",
                "-p",
                "--index",
                "--index-url",
                "--extra-index-url",
                "--pip-args",
            ],
        ),
    };
    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if let Some((flag, value)) = arg.split_once('=') {
            if explicit.contains(&flag) {
                return Some(value);
            }
        }
        if explicit.contains(&arg) {
            return args.get(i + 1).copied();
        }
        if arg == "--" {
            return args.get(i + 1).copied();
        }
        if !arg.starts_with('-') {
            return Some(arg);
        }
        if value_flags.contains(&arg) {
            i += 1;
        }
        i += 1;
    }
    None
}

fn parse_npm_spec(spec: &str) -> Option<Package> {
    let spec = spec.strip_prefix("npm:").unwrap_or(spec);
    if spec.is_empty() || spec.starts_with(['.', '/', '~']) || spec.contains("://") {
        return None;
    }
    if spec.contains(':') || spec.ends_with(".tgz") {
        return None;
    }
    // A scoped name carries its own leading `@`; the version separator is the
    // last `@` after it.
    let (name, version) = match spec[1..].rfind('@') {
        Some(at) => (&spec[..at + 1], Some(&spec[at + 2..])),
        None => (spec, None),
    };
    if name.is_empty() || (name.starts_with('@') && !name.contains('/')) {
        return None;
    }
    Some(Package {
        ecosystem: Ecosystem::Npm,
        name: name.to_string(),
        version: version.filter(|v| is_exact_version(v)).map(str::to_string),
    })
}

fn parse_pypi_spec(spec: &str) -> Option<Package> {
    if spec.is_empty() || spec.starts_with(['.', '/', '~']) || spec.contains("://") {
        return None;
    }
    let (name, version) = if let Some((n, v)) = spec.split_once("==") {
        (n, Some(v))
    } else if let Some((n, v)) = spec.split_once('@') {
        (n, Some(v))
    } else {
        let end = spec.find(['<', '>', '~', '!', '=']).unwrap_or(spec.len());
        (&spec[..end], None)
    };
    let name = name.split('[').next().unwrap_or(name).trim();
    if name.is_empty() {
        return None;
    }
    Some(Package {
        ecosystem: Ecosystem::PyPI,
        name: name.to_string(),
        version: version
            .map(str::trim)
            .filter(|v| is_exact_version(v))
            .map(str::to_string),
    })
}

/// A concrete release (`1.2.3`, `0.4.0-beta.1`), not a dist-tag or a range.
fn is_exact_version(v: &str) -> bool {
    v.starts_with(|c: char| c.is_ascii_digit())
        && !v.contains(['^', '~', '>', '<', '*', '|', ' ', ','])
        && !v.split('.').any(|p| p.eq_ignore_ascii_case("x"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(def: serde_json::Value) -> ServerSpec {
        ServerSpec::from_definition(&def)
    }

    fn rules(def: serde_json::Value) -> Vec<(String, RiskLevel)> {
        check(&spec(def))
            .into_iter()
            .map(|v| (v.categories[0].clone(), v.level))
            .collect()
    }

    #[test]
    fn normalizes_every_agent_shape() {
        let mcp_servers = spec(json!({"command": "npx", "args": ["-y", "pkg"], "env": {"A": "1"}}));
        let opencode = spec(
            json!({"type": "local", "command": ["npx", "-y", "pkg"], "environment": {"A": "1"}}),
        );
        assert_eq!(mcp_servers, opencode);
        assert_eq!(mcp_servers.command.as_deref(), Some("npx"));
        assert_eq!(mcp_servers.args, vec!["-y", "pkg"]);
        assert_eq!(
            spec(json!({"httpUrl": "https://x.dev/mcp"})).url.as_deref(),
            Some("https://x.dev/mcp")
        );
    }

    #[test]
    fn plain_http_is_flagged_except_on_loopback() {
        assert_eq!(
            rules(json!({"url": "http://user:tok@mcp.example.com:8080/sse?key=abc"})),
            vec![("insecure-transport".into(), RiskLevel::High)]
        );
        for local in [
            "http://localhost:3000/mcp",
            "http://127.0.0.1/mcp",
            "http://[::1]:9000",
        ] {
            assert!(rules(json!({ "url": local })).is_empty(), "{local}");
        }
        assert!(rules(json!({"url": "https://mcp.example.com"})).is_empty());
    }

    #[test]
    fn transport_summary_never_echoes_credentials_or_path() {
        let v = check(&spec(
            json!({"url": "http://user:s3cret@mcp.example.com/p?token=abc"}),
        ));
        assert_eq!(v[0].summary, "connects to mcp.example.com over plain http");
    }

    #[test]
    fn tls_off_in_env_is_flagged() {
        assert_eq!(
            rules(json!({"url": "https://x.dev", "env": {"NODE_TLS_REJECT_UNAUTHORIZED": "0"}})),
            vec![("tls-verification-disabled".into(), RiskLevel::High)]
        );
    }

    #[test]
    fn shell_launches_are_graded_by_what_they_run() {
        assert_eq!(
            rules(json!({"command": "bash", "args": ["-c", "curl -fsSL https://x.sh | sh"]})),
            vec![("pipe-to-shell".into(), RiskLevel::Critical)]
        );
        assert_eq!(
            rules(json!({"command": "/bin/sh", "args": ["-c", "node server.js"]})),
            vec![("inline-shell".into(), RiskLevel::Medium)]
        );
        assert!(rules(json!({"command": "bash", "args": ["./start.sh"]})).is_empty());
    }

    #[test]
    fn container_escapes_are_flagged() {
        let found = rules(json!({"command": "docker", "args": [
            "run", "-i", "--rm", "--privileged", "--network=host",
            "-v", "/var/run/docker.sock:/var/run/docker.sock",
            "--mount", "type=bind,source=/,target=/host",
            "-e", "TOKEN", "ghcr.io/acme/mcp:1.2.0"
        ]}));
        let ids: Vec<&str> = found.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "privileged-container",
                "host-namespace",
                "docker-socket",
                "host-root-mount"
            ]
        );
    }

    #[test]
    fn container_image_pinning() {
        let unpinned = |image: &str| {
            rules(json!({"command": "docker", "args": ["run", "-i", "--rm", "-e", "X", image]}))
                .iter()
                .any(|(id, _)| id == "unpinned-image")
        };
        assert!(unpinned("ghcr.io/acme/mcp"));
        assert!(unpinned("acme/mcp:latest"));
        assert!(unpinned("localhost:5000/acme/mcp"));
        assert!(!unpinned("acme/mcp:1.2.0"));
        assert!(!unpinned("acme/mcp@sha256:abcd"));
    }

    #[test]
    fn package_runners_resolve_the_launched_package() {
        let pkg = |def| launched_package(&spec(def));
        assert_eq!(
            pkg(
                json!({"command": "npx", "args": ["-y", "@modelcontextprotocol/server-github@2025.4.8"]})
            ),
            Some(Package {
                ecosystem: Ecosystem::Npm,
                name: "@modelcontextprotocol/server-github".into(),
                version: Some("2025.4.8".into()),
            })
        );
        assert_eq!(
            pkg(json!({"command": "npx", "args": ["-y", "server-x@latest"]})).map(|p| p.version),
            Some(None)
        );
        assert_eq!(
            pkg(json!({"command": "pnpm", "args": ["dlx", "--package=tool@1.0.0", "tool"]}))
                .map(|p| (p.name, p.version)),
            Some(("tool".into(), Some("1.0.0".into())))
        );
        assert_eq!(
            pkg(
                json!({"command": "uvx", "args": ["--from", "mcp-server-git==0.6.2", "mcp-server-git"]})
            ),
            Some(Package {
                ecosystem: Ecosystem::PyPI,
                name: "mcp-server-git".into(),
                version: Some("0.6.2".into()),
            })
        );
        assert_eq!(
            pkg(json!({"command": "uvx", "args": ["mcp-server-fetch[all]>=1.0"]}))
                .map(|p| (p.name, p.version)),
            Some(("mcp-server-fetch".into(), None))
        );
        assert_eq!(
            pkg(json!({"command": "npx", "args": ["./local/server.js"]})),
            None
        );
        assert_eq!(
            pkg(json!({"command": "npx", "args": ["github:acme/server"]})),
            None
        );
        assert_eq!(pkg(json!({"command": "node", "args": ["server.js"]})), None);
    }

    #[test]
    fn unpinned_package_is_low_so_the_default_gate_passes() {
        assert_eq!(
            rules(json!({"command": "npx.cmd", "args": ["-y", "server-x"]})),
            vec![("unpinned-package".into(), RiskLevel::Low)]
        );
        assert!(rules(json!({"command": "npx", "args": ["-y", "server-x@1.0.0"]})).is_empty());
    }
}
