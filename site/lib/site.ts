/**
 * Canonical origin for the site.
 *
 * The apex host is canonical: it is what the README and installer scripts
 * publish (`curl -fsSL kasetto.dev/install | sh`), so `www` permanently
 * redirects here. Every absolute URL the site emits - canonical tags, the
 * sitemap, `llms.txt` links - must be built from this constant.
 */
export const SITE_URL = "https://kasetto.dev";
