import { FaGithub, FaGlobe, FaLinkedinIn } from "react-icons/fa";
import { GoArrowRight, GoArrowUpRight } from "react-icons/go";
import { softwareApplicationJsonLd } from "@/lib/structured-data";
import { AGENTS, AgentsGrid } from "./components/agents-grid";
import { BrandLogo } from "./components/brand-mark";
import { CopyButton } from "./components/copy-button";
import { ConfigExample } from "./components/feature-tabs";
import { HeroTerminal } from "./components/hero-terminal";
import { JsonLd } from "./components/json-ld";
import { AgentIcon } from "./components/landing/agent-icon";
import { AssetKinds } from "./components/landing/asset-kinds";
import { AuditGate, AuditPolicy } from "./components/landing/audit-gate";
import { ForgeWall } from "./components/landing/forge-wall";
import { HeroCassette } from "./components/landing/hero-cassette";
import { LockDiff, LockExample } from "./components/landing/lock-example";
import { VaultList } from "./components/landing/secret-providers";
import { SecretSources } from "./components/secret-sources";

const REPO = "https://github.com/pivoshenko/kasetto";
const CURL = "curl -fsSL kasetto.dev/install | sh";
const PLACEHOLDER = `\${kst_...}`;

const INSTALL = [
  { label: "macOS / Linux", cmd: CURL },
  {
    label: "Windows (PowerShell)",
    cmd: 'powershell -ExecutionPolicy Bypass -c "irm kasetto.dev/install.ps1 | iex"',
  },
  {
    label: "Homebrew",
    cmd: "brew tap pivoshenko/tap\nbrew trust pivoshenko/tap\nbrew install kasetto",
  },
  { label: "Cargo", cmd: "cargo install kasetto" },
];

const JUMP = [
  { href: "#declare", label: "Config" },
  { href: "#sources", label: "Assets and sources" },
  { href: "#secrets", label: "Secrets" },
  { href: "#lock", label: "Lock file" },
  { href: "#audit", label: "Audit" },
  { href: "#agents", label: "Agents" },
  { href: "#install", label: "Install" },
];

const FOOTER = [
  {
    title: "Docs",
    links: [
      { label: "Quick start", href: "/docs" },
      { label: "Installation", href: "/docs/installation" },
      { label: "Configuration", href: "/docs/configuration" },
      { label: "Commands", href: "/docs/commands" },
      { label: "Cookbook", href: "/docs/cookbook" },
    ],
  },
  {
    title: "Project",
    links: [
      { label: "Repository", href: REPO },
      { label: "Releases", href: `${REPO}/releases` },
      { label: "Issues", href: `${REPO}/issues` },
      { label: "Contributing", href: `${REPO}/blob/main/CONTRIBUTING.md` },
    ],
  },
  {
    title: "More",
    links: [
      { label: "Supported agents", href: "/docs/agents" },
      { label: "FAQ", href: "/docs/faq" },
      { label: "Security", href: "/docs/security" },
      { label: "CI & automation", href: "/docs/ci" },
    ],
  },
];

async function getLatestVersion(): Promise<string | null> {
  try {
    const res = await fetch("https://api.github.com/repos/pivoshenko/kasetto/releases/latest", {
      next: { revalidate: 3600 },
    });
    const release = res.ok ? ((await res.json()) as { tag_name: string }) : null;
    return release?.tag_name ?? null;
  } catch {
    return null;
  }
}

function External({
  href,
  className,
  label,
  children,
}: {
  href: string;
  className?: string;
  label?: string;
  children: React.ReactNode;
}) {
  return (
    <a
      href={href}
      className={className}
      target="_blank"
      rel="noopener noreferrer"
      aria-label={label}
    >
      {children}
    </a>
  );
}

function Feature({
  id,
  title,
  body,
  link,
  flip,
  split,
  aside,
  children,
}: {
  id: string;
  title: React.ReactNode;
  body: React.ReactNode;
  link: { href: string; label: string };
  flip?: boolean;
  split?: "5-7" | "7-5";
  aside?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section
      id={id}
      className="lp-feature"
      data-flip={flip ? "" : undefined}
      data-split={split}
      aria-labelledby={`${id}-title`}
    >
      <h2 id={`${id}-title`} className="lp-h2 lp-feature-title display">
        {title}
      </h2>
      <div className="lp-feature-intro">
        <p className="lp-lede">{body}</p>
        <a href={link.href} className="lp-textlink">
          {link.label} <GoArrowRight aria-hidden />
        </a>
      </div>
      <div className="lp-feature-art">{children}</div>
      {aside ? <div className="lp-feature-aside">{aside}</div> : null}
    </section>
  );
}

