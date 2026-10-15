import { Redis } from "@upstash/redis";

/**
 * Install and usage counters backed by Upstash Redis.
 *
 * Counts are durable and additive: totals never expire, per-day keys expire
 * after {@link RETENTION_SECS}. Uniques are HyperLogLogs of a per-day digest,
 * so a machine counts once per day and no digest correlates across days.
 */

export type HitKind = "install" | "install-ps1" | "ping";

const RETENTION_SECS = 90 * 24 * 60 * 60;

let client: Redis | null | undefined;

// The Vercel-native Upstash integration injects KV_*, the direct one UPSTASH_*
function redis(): Redis | null {
  if (client !== undefined) {
    return client;
  }
  const url = process.env.KV_REST_API_URL ?? process.env.UPSTASH_REDIS_REST_URL;
  const token = process.env.KV_REST_API_TOKEN ?? process.env.UPSTASH_REDIS_REST_TOKEN;
  if (!url || !token) {
    console.warn("[stats] no Upstash credentials in env, counters are disabled");
    client = null;
    return client;
  }
  client = new Redis({ url, token });
  return client;
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

/** ISO-8601 week stamp, e.g. `2026-W42`. */
function isoWeek(): string {
  const now = new Date();
  const t = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()));
  const weekday = t.getUTCDay() || 7;
  t.setUTCDate(t.getUTCDate() + 4 - weekday);
  const year = t.getUTCFullYear();
  const week = Math.ceil(((t.getTime() - Date.UTC(year, 0, 1)) / 86_400_000 + 1) / 7);
  return `${year}-W${String(week).padStart(2, "0")}`;
}

/**
 * Digest identifying a machine for the lifetime of one `stamp`.
 *
 * `stamp` is inside the hash, so the digest rotates with it and cannot follow a
 * machine beyond it: a day stamp gives daily uniques, a week stamp gives weekly
 * uniques. Counting distinct machines over a period requires a digest stable
 * across that period, so the period is chosen here rather than by unioning
 * shorter buckets, which would count one machine once per bucket.
 *
 * `cf-connecting-ip` is read first so the digest stays correct if the domain is
 * ever put behind Cloudflare's proxy, where `x-forwarded-for` becomes the edge
 * IP and would collapse every visitor in a region onto one digest.
 */
async function visitorDigest(request: Request, stamp: string): Promise<string> {
  const ip =
    request.headers.get("cf-connecting-ip") ??
    request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() ??
    "unknown";
  const ua = request.headers.get("user-agent") ?? "unknown";
  const salt = process.env.STATS_SALT ?? "";
  const bytes = new TextEncoder().encode(`${stamp}:${ip}:${ua}:${salt}`);
  const hash = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(hash).slice(0, 16))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** Record one hit. Never throws: a counter failure must not fail the request it rode in on. */
export async function recordHit(kind: HitKind, request: Request): Promise<void> {
  const db = redis();
  if (!db) {
    return;
  }
  try {
    const day = today();
    const week = isoWeek();
    const dayKey = `kasetto:${kind}:day:${day}`;
    const dayUniqKey = `kasetto:${kind}:uniq:${day}`;
    const weekUniqKey = `kasetto:${kind}:uniq:week:${week}`;
    await db
      .pipeline()
      .incr(`kasetto:${kind}:total`)
      .incr(dayKey)
      .expire(dayKey, RETENTION_SECS)
      .pfadd(dayUniqKey, await visitorDigest(request, day))
      .expire(dayUniqKey, RETENTION_SECS)
      .pfadd(weekUniqKey, await visitorDigest(request, week))
      .expire(weekUniqKey, RETENTION_SECS)
      .exec();
  } catch (error) {
    console.error("[stats] failed to record hit", error);
  }
}

/** Remember the last release tag GitHub served, so /latest can fall back to it. */
export async function rememberTag(tag: string): Promise<void> {
  const db = redis();
  if (!db) {
    return;
  }
  try {
    await db.set("kasetto:latest-tag", tag);
  } catch (error) {
    console.error("[stats] failed to remember tag", error);
  }
}

export async function lastKnownTag(): Promise<string | null> {
  const db = redis();
  if (!db) {
    return null;
  }
  try {
    return await db.get<string>("kasetto:latest-tag");
  } catch (error) {
    console.error("[stats] failed to read tag", error);
    return null;
  }
}

export type KindStats = {
  total: number;
  /** Distinct machines today. */
  uniqueToday: number;
  /** Distinct machines this ISO week, not a rolling 7-day window. */
  uniqueThisWeek: number;
  hitsToday: number;
};

export type Stats = Record<HitKind, KindStats> | null;

/** Read every counter back. Returns null when no store is configured. */
export async function readStats(): Promise<Stats> {
  const db = redis();
  if (!db) {
    return null;
  }
  const kinds: HitKind[] = ["install", "install-ps1", "ping"];
  const day = today();
  const week = isoWeek();
  const entries = await Promise.all(
    kinds.map(async (kind) => {
      const [total, hitsToday, uniqueToday, uniqueThisWeek] = await Promise.all([
        db.get<number>(`kasetto:${kind}:total`),
        db.get<number>(`kasetto:${kind}:day:${day}`),
        db.pfcount(`kasetto:${kind}:uniq:${day}`),
        db.pfcount(`kasetto:${kind}:uniq:week:${week}`),
      ]);
      return [
        kind,
        {
          total: total ?? 0,
          hitsToday: hitsToday ?? 0,
          uniqueToday: uniqueToday ?? 0,
          uniqueThisWeek: uniqueThisWeek ?? 0,
        },
      ] as const;
    })
  );
  return Object.fromEntries(entries) as Record<HitKind, KindStats>;
}
