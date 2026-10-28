/**
 * The GitHub API calls the site makes, in one place so every caller sends the
 * same headers and shares one cache entry per URL.
 *
 * Unauthenticated GitHub allows 60 requests an hour per IP and every request
 * from the site leaves one Vercel IP, so each helper revalidates on a timer
 * rather than forwarding per request, and returns null on any failure so the
 * caller can degrade instead of throwing.
 */

const API = "https://api.github.com/repos/pivoshenko/kasetto";

async function fetchJson<T>(path: string, revalidate: number): Promise<T | null> {
  try {
    const res = await fetch(`${API}${path}`, {
      headers: {
        Accept: "application/vnd.github+json",
        // GitHub rejects API requests without a User-Agent
        "User-Agent": "kasetto.dev",
      },
      next: { revalidate },
    });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as T;
  } catch (error) {
    console.error(`[github] failed to fetch ${path}`, error);
    return null;
  }
}

/** Latest release tag, such as `v3.11.0`, or null when GitHub cannot be reached. */
export async function getLatestRelease(revalidate = 3600): Promise<string | null> {
  const release = await fetchJson<{ tag_name?: string }>("/releases/latest", revalidate);
  return release?.tag_name ?? null;
}

/** Star count formatted for display (`212`, `1.3K`), or null when GitHub cannot be reached. */
export async function getStars(revalidate = 3600): Promise<string | null> {
  const repo = await fetchJson<{ stargazers_count: number }>("", revalidate);
  if (!repo) {
    return null;
  }
  const n = repo.stargazers_count;
  return n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n);
}
