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
const UNIQUE_WINDOW_DAYS = 7;

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

function lastDays(count: number): string[] {
  const now = Date.now();
  return Array.from({ length: count }, (_, i) =>
    new Date(now - i * 86_400_000).toISOString().slice(0, 10)
  );
}

/**
 * Digest identifying a machine for one day only.
 *
 * `day` is inside the hash, so the digest rotates daily and cannot be used to
 * follow a machine over time. `cf-connecting-ip` is read first so the digest
 * stays correct if the domain is ever put behind Cloudflare's proxy, where
 * `x-forwarded-for` becomes the edge IP and would collapse every visitor in a
 * region onto one digest.
 */
async function visitorDigest(request: Request, day: string): Promise<string> {
  const ip =
    request.headers.get("cf-connecting-ip") ??
    request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() ??
    "unknown";
  const ua = request.headers.get("user-agent") ?? "unknown";
  const salt = process.env.STATS_SALT ?? "";
  const bytes = new TextEncoder().encode(`${day}:${ip}:${ua}:${salt}`);
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
    const dayKey = `kasetto:${kind}:day:${day}`;
    const uniqKey = `kasetto:${kind}:uniq:${day}`;
    await db
      .pipeline()
      .incr(`kasetto:${kind}:total`)
      .incr(dayKey)
      .expire(dayKey, RETENTION_SECS)
      .pfadd(uniqKey, await visitorDigest(request, day))
      .expire(uniqKey, RETENTION_SECS)
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
  today: number;
  uniqueLast7Days: number;
};

export type Stats = Record<HitKind, KindStats> | null;

/** Read every counter back. Returns null when no store is configured. */
export async function readStats(): Promise<Stats> {
  const db = redis();
  if (!db) {
    return null;
  }
  const kinds: HitKind[] = ["install", "install-ps1", "ping"];
  const days = lastDays(UNIQUE_WINDOW_DAYS);
  const entries = await Promise.all(
    kinds.map(async (kind) => {
      // UNIQUE_WINDOW_DAYS is a positive constant, so the tuple is never empty
      const uniqueKeys = days.map((day) => `kasetto:${kind}:uniq:${day}`) as [
        string,
        ...string[],
      ];
      const [total, todayCount, unique] = await Promise.all([
        db.get<number>(`kasetto:${kind}:total`),
        db.get<number>(`kasetto:${kind}:day:${today()}`),
        db.pfcount(...uniqueKeys),
      ]);
      return [
        kind,
        { total: total ?? 0, today: todayCount ?? 0, uniqueLast7Days: unique ?? 0 },
      ] as const;
    })
  );
  return Object.fromEntries(entries) as Record<HitKind, KindStats>;
}