export default async function Page() {
  const version = await getLatestVersion();

  return (
    <div className="lp">
      <JsonLd data={softwareApplicationJsonLd()} />

      {/* == Hero == */}
      <header className="lp-hero">
        <div className="lp-hero-copy">
          <h1 className="lp-h1 display">
            <span className="lp-h1-line">Your AI environment,</span>{" "}
            <span className="lp-h1-line">
              <em>under control</em>
            </span>
          </h1>
          <p className="lp-lede lp-hero-lede">
            Declarative AI agent environment manager, written in Rust.
          </p>
          <div className="lp-actions">
            <a href="/docs" className="lp-btn lp-btn--primary">
              Read the docs <GoArrowRight aria-hidden />
            </a>
            <External href={REPO} className="lp-btn">
              <FaGithub aria-hidden /> Star on GitHub
            </External>
          </div>
          <div className="lp-cmd">
            <span className="lp-cmd-prompt" aria-hidden="true">
              $
            </span>
            <code className="lp-cmd-text">{CURL}</code>
            <CopyButton text={CURL} label="Copy" />
          </div>
        </div>
        <div className="lp-hero-art">
          <HeroCassette className="lp-cassette" />
        </div>
      </header>

      {/* == Agent Strip == */}
      <section className="lp-strip" aria-label="Supported agents">
        <div className="lp-marquee">
          {/* Two identical lists side by side; the track slides by exactly one list width, so the loop has no seam */}
          <div className="lp-marquee-track">
            {[false, true].map((copy) => (
              <ul key={String(copy)} className="lp-strip-list" aria-hidden={copy || undefined}>
                {AGENTS.map((a) => (
                  <li key={a.value}>
                    <AgentIcon preset={a.value} name={a.name} className="lp-strip-icon" />
                    <span className="lp-strip-name">{a.name}</span>
                  </li>
                ))}
              </ul>
            ))}
          </div>
        </div>
        <a href="#agents" className="lp-strip-more">
          See all <GoArrowRight aria-hidden />
        </a>
      </section>

      {/* == Section Jump Links == */}
      <nav className="lp-jump" aria-label="Sections">
        <ul>
          {JUMP.map((j) => (
            <li key={j.href}>
              <a href={j.href}>{j.label}</a>
            </li>
          ))}
        </ul>
      </nav>

      {/* == Declare == */}
      <Feature
        id="declare"
        title={
          <>
            See everything in <em>one file</em>
          </>
        }
        body={
          <>
            <code>kasetto.yaml</code> is the full inventory of your AI environment: every skill,
            slash command, MCP, and instruction, and the repository each one comes from.{" "}
            <code>kst sync</code> makes any machine match it: fetch each source, write every asset
            where the agent expects it, change only what differs.
          </>
        }
        link={{ href: "/docs/configuration", label: "Configuration reference" }}
        aside={<HeroTerminal />}
      >
        <ConfigExample />
      </Feature>

      {/* == Assets and Sources == */}
      <section id="sources" className="lp-section" aria-labelledby="sources-title">
        <div className="lp-section-head">
          <h2 id="sources-title" className="lp-h2 display">
            From any forge <em>to every agent</em>
          </h2>
        </div>
        <AssetKinds />
        <ForgeWall />
      </section>

      {/* == Secrets == */}
      <Feature
        id="secrets"
        title={
          <>
            Tokens <em>never touch git</em>
          </>
        }
        body={
          <>
            Write a <code>{PLACEHOLDER}</code> placeholder instead of a token. Kasetto resolves it
            at sync time from your environment, a credentials file, or the vault you already use,
            through the CLI session you are signed in to, and writes the value only to the
            agent&apos;s config. Never to the lock.
          </>
        }
        link={{ href: "/docs/secrets", label: "Secrets guide" }}
        aside={<VaultList />}
      >
        <SecretSources />
      </Feature>

      {/* == Lock == */}
      <Feature
        id="lock"
        split="7-5"
        title={
          <>
            Know exactly <em>what is installed</em>
          </>
        }
        body={
          <>
            Every sync records source, resolved revision, destination, and content hash in{" "}
            <code>kasetto.lock</code>. Commit it, and every change to your AI environment shows up
            in code review. <code>--locked</code> never touches the network and fails if the lock
            cannot satisfy the config.
          </>
        }
        link={{ href: "/docs/how-sync-works", label: "How sync works" }}
        aside={<LockDiff />}
      >
        <LockExample />
      </Feature>

      {/* == Audit == */}
      <Feature
        id="audit"
        split="7-5"
        title={
          <>
            Catch a bad install <em>before it ships</em>
          </>
        }
        body={
          <>
            <code>kst audit</code> grades every installed skill and MCP: skills.sh verdicts for
            skills, launch checks plus OSV and deps.dev for the package an MCP runs. As a gate on
            sync, anything at or above your threshold fails the run, so CI goes red instead of your
            agent picking it up.
          </>
        }
        link={{ href: "/docs/security", label: "Security model" }}
        aside={<AuditPolicy />}
      >
        <AuditGate />
      </Feature>

      {/* == Agent Wall == */}
      <section id="agents" className="lp-section" aria-labelledby="agents-title">
        <div className="lp-section-head">
          <h2 id="agents-title" className="lp-h2 display">
            Works with <em>the agent you use</em>
          </h2>
        </div>
        <AgentsGrid />
      </section>

      {/* == Install == */}
      <section id="install" className="lp-feature" aria-labelledby="install-title">
        <h2 id="install-title" className="lp-h2 lp-feature-title display">
          One binary, <em>any machine</em>
        </h2>
        <div className="lp-feature-intro">
          <p className="lp-lede">
            A single static binary for macOS, Linux, and Windows, with JSON output and real exit
            codes for CI. It installs as both <code>kasetto</code> and the short alias{" "}
            <code>kst</code>.
          </p>
          <div className="lp-install-links">
            <a href="/docs/installation" className="lp-textlink">
              Installation guide <GoArrowRight aria-hidden />
            </a>
            <External href={`${REPO}/releases`} className="lp-textlink">
              {version ? `Release ${version}` : "Releases"} <GoArrowUpRight aria-hidden />
            </External>
          </div>
        </div>
        <ul className="lp-install-list lp-feature-art">
          {INSTALL.map((m) => (
            <li key={m.label} className="lp-install-row">
              <span className="lp-install-label">{m.label}</span>
              <pre className="lp-install-cmd">
                <code>{m.cmd}</code>
              </pre>
              <CopyButton text={m.cmd} label="Copy" />
            </li>
          ))}
        </ul>
      </section>

      {/* == Footer == */}
      <footer className="lp-footer">
        <div className="lp-footer-grid">
          <div className="lp-footer-brand">
            <a href="/" className="lp-footer-name display" aria-label="Kasetto home">
              <BrandLogo className="brand-logo" /> kasetto
            </a>
            <div className="lp-footer-social">
              <External href="https://pivoshenko.dev" label="Author's blog">
                <FaGlobe aria-hidden />
              </External>
              <External href={REPO} label="GitHub">
                <FaGithub aria-hidden />
              </External>
              <External href="https://linkedin.com/in/pivoshenko" label="LinkedIn">
                <FaLinkedinIn aria-hidden />
              </External>
            </div>
            <a
              href="https://backlinklog.com/listing/kasetto.dev?utm_source=backlinklog&utm_medium=badge"
              className="lp-footer-badge"
            >
              <img
                src="https://backlinklog.com/badge/kasetto.dev.svg"
                alt="Listed on BacklinkLog"
                width={160}
                height={40}
              />
            </a>
          </div>

          {FOOTER.map((col) => (
            <nav key={col.title} className="lp-footer-col" aria-label={col.title}>
              <h3 className="lp-footer-title">{col.title}</h3>
              <ul>
                {col.links.map((l) => (
                  <li key={l.label}>
                    {l.href.startsWith("http") ? (
                      <External href={l.href}>{l.label}</External>
                    ) : (
                      <a href={l.href}>{l.label}</a>
                    )}
                  </li>
                ))}
              </ul>
            </nav>
          ))}
        </div>

        <div className="lp-footer-bottom">
          <span>
            2026 Volodymyr Pivoshenko{" "}
            <a href="mailto:contact@pivoshenko.dev">&lt;contact@pivoshenko.dev&gt;</a>
          </span>
          <span>MIT - Apache 2.0</span>
        </div>
      </footer>
    </div>
  );
}
