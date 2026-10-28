import { after } from "next/server";
import { type HitKind, recordHit } from "@/lib/stats";

const RAW_BASE = "https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts";

/**
 * Proxy an install script and count the hit.
 *
 * The script is fetched through the data cache so a burst of installs does not
 * hit GitHub once each. Anything but a 200 from GitHub becomes a 502 with a
 * plain-text message: the caller is `curl ... | sh`, and an HTML error page
 * piped into a shell is worse than a clean failure.
 *
 * The count is recorded in `after()` so it never delays the script reaching
 * the shell that is waiting to pipe it.
 */
export async function serveInstallScript(
  request: Request,
  script: string,
  kind: HitKind
): Promise<Response> {
  let res: Response;
  try {
    res = await fetch(`${RAW_BASE}/${script}`, { next: { revalidate: 300 } });
  } catch (error) {
    console.error(`[install] failed to reach GitHub for ${script}`, error);
    return unavailable(script);
  }
  if (!res.ok) {
    console.error(`[install] GitHub returned ${res.status} for ${script}`);
    return unavailable(script);
  }
  after(() => recordHit(kind, request));
  return new Response(res.body, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}

function unavailable(script: string): Response {
  return new Response(
    `echo "kasetto: ${script} is unavailable right now, try again later" >&2; exit 1\n`,
    {
      status: 502,
      headers: { "Content-Type": "text/plain; charset=utf-8" },
    }
  );
}
