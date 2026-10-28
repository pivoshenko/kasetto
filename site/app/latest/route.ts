import { after } from "next/server";
import { getLatestRelease } from "@/lib/github";
import { lastKnownTag, recordHit, rememberTag } from "@/lib/stats";

/**
 * Latest release tag, for the CLI's daily update check.
 *
 * Every running copy of kasetto hits this once per 24h, which makes it the
 * active-use counter. The GitHub call is revalidated every 10 minutes rather
 * than forwarded per request, and on a GitHub failure the last known tag is
 * served, so a cold cache during a GitHub outage degrades to a stale answer
 * instead of breaking the check for every installed copy at once.
 */
export async function GET(request: Request) {
  // Only the CLI's own checks count as active use; scanners and curious humans
  // reach this route too and would otherwise inflate the metric
  if (request.headers.get("user-agent")?.startsWith("kasetto/")) {
    after(() => recordHit("ping", request));
  }

  const release = await getLatestRelease(600);
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
