import { readStats } from "@/lib/stats";

export const dynamic = "force-dynamic";

/**
 * Install and usage counters as JSON.
 *
 * `install` and `install-ps1` count script downloads, so they see only the
 * `curl`/`irm` path and not Homebrew, Scoop or `cargo install`. `ping` counts
 * the CLI's daily update check, so it sees every install channel, and its
 * `uniqueLast7Days` is the closest thing to a weekly active-install figure.
 *
 * Ungated on purpose: these are vanity counters, not private data.
 */
export async function GET() {
  const stats = await readStats();
  if (!stats) {
    return Response.json({ error: "no stats store configured" }, { status: 503 });
  }
  return Response.json(stats);
}
