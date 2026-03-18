use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "kasetto")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Sync {
        #[arg(long, default_value = "skills.config.yaml")]
        config: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        quiet: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        plain: bool,
        #[arg(long)]
        verbose: bool,
    },
    InstallHooks {
        #[arg(long, default_value = "skills.config.yaml")]
        config: String,
        #[arg(long, default_value_t = 10)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 300)]
        cache_ttl_seconds: u64,
    },
}

#[derive(Debug, Deserialize)]
struct Config {
    destination: String,
    skills: Vec<SourceSpec>,
}

#[derive(Debug, Deserialize)]
struct SourceSpec {
    source: String,
    branch: Option<String>,
    skills: SkillsField,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SkillsField {
    Wildcard(String),
    List(Vec<SkillTarget>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SkillTarget {
    Name(String),
    Obj { name: String, path: Option<String> },
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
struct SkillEntry {
    destination: String,
    hash: String,
    skill: String,
    source: String,
    source_revision: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct State {
    version: u8,
    last_run: Option<String>,
    skills: BTreeMap<String, SkillEntry>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            last_run: None,
            skills: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Default)]
struct Summary {
    installed: usize,
    updated: usize,
    removed: usize,
    unchanged: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
struct Action {
    source: Option<String>,
    skill: Option<String>,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    run_id: String,
    config: String,
    destination: String,
    dry_run: bool,
    summary: Summary,
    actions: Vec<Action>,
}

const BANNER: &str = r#"
╔════════════════════════════════════════════════════════════╗
║  ██╗  ██╗ █████╗ ███████╗███████╗████████╗████████╗ ██████╗ ║
║  ██║ ██╔╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝╚══██╔══╝██╔═══██╗║
║  █████╔╝ ███████║███████╗█████╗     ██║      ██║   ██║   ██║║
║  ██╔═██╗ ██╔══██║╚════██║██╔══╝     ██║      ██║   ██║   ██║║
║  ██║  ██╗██║  ██║███████║███████╗   ██║      ██║   ╚██████╔╝║
║  ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝      ╚═╝    ╚═════╝ ║
║                        カセット  |  kasetto                   ║
╚════════════════════════════════════════════════════════════╝
"#;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Sync {
        config: "skills.config.yaml".into(),
        dry_run: false,
        quiet: false,
        json: false,
        plain: false,
        verbose: false,
    }) {
        Commands::Sync {
            config,
            dry_run,
            quiet,
            json,
            plain,
            verbose,
        } => run_sync(&config, dry_run, quiet, json, plain, verbose),
        Commands::InstallHooks {
            config,
            timeout_seconds,
            cache_ttl_seconds,
        } => install_hooks(&config, timeout_seconds, cache_ttl_seconds),
    }
}

fn load_config_any(config_path: &str) -> Result<(Config, PathBuf, String)> {
    if config_path.starts_with("http://") || config_path.starts_with("https://") {
        let text = reqwest::blocking::get(config_path)
            .with_context(|| format!("failed to fetch remote config: {config_path}"))?
            .error_for_status()
            .with_context(|| format!("remote config returned non-success status: {config_path}"))?
            .text()?;
        let cfg: Config = serde_yaml::from_str(&text)?;
        let cfg_dir = std::env::current_dir().context("failed to get current directory")?;
        return Ok((cfg, cfg_dir, config_path.to_string()));
    }

    let cfg_abs = fs::canonicalize(config_path)
        .with_context(|| format!("config not found: {config_path}"))?;
    let cfg_text = fs::read_to_string(&cfg_abs)?;
    let cfg: Config = serde_yaml::from_str(&cfg_text)?;
    let cfg_dir = cfg_abs
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("invalid config path"))?;
    Ok((cfg, cfg_dir, cfg_abs.to_string_lossy().to_string()))
}

fn run_sync(
    config_path: &str,
    dry_run: bool,
    quiet: bool,
    as_json: bool,
    plain: bool,
    verbose: bool,
) -> Result<()> {
    let animate = animations_enabled(quiet, as_json, plain);
    if !quiet && !as_json {
        if plain {
            println!("kasetto | カセット");
        } else {
            print!("{}", BANNER);
        }
    }

    let (cfg, cfg_dir, cfg_label) = with_spinner(animate, plain, "Loading config", || {
        load_config_any(config_path)
    })?;
    let destination = resolve_path(&cfg_dir, &cfg.destination);
    if !dry_run {
        with_spinner(animate, plain, "Preparing destination", || {
            fs::create_dir_all(&destination)?;
            Ok(())
        })?;
    }

    let mut state = load_state()?;
    let mut desired_keys = HashSet::new();
    let mut summary = Summary::default();
    let mut actions = Vec::new();

    for (i, src) in cfg.skills.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-{}-{}", now_unix(), i));
        let source_step = format!("Syncing source {}", src.source);
        match with_spinner(animate, plain, &source_step, || {
            materialize_source(src, &cfg_dir, &stage)
        }) {
            Ok((root, rev, available)) => {
                let targets = select_targets(&src.skills, &available)?;
                for (skill_name, skill_path) in targets {
                    let key = format!("{}::{}", src.source, skill_name);
                    desired_keys.insert(key.clone());
                    let hash_step = format!("Hashing {}", skill_name);
                    let hash = with_spinner(animate, plain, &hash_step, || hash_dir(&skill_path))?;
                    let dest = destination.join(&skill_name);
                    if let Some(prev) = state.skills.get(&key) {
                        if prev.hash == hash && dest.exists() {
                            summary.unchanged += 1;
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(skill_name),
                                status: "unchanged".into(),
                                error: None,
                            });
                            continue;
                        }
                    }

                    if dry_run {
                        let status = if state.skills.contains_key(&key) {
                            "would_update"
                        } else {
                            "would_install"
                        };
                        if status == "would_install" {
                            summary.installed += 1
                        } else {
                            summary.updated += 1
                        }
                        actions.push(Action {
                            source: Some(src.source.clone()),
                            skill: Some(skill_name),
                            status: status.into(),
                            error: None,
                        });
                        continue;
                    }

                    let copy_step = format!("Applying {}", skill_name);
                    with_spinner(animate, plain, &copy_step, || copy_dir(&skill_path, &dest))?;
                    let status = if state.skills.contains_key(&key) {
                        summary.updated += 1;
                        "updated"
                    } else {
                        summary.installed += 1;
                        "installed"
                    };
                    state.skills.insert(
                        key,
                        SkillEntry {
                            destination: dest.to_string_lossy().to_string(),
                            hash,
                            skill: skill_name.clone(),
                            source: src.source.clone(),
                            source_revision: rev.clone(),
                            updated_at: now_iso(),
                        },
                    );
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(skill_name),
                        status: status.into(),
                        error: None,
                    });
                }
                let _ = fs::remove_dir_all(root);
            }
            Err(e) => {
                summary.failed += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: None,
                    status: "source_error".into(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let existing_keys: Vec<String> = state.skills.keys().cloned().collect();
    for k in existing_keys {
        if desired_keys.contains(&k) {
            continue;
        }
        if let Some(entry) = state.skills.get(&k).cloned() {
            if dry_run {
                summary.removed += 1;
                actions.push(Action {
                    source: Some(entry.source),
                    skill: Some(entry.skill),
                    status: "would_remove".into(),
                    error: None,
                });
            } else {
                let _ = fs::remove_dir_all(&entry.destination);
                state.skills.remove(&k);
                summary.removed += 1;
                actions.push(Action {
                    source: Some(entry.source),
                    skill: Some(entry.skill),
                    status: "removed".into(),
                    error: None,
                });
            }
        }
    }

    if !dry_run {
        state.last_run = Some(now_iso());
        with_spinner(animate, plain, "Saving state", || save_state(&state))?;
    }

    let report = Report {
        run_id: format!("{}", now_unix()),
        config: cfg_label,
        destination: destination.to_string_lossy().to_string(),
        dry_run,
        summary,
        actions,
    };
    let report_path = with_spinner(animate, plain, "Writing report", || save_report(&report))?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if !quiet {
        println!(
            "Summary: installed={} updated={} removed={} unchanged={} failed={}",
            report.summary.installed,
            report.summary.updated,
            report.summary.removed,
            report.summary.unchanged,
            report.summary.failed
        );
        println!("Report: {}", report_path.display());

        if verbose {
            println!("\nActions:");
            for a in &report.actions {
                let status = status_chip(&a.status, plain);
                let src = a.source.as_deref().unwrap_or("-");
                let skill = a.skill.as_deref().unwrap_or("-");
                if let Some(err) = &a.error {
                    println!("  {} {} :: {} -> {}", status, src, skill, err);
                } else {
                    println!("  {} {} :: {}", status, src, skill);
                }
            }
        }
    }

    if report.summary.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn animations_enabled(quiet: bool, as_json: bool, plain: bool) -> bool {
    !quiet && !as_json && !plain && std::io::stderr().is_terminal()
}

fn with_spinner<T, F>(
    enabled: bool,
    plain: bool,
    label: impl Into<String>,
    operation: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let label = label.into();
    if !enabled {
        return operation();
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let thread_label = label.clone();
    let handle = thread::spawn(move || {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 0usize;
        let mut stderr = std::io::stderr();
        while !stop_flag.load(Ordering::Relaxed) {
            let _ = write!(
                stderr,
                "\r\x1b[2K{}\x1b[90m {}\x1b[0m",
                frames[idx % frames.len()],
                thread_label
            );
            let _ = stderr.flush();
            idx = idx.wrapping_add(1);
            thread::sleep(Duration::from_millis(80));
        }
    });

    let result = operation();
    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();

    let mut stderr = std::io::stderr();
    let symbol = if result.is_ok() { "✓" } else { "✗" };
    if plain {
        let _ = writeln!(stderr, "{} {}", symbol, label);
    } else if result.is_ok() {
        let _ = writeln!(
            stderr,
            "\r\x1b[2K\x1b[32m{}\x1b[0m\x1b[90m {}\x1b[0m",
            symbol, label
        );
    } else {
        let _ = writeln!(
            stderr,
            "\r\x1b[2K\x1b[31m{}\x1b[0m\x1b[90m {}\x1b[0m",
            symbol, label
        );
    }
    let _ = stderr.flush();

    result
}

fn status_chip(status: &str, plain: bool) -> String {
    if plain {
        return format!("[{}]", status.to_uppercase());
    }
    match status {
        "installed" | "updated" | "removed" => format!("\x1b[30;42m {} \x1b[0m", status),
        "unchanged" => format!("\x1b[30;47m {} \x1b[0m", status),
        "would_install" | "would_update" | "would_remove" => {
            format!("\x1b[30;43m {} \x1b[0m", status)
        }
        _ => format!("\x1b[30;41m {} \x1b[0m", status),
    }
}

fn install_hooks(config_path: &str, timeout: u64, ttl: u64) -> Result<()> {
    print!("{}", BANNER);
    let (_cfg, _cfg_dir, cfg_label) = load_config_any(config_path)?;
    let home = dirs_home()?;
    let runner_dir = home.join(".kasetto/hooks");
    fs::create_dir_all(&runner_dir)?;
    let runner = runner_dir.join("session-start.sh");

    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
CONFIG="{}"
LOCK_FILE="${{HOME}}/.kasetto/hooks/sync.lock"
STAMP_FILE="${{HOME}}/.kasetto/hooks/last_sync_unix"
TIMEOUT={}
TTL={}
mkdir -p "${{HOME}}/.kasetto/hooks"
if [[ -f "$STAMP_FILE" ]]; then
  last=$(cat "$STAMP_FILE" || echo 0)
  now=$(date +%s)
  if (( now - last < TTL )); then exit 0; fi
fi
exec 9>"$LOCK_FILE"
if ! flock -n 9; then exit 0; fi
if timeout "$TIMEOUT" kasetto sync --config "$CONFIG" --quiet; then date +%s > "$STAMP_FILE"; fi
"#,
        cfg_label, timeout, ttl
    );

    fs::write(&runner, script)?;
    set_exec(&runner)?;

    for p in [
        home.join(".claude/hooks/session-start.sh"),
        home.join(".cursor/hooks/session-start.sh"),
    ] {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &p,
            format!("#!/usr/bin/env bash\nexec \"{}\"\n", runner.display()),
        )?;
        set_exec(&p)?;
        println!("- {}", p.display());
    }
    println!("- {}", runner.display());
    Ok(())
}

fn materialize_source(
    src: &SourceSpec,
    cfg_dir: &Path,
    stage: &Path,
) -> Result<(PathBuf, String, HashMap<String, PathBuf>)> {
    if src.source.contains("://") {
        let (owner, repo) = parse_github(&src.source)?;
        let branch = src.branch.clone().unwrap_or_else(|| "main".into());
        let url = format!("https://codeload.github.com/{owner}/{repo}/tar.gz/refs/heads/{branch}");
        download_extract(&url, stage).or_else(|_| {
            if src.branch.is_none() {
                let url2 =
                    format!("https://codeload.github.com/{owner}/{repo}/tar.gz/refs/heads/master");
                download_extract(&url2, stage)
            } else {
                Err(anyhow!("failed to download source"))
            }
        })?;
        let available = discover(stage)?;
        Ok((stage.to_path_buf(), format!("branch:{branch}"), available))
    } else {
        let root = resolve_path(cfg_dir, &src.source);
        let available = discover(&root)?;
        Ok((root, "local".into(), available))
    }
}

fn parse_github(url: &str) -> Result<(String, String)> {
    let p = url.trim_end_matches('/').trim_end_matches(".git");
    let parts: Vec<_> = p.split('/').collect();
    if parts.len() < 2 {
        return Err(anyhow!("unsupported github url"));
    }
    Ok((
        parts[parts.len() - 2].to_string(),
        parts[parts.len() - 1].to_string(),
    ))
}

fn discover(root: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut out = HashMap::new();
    for base in [root.to_path_buf(), root.join("skills")] {
        if !base.exists() {
            continue;
        }
        for e in fs::read_dir(base)? {
            let e = e?;
            if !e.file_type()?.is_dir() {
                continue;
            }
            let d = e.path();
            if d.join("SKILL.md").exists() {
                out.insert(e.file_name().to_string_lossy().to_string(), d);
            }
        }
    }
    Ok(out)
}

fn select_targets(
    sf: &SkillsField,
    available: &HashMap<String, PathBuf>,
) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    match sf {
        SkillsField::Wildcard(s) if s == "*" => {
            for (k, v) in available {
                out.push((k.clone(), v.clone()));
            }
        }
        SkillsField::List(items) => {
            for it in items {
                match it {
                    SkillTarget::Name(name) => {
                        let p = available
                            .get(name)
                            .ok_or_else(|| anyhow!("skill not found: {name}"))?;
                        out.push((name.clone(), p.clone()));
                    }
                    SkillTarget::Obj { name, path } => {
                        if let Some(path) = path {
                            let d = PathBuf::from(path).join(name);
                            if d.join("SKILL.md").exists() {
                                out.push((name.clone(), d));
                                continue;
                            }
                        }
                        let p = available
                            .get(name)
                            .ok_or_else(|| anyhow!("skill not found: {name}"))?;
                        out.push((name.clone(), p.clone()));
                    }
                }
            }
        }
        _ => return Err(anyhow!("invalid skills field")),
    }
    Ok(out)
}

