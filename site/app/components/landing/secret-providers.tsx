import type { IconType } from "react-icons";
import { FaApple, FaAws, FaFileShield, FaMicrosoft, FaTerminal } from "react-icons/fa6";
import {
  Si1Password,
  SiGnuprivacyguard,
  SiGooglecloud,
  SiKeepassxc,
  SiVault,
} from "react-icons/si";

// `usage` is the placeholder body (without the `${...}` wrapper, added in JSX so the strings
// stay plain, not template literals); `via` is the CLI whose session the lookup reuses
export const SECRET_PROVIDERS: { icon: IconType; name: string; via: string; usage: string }[] = [
  { icon: Si1Password, name: "1Password", via: "op", usage: "kst:op:Private/Vercel/token" },
  { icon: FaTerminal, name: "Environment", via: "env", usage: "kst_VERCEL_TOKEN" },
  { icon: FaFileShield, name: "credentials.yaml", via: "file", usage: "kst:crd:vercel/token" },
  { icon: SiVault, name: "HashiCorp Vault", via: "vault", usage: "kst:vault:secret/app#token" },
  { icon: SiKeepassxc, name: "KeePass", via: "keepassxc-cli", usage: "kst:kp:Vercel/API#Password" },
  { icon: FaAws, name: "AWS Secrets Manager", via: "aws", usage: "kst:aws:prod/vercel#token" },
  {
    icon: SiGooglecloud,
    name: "Google Secret Manager",
    via: "gcloud",
    usage: "kst:gcp:vercel-token",
  },
  { icon: FaMicrosoft, name: "Azure Key Vault", via: "az", usage: "kst:az:my-vault/vercel" },
  { icon: SiGnuprivacyguard, name: "pass / gopass", via: "pass", usage: "kst:pass:work/vercel" },
  { icon: FaApple, name: "macOS Keychain", via: "security", usage: "kst:keychain:vercel-token" },
];

/** Every vault as one plain list. */
export function VaultList() {
  return (
    <ul className="lp-roster">
      {SECRET_PROVIDERS.map((p) => {
        const Icon = p.icon;
        return (
          <li key={p.name}>
            <Icon aria-hidden />
            <span className="lp-roster-name">{p.name}</span>
          </li>
        );
      })}
    </ul>
  );
}
