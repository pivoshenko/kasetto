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
import { AuditGate } from "./components/landing/audit-gate";
import { ForgeWall } from "./components/landing/forge-wall";
import { HeroCassette } from "./components/landing/hero-cassette";
import { LockExample } from "./components/landing/lock-example";
import { SecretWall } from "./components/landing/secret-providers";
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
  points,
  link,
  flip,
  children,
}: {
  id: string;
  title: React.ReactNode;
  body: React.ReactNode;
  points: React.ReactNode[];
  link: { href: string; label: string };
  flip?: boolean;
  children: React.ReactNode;
}) {
  return (
    <section
      id={id}
      className="lp-feature"
      data-flip={flip ? "" : undefined}
      aria-labelledby={`${id}-title`}
    >
      <h2 id={`${id}-title`} className="lp-h2 lp-feature-title display">
        {title}
      </h2>
      <div className="lp-feature-copy">
        <p className="lp-lede">{body}</p>
        <ul className="lp-points">
          {points.map((p, i) => (
            <li key={i}>{p}</li>
          ))}
        </ul>
        <a href={link.href} className="lp-textlink">
          {link.label} <GoArrowRight aria-hidden />
        </a>
      </div>
      <div className="lp-feature-art">{children}</div>
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

      {/* == How It Works == */}
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
            slash command, MCP, and instruction, and the repo each one comes from.
          </>
        }
        points={[
          <>
            Pin a tag or commit with <code>ref:</code>, or follow a <code>branch:</code>
          </>,
          <>
            Share a team base with <code>extends:</code> and layer your own entries on top
          </>,
          <>
            <code>kst add</code> and <code>kst remove</code> edit the file and keep your comments
          </>,
        ]}
        link={{ href: "/docs/configuration", label: "Configuration reference" }}
      >
        <ConfigExample />
      </Feature>

      {/* == Asset Kinds == */}
      <section id="kinds" className="lp-section lp-section--tight" aria-labelledby="kinds-title">
        <div className="lp-section-head">
          <h2 id="kinds-title" className="lp-h2 display">
            Four kinds of asset, <em>each in native format</em>
          </h2>
        </div>
        <AssetKinds />
      </section>

      <Feature
        id="lock"
        flip
        title={
          <>
            Know exactly <em>what is installed</em>
          </>
        }
        body={
          <>
            Every sync records what it installed in <code>kasetto.lock</code>: source, resolved
            revision, destination, content hash. Commit it, and every change to your AI environment
            shows up in code review.
          </>
        }
        points={[
          <>
            <code>--locked</code> never touches the network and fails if the lock cannot satisfy the
            config
          </>,
          <>
            <code>--update</code> re-resolves moving branches, for every source or only the assets
            you name
          </>,
          <>Paths are stored relative to the project, so the lock is portable across machines</>,
        ]}
        link={{ href: "/docs/how-sync-works", label: "How sync works" }}
      >
        <LockExample />
      </Feature>

      <Feature
        id="sync"
        title={
          <>
            The same setup on <em>every machine</em>
          </>
        }
        body={
          <>
            <code>kst sync</code> makes a machine match the lock: fetch each source, write every
            asset where the agent expects it, change only what differs. A new teammate runs one
            command and has the environment everyone else already has.
          </>
        }
        points={[
          <>Content is hashed and diffed against the lock, so unchanged assets are never touched</>,
          <>Anything you drop from the config is pruned on the next sync, so nothing lingers</>,
          <>
            Shared files such as <code>CLAUDE.md</code> get managed blocks, so your own edits
            survive
          </>,
        ]}
        link={{ href: "/docs/commands", label: "Command reference" }}
      >
        <HeroTerminal />
      </Feature>

      {/* == Forge Wall == */}
      <section
        id="sources"
        className="lp-section lp-section--tight"
        aria-labelledby="sources-title"
      >
        <div className="lp-section-head">
          <h2 id="sources-title" className="lp-h2 display">
            Pull from any forge, <em>public or private</em>
          </h2>
        </div>
        <ForgeWall />
      </section>

      <Feature
        id="audit"
        flip
        title={
          <>
            Catch a bad install <em>before it ships</em>
          </>
        }
        body={
          <>
            <code>kst audit</code> grades every installed skill and MCP. Add <code>--audit</code> to
            a sync and it becomes a gate: anything at or above your threshold fails the run, so CI
            goes red instead of your agent picking it up.
          </>
        }
        points={[
          <>Skills take the verdicts skills.sh partners publish, worst one first</>,
          <>
            MCPs get offline launch checks, then OSV advisories and deps.dev health for the package
            they run
          </>,
          <>
            An asset nothing can judge is unverified, never safe: it warns, or fails with{" "}
            <code>--fail-on-unverified</code>
          </>,
        ]}
        link={{ href: "/docs/security", label: "Security model" }}
      >
        <AuditGate />
      </Feature>

      <Feature
        id="secrets"
        title={
          <>
            Tokens <em>never touch git</em>
          </>
        }
        body={
          <>
            MCPs need API keys. Write a <code>{PLACEHOLDER}</code> placeholder instead, and Kasetto
            resolves it at sync time from your environment, a credentials file, or the secret
            manager you already use.
          </>
        }
        points={[
          <>Resolved values are written only to the agent&apos;s config, never to the lock</>,
          <>Secret managers are read through the CLI session you are already signed in to</>,
          <>Private repos authenticate with tokens from env vars; Kasetto stores no credentials</>,
        ]}
        link={{ href: "/docs/secrets", label: "Secrets guide" }}
      >
        <SecretSources />
      </Feature>

      {/* == Vault Wall == */}
      <section id="vaults" className="lp-section lp-section--tight" aria-labelledby="vaults-title">
        <div className="lp-section-head">
          <h2 id="vaults-title" className="lp-h2 display">
            Any vault, <em>one placeholder</em>
          </h2>
        </div>
        <SecretWall />
      </section>

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
      <section id="install" className="lp-section lp-install" aria-labelledby="install-title">
        <h2 id="install-title" className="lp-h2 lp-feature-title display">
          One binary, <em>any machine</em>
        </h2>
        <div className="lp-install-copy">
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
        <ul className="lp-install-list">
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