fn hash_dir(path: &Path) -> Result<String> {
    let mut files = Vec::new();
    for e in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if e.file_type().is_file() {
            files.push(e.path().to_path_buf());
        }
    }
    files.sort();
    let mut hasher = Sha256::new();
    for f in files {
        let rel = f.strip_prefix(path).unwrap().to_string_lossy();
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let mut file = fs::File::open(&f)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        hasher.update(&buf);
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    for e in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel = e.path().strip_prefix(src)?;
        let target = dst.join(rel);
        if e.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(p) = target.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(e.path(), &target)?;
        }
    }
    Ok(())
}

fn download_extract(url: &str, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    let body = reqwest::blocking::get(url)?.bytes()?;
    let gz = flate2::read::GzDecoder::new(body.as_ref());
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let p = entry.path()?;
        let parts: Vec<_> = p.components().collect();
        if parts.len() < 2 {
            continue;
        }
        let rel = parts
            .iter()
            .skip(1)
            .map(|c| c.as_os_str())
            .collect::<PathBuf>();
        if rel.to_string_lossy().contains("..") {
            return Err(anyhow!("unsafe archive path"));
        }
        let target = dst.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(target)?;
    }
    Ok(())
}

fn resolve_path(base: &Path, raw: &str) -> PathBuf {
    let p = PathBuf::from(
        raw.replace(
            '~',
            &dirs_home()
                .unwrap_or_else(|_| PathBuf::from("~"))
                .to_string_lossy(),
        ),
    );
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

fn dirs_home() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| anyhow!("HOME is not set"))
}

fn set_exec(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(path)?.permissions();
        p.set_mode(0o755);
        fs::set_permissions(path, p)?;
    }
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn now_iso() -> String {
    chrono_like_now()
}

fn chrono_like_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}

fn state_path() -> Result<PathBuf> {
    Ok(dirs_home()?.join(".ai/bootstrap/state.json"))
}

fn load_state() -> Result<State> {
    let p = state_path()?;
    if !p.exists() {
        return Ok(State::default());
    }
    let b = fs::read_to_string(p)?;
    Ok(serde_json::from_str(&b).unwrap_or_default())
}

fn save_state(state: &State) -> Result<()> {
    let p = state_path()?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(p, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

fn save_report(report: &Report) -> Result<PathBuf> {
    let home = dirs_home()?;
    let run_dir = home.join(format!(".ai/bootstrap/runs/run-{}", report.run_id));
    fs::create_dir_all(&run_dir)?;
    let path = run_dir.join("report.json");
    fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}
