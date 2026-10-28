"use client";

import { useEffect, useState } from "react";
import { SECRET_PROVIDERS as SOURCES } from "./landing/secret-providers";

const CYCLE_MS = 2600;

export function SecretSources() {
  const [active, setActive] = useState(0);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const t = setInterval(() => setActive((i) => (i + 1) % SOURCES.length), CYCLE_MS);
    return () => clearInterval(t);
  }, []);

  const src = SOURCES[active] ?? SOURCES[0];
  const Icon = src.icon;
  const placeholder = `\${${src.usage}}`;

  return (
    <figure className="lp-pane lp-inject" aria-label="How a secret placeholder is resolved">
      <figcaption className="lp-pane-bar">
        <span>Secrets injection</span>
      </figcaption>

      <div className="lp-inject-body">
        <div className="lp-inject-file">
          <div className="lp-inject-path">
            <span>mcps/vercel.json</span>
            <span className="lp-inject-flag">in git</span>
          </div>
          <pre className="lp-inject-code">
            <span className="sy-punct">{'"env": { '}</span>
            <span className="sy-key">"VERCEL_TOKEN"</span>
            <span className="sy-punct">: </span>
            <span key={src.usage} className="lp-inject-ph">{`"${placeholder}"`}</span>
            <span className="sy-punct">{" }"}</span>
          </pre>
        </div>

        <div className="lp-inject-resolve" key={active}>
          <span className="lp-inject-wire" aria-hidden />
          <span className="lp-inject-via">
            <Icon aria-hidden />
            <span className="lp-inject-via-text">
              <b>{src.name}</b>
              {src.via !== "env" && src.via !== "file" ? (
                <>
                  {" "}
                  via <code>{src.via}</code>
                </>
              ) : null}
            </span>
          </span>
        </div>

        <div className="lp-inject-out">
          <div className="lp-inject-file" data-tone="secret">
            <div className="lp-inject-path">
              <span>~/.claude.json</span>
              <span className="lp-inject-flag">local only</span>
            </div>
            <pre className="lp-inject-code">
              <span className="sy-key">"VERCEL_TOKEN"</span>
              <span className="sy-punct">: </span>
              <span className="lp-inject-value" key={active}>
                "vcp_7Qx<span className="lp-inject-mask">••••••••••</span>"
              </span>
            </pre>
          </div>
          <div className="lp-inject-file" data-tone="lock">
            <div className="lp-inject-path">
              <span>kasetto.lock</span>
              <span className="lp-inject-flag">in git</span>
            </div>
            <pre className="lp-inject-code">
              <span className="sy-key">hash</span>
              <span className="sy-punct">: </span>
              <span className="sy-str">9b2e41e0...</span>
              <span className="lp-inject-note"># of the placeholder</span>
            </pre>
          </div>
        </div>
      </div>
    </figure>
  );
}
