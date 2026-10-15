import { after } from "next/server";
import { type HitKind, recordHit } from "@/lib/stats";

const RAW_BASE = "https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts";

/**
 * Proxy an install script and count the hit.
 *
 * The count is recorded in `after()` so it never delays the script reaching
 * the shell that is waiting to pipe it.
 */
export async function serveInstallScript(
  request: Request,
  script: string,
  kind: HitKind
): Promise<Response> {
  const res = await fetch(`${RAW_BASE}/${script}`);
  after(() => recordHit(kind, request));
  return new Response(res.body, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
