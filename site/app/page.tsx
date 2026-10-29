import Link from "next/link";
import { FaGithub, FaGlobe, FaLinkedinIn } from "react-icons/fa";
import { GoArrowRight, GoArrowUpRight } from "react-icons/go";
import { getLatestRelease } from "@/lib/github";
import { REPO_URL as REPO } from "@/lib/site";
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
  { href: "#declare", label: "Config & Sync" },
  { href: "#sources", label: "Assets & Sources" },
  { href: "#secrets", label: "Secrets Management" },
  { href: "#audit", label: "Audit & Security" },
  { href: "#lock", label: "Lock & Reproducibility" },
  { href: "#agents", label: "Agents Support" },
  { href: "#install", label: "Installation" },
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
  split,
  aside,
  children,
}: {
  id: string;
  title: React.ReactNode;
  body: React.ReactNode;
  link: { href: string; label: string };
  split?: "5-7" | "7-5";
  aside?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section id={id} className="lp-feature" data-split={split} aria-labelledby={`${id}-title`}>
      <h2 id={`${id}-title`} className="lp-h2 lp-feature-title display">
        {title}
      </h2>
      <div className="lp-feature-intro">
        <p className="lp-lede">{body}</p>
        <Link href={link.href} className="lp-textlink">
          {link.label} <GoArrowRight aria-hidden />
        </Link>
      </div>
      <div className="lp-feature-art">{children}</div>
      {aside ? <div className="lp-feature-aside">{aside}</div> : null}
    </section>
  );
}

export default async function Page() {
  const version = await getLatestRelease();

  return (
    <div className="lp">
      <JsonLd data={softwareApplicationJsonLd()} />

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
            <Link href="/docs" className="lp-btn lp-btn--primary">
              Read the docs <GoArrowRight aria-hidden />
            </Link>
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

      <nav className="lp-jump" aria-label="Sections">
        <ul>
          {JUMP.map((j) => (
            <li key={j.href}>
              <a href={j.href}>{j.label}</a>
            </li>
          ))}
        </ul>
      </nav>

      <Feature
        id="declare"
        title={
          <>
            See everything in <em>one file</em>
          </>
        }
        body={
          <>
            <code>kasetto.yaml</code> lists every skill, command, MCP, and instruction you use, and
            the repository each one comes from. <code>kst sync</code> fetches those repositories and
            writes each asset where your agent looks for it. Run it again on another machine and you
            get the same setup; run it after a change and only that change is touched.
          </>
        }
        link={{ href: "/docs/configuration", label: "Configuration reference" }}
        aside={<HeroTerminal />}
      >
        <ConfigExample />
      </Feature>

      <section id="sources" className="lp-section" aria-labelledby="sources-title">
        <div className="lp-section-head">
          <h2 id="sources-title" className="lp-h2 display">
            From any source <em>to your agent</em>
          </h2>
        </div>
        <AssetKinds />
        <ForgeWall />
      </section>

      <Feature
        id="secrets"
        title={
          <>
            Tokens <em>never touch git</em>
          </>
        }
        body={
          <>
            Put a <code>{PLACEHOLDER}</code> placeholder where the token would go. When you sync,
            Kasetto reads the real value from your environment, a credentials file, or the vault CLI
            you are already signed in to, and writes it into the agent&apos;s config. The lock file
            only ever sees the placeholder.
          </>
        }
        link={{ href: "/docs/secrets", label: "Secrets guide" }}
        aside={<VaultList />}
      >
        <SecretSources />
      </Feature>

      <Feature
        id="audit"
        split="7-5"
        title={
          <>
            A security and audit layer <em>for your agent</em>
          </>
        }
        body={
          <>
            <code>kst audit</code> checks every installed skill and MCP. Skills get the verdicts
            skills.sh partners publish. MCPs get an offline look at how they launch, then OSV
            advisories and deps.dev health for the package they run. Turn it on as a sync gate and
            anything at or above your threshold fails the run, so the problem stops in CI rather
            than in your agent.
          </>
        }
        link={{ href: "/docs/security", label: "Security model" }}
        aside={<AuditPolicy />}
      >
        <AuditGate />
      </Feature>

      <Feature
        id="lock"
        split="7-5"
        title={
          <>
            Every install, <em>pinned in a lock file</em>
          </>
        }
        body={
          <>
            Each sync writes the source, resolved revision, destination, and content hash of what it
            installed to <code>kasetto.lock</code>. Commit the file and changes to your setup show
            up in code review like any other diff. <code>--locked</code> installs from the lock
            alone, without the network, and fails if the config has drifted from it.
          </>
        }
        link={{ href: "/docs/how-sync-works", label: "How sync works" }}
        aside={<LockDiff />}
      >
        <LockExample />
      </Feature>

      <section id="agents" className="lp-section" aria-labelledby="agents-title">
        <div className="lp-section-head">
          <h2 id="agents-title" className="lp-h2 display">
            Works with <em>the agent you use</em>
          </h2>
        </div>
        <AgentsGrid />
      </section>

      <section id="install" className="lp-feature" aria-labelledby="install-title">
        <h2 id="install-title" className="lp-h2 lp-feature-title display">
          One binary, <em>any machine</em>
        </h2>
        <div className="lp-feature-intro">
          <p className="lp-lede">
            One static binary for macOS, Linux, and Windows, with JSON output and exit codes you can
            build CI on. It installs as <code>kasetto</code> and as the short alias <code>kst</code>
            .
          </p>
          <div className="lp-install-links">
            <Link href="/docs/installation" className="lp-textlink">
              Installation guide <GoArrowRight aria-hidden />
            </Link>
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

      <footer className="lp-footer">
        <div className="lp-footer-grid">
          <div className="lp-footer-brand">
            <Link href="/" className="lp-footer-name display" aria-label="Kasetto home">
              <BrandLogo className="brand-logo" /> kasetto
            </Link>
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
                      <Link href={l.href}>{l.label}</Link>
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
