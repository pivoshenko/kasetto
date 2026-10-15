import { after } from "next/server";
import { lastKnownTag, recordHit, rememberTag } from "@/lib/stats";

const GITHUB_LATEST = "https://api.github.com/repos/pivoshenko/kasetto/releases/latest";

/**
 * Latest release tag, for the CLI's daily update check.
 *
 * Every running copy of kasetto hits this once per 24h, which makes it the
 * active-use counter. The GitHub call is revalidated every 10 minutes rather
 * than forwarded per request: unauthenticated GitHub allows 60 requests/hour
 * per IP, and every caller arriving through this route shares one Vercel IP.
 * On a GitHub failure the last known tag is served, so a cold cache during a
 * GitHub outage degrades to a stale answer instead of breaking the check for
 * every installed copy at once.
 */
export async function GET(request: Request) {
  // Only the CLI's own checks count as active use; scanners and curious humans
  // reach this route too and would otherwise inflate the metric
  if (request.headers.get("user-agent")?.startsWith("kasetto/")) {
    after(() => recordHit("ping", request));
  }

  const release = await fetchLatest();
  if (!release) {
    const fallback = await lastKnownTag();
    if (!fallback) {
      return Response.json({ error: "latest release unavailable" }, { status: 502 });
    }
    return Response.json({ tag_name: fallback, stale: true });
  }

  after(() => rememberTag(release));
  return Response.json({ tag_name: release });
}

/** Latest tag from GitHub, or null on any failure so the caller can serve stale. */
async function fetchLatest(): Promise<string | null> {
  try {
    const res = await fetch(GITHUB_LATEST, {
      headers: {
        Accept: "application/vnd.github+json",
        // GitHub rejects API requests without a User-Agent
        "User-Agent": "kasetto.dev",
      },
      next: { revalidate: 600 },
    });
    if (!res.ok) {
      return null;
    }
    const body = (await res.json()) as { tag_name?: string };
    return body.tag_name ?? null;
  } catch (error) {
    console.error("[latest] failed to reach GitHub", error);
    return null;
  }
}
