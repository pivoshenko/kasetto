import { readStats, type Stats } from "@/lib/stats";

export const dynamic = "force-dynamic";

/**
 * Install and usage counters as JSON.
 *
 * `install` and `install-ps1` count script downloads, so they see only the
 * `curl`/`irm` path and not Homebrew, Scoop or `cargo install`. `ping` counts
 * the CLI's daily update check, so it sees every install channel, and its
 * `uniqueThisWeek` is the active-install figure.
 *
 * `uniqueThisWeek` buckets by ISO week rather than a rolling 7 days, because a
 * machine only counts once per period the digest is stable across.
 *
 * Ungated on purpose: these are vanity counters, not private data.
 */
export async function GET() {
  // `readStats` propagates rather than swallowing, unlike the write helpers:
  // those ride on a request that must succeed regardless, while reporting is
  // this route's whole job, so a dead store has to be distinguishable here
  // from a store that was never configured
  let stats: Stats;
  try {
    stats = await readStats();
  } catch (error) {
    console.error("[stats] failed to read counters", error);
    return Response.json({ error: "stats store unavailable" }, { status: 503 });
  }
  if (!stats) {
    return Response.json({ error: "no stats store configured" }, { status: 503 });
  }
  return Response.json(stats);
}
