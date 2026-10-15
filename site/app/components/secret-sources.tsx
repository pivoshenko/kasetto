import type { IconType } from "react-icons";
import { FaApple, FaAws, FaFileShield, FaMicrosoft, FaTerminal } from "react-icons/fa6";
import {
  Si1Password,
  SiGnuprivacyguard,
  SiGooglecloud,
  SiKeepassxc,
  SiVault,
} from "react-icons/si";

// `usage` is the placeholder body (without the `${...}` wrapper, added in JSX so
// the strings stay plain, not template literals)
const SOURCES: { icon: IconType; name: string; usage: string }[] = [
  { icon: FaTerminal, name: "Environment", usage: "kst_VERCEL_TOKEN" },
  { icon: FaFileShield, name: "credentials.yaml", usage: "kst:crd:vercel/token" },
  { icon: Si1Password, name: "1Password", usage: "kst:op:Private/Vercel/token" },
  { icon: SiVault, name: "HashiCorp Vault", usage: "kst:vault:secret/app#token" },
  { icon: SiKeepassxc, name: "KeePass", usage: "kst:kp:Vercel/API#Password" },
  { icon: FaAws, name: "AWS Secrets Manager", usage: "kst:aws:prod/db#password" },
  { icon: SiGooglecloud, name: "Google Secret Manager", usage: "kst:gcp:db-password" },
  { icon: FaMicrosoft, name: "Azure Key Vault", usage: "kst:az:my-vault/db" },
  { icon: SiGnuprivacyguard, name: "pass / gopass", usage: "kst:pass:work/vercel" },
  { icon: FaApple, name: "macOS Keychain", usage: "kst:keychain:vercel-token" },
];

export function SecretSources() {
  return (
    <figure className="lp-pane">
      <figcaption className="lp-pane-bar">
        <span>secret sources</span>
        <span className="lp-pane-tag">{SOURCES.length} backends</span>
      </figcaption>
      <ul className="lp-secrets">
        {SOURCES.map((s) => {
          const Icon = s.icon;
          return (
            <li key={s.name} className="lp-secret">
              <Icon className="lp-secret-icon" aria-hidden />
              <span className="lp-secret-name">{s.name}</span>
              <code className="lp-secret-usage">{`\${${s.usage}}`}</code>
            </li>
          );
        })}
      </ul>
    </figure>
  );
}
