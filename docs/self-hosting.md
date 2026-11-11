# Self-hosting the web app

The web app is a static site: HTML, JavaScript, WebAssembly and images. It needs no server-side
code, database or runtime, so any web server that can serve files and set response headers can
host it. The public instance runs on GitHub Pages.

## 1. Build

Building needs the Rust and Node toolchains described in the README (the "Build" section).

```bash
cd ui
npm ci
npm run wasm:all   # serial, threaded and wasm64 engines
npm run build      # type check + production build
```

The site is written to `ui/dist/`. Copy that folder's contents to the web root, or to any
sub-path: all asset references are relative, so `https://example.org/` and
`https://example.org/tools/gtfs/` both work without rebuilding.

## 2. Response headers

### Cross-origin isolation (multi-threaded engine)

The threaded engine uses `SharedArrayBuffer`, which browsers enable only on a cross-origin
isolated page. Send these two headers on every response:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Without them the app still works. `coi-serviceworker.js` (loaded by `index.html`) adds the
headers from a service worker; that is how the GitHub Pages instance gets isolation, since Pages
cannot send custom headers. It reloads the page once on the first visit. When the server already
sends the headers, the service worker does nothing, so it can stay in place.

To check: open the site, then run `crossOriginIsolated` in the browser console. It should print
`true`.

### Content-Security-Policy

The production build already carries a CSP in a `<meta>` tag in `index.html`. It allows no
inline scripts and no inline event handlers; the report tabs' two small print scripts are
allowed by hash.

You can additionally send the same policy as a header. That lets you add `frame-ancestors`,
which browsers ignore in a `<meta>` tag:

```text
Content-Security-Policy: <the policy from dist/index.html>; frame-ancestors 'none'
```

Copy the policy from the `<meta http-equiv="Content-Security-Policy">` tag of **your own build**
(turn `&#39;` back into `'`). Do not reuse a policy from another build or from this document:
the two `sha256-…` values change whenever the report print scripts change, and a stale value
breaks printing. When both the header and the `<meta>` tag are present, the browser enforces
both, so the header must allow everything the `<meta>` allows.

### Other headers

```text
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
Strict-Transport-Security: max-age=31536000   # HTTPS only
```

## 3. MIME types and caching

- Serve `.wasm` as `application/wasm` and `.js` as `text/javascript`. Most servers already do;
  older configurations may need the `.wasm` entry added.
- Files under `assets/` have content hashes in their names: cache them for a long time
  (`Cache-Control: public, max-age=31536000, immutable`).
- Serve `index.html`, `coi-serviceworker.js` and `gtag-init.js` with `Cache-Control: no-cache`
  so a new release is picked up on the next visit.
- Turn off server-side scripting (PHP, CGI, SSI) for the site's folder; nothing in it needs it.

## 4. Example: nginx

nginx does not inherit `add_header` lines into a `location` block that sets headers of its own,
so keep the shared headers in a snippet and include it everywhere:

```nginx
# /etc/nginx/snippets/gtfs-analyzer-headers.conf
add_header Cross-Origin-Opener-Policy   "same-origin" always;
add_header Cross-Origin-Embedder-Policy "require-corp" always;
# Paste the policy from your build's index.html, then append frame-ancestors.
add_header Content-Security-Policy      "<policy from dist/index.html>; frame-ancestors 'none'" always;
add_header X-Content-Type-Options       "nosniff" always;
add_header Referrer-Policy              "strict-origin-when-cross-origin" always;
add_header Strict-Transport-Security    "max-age=31536000" always;
```

```nginx
server {
    listen 443 ssl;
    server_name example.org;
    root /var/www/gtfs-analyzer;

    location / {
        include snippets/gtfs-analyzer-headers.conf;
    }
    location /assets/ {
        include snippets/gtfs-analyzer-headers.conf;
        add_header Cache-Control "public, max-age=31536000, immutable" always;
    }
    location ~ ^/(index\.html|coi-serviceworker\.js|gtag-init\.js)?$ {
        include snippets/gtfs-analyzer-headers.conf;
        add_header Cache-Control "no-cache" always;
    }
}
```

Check that nginx's `mime.types` maps `wasm` to `application/wasm` (recent versions do). Do not
add a `types { }` block inside `server`: it replaces the whole MIME table instead of extending it.

## 5. Values tied to the upstream site

A few files refer to the upstream project's own site and accounts. Change or remove them for
your instance:

| What | Where |
|---|---|
| Canonical URL, Open Graph URL, structured-data URL | `ui/index.html` (`<link rel="canonical">`, `og:url`, the JSON-LD block) |
| Web analytics tag (upstream measurement ID) | `ui/index.html` (the `googletagmanager.com` script tag) and `ui/public/gtag-init.js`; if you remove them, also drop the Google host from `script-src` and `img-src` in `ui/vite.config.ts` |
| ChatGPT plugin legal pages (privacy, terms, support) | `ui/public/plugin/` (copied into every build) |

Edit these before building, so the `<meta>` CSP in the build matches.

## 6. Verify

1. Open the site and load a feed: the analysis completes and the report pages open.
2. In the browser console, `crossOriginIsolated` prints `true`.
3. The console shows no `Content Security Policy` errors.
4. In the planner page, open a card with a map: tiles and markers appear.
5. On the export page, open the executive report: its print button opens the print dialog.

The repository's browser test `ui/tests/csp-xss.spec.ts` automates these checks against a built
site, using a feed with script payloads in its ids and names; it fails on any executed payload or
CSP violation. It runs against `vite preview` by default (see `ui/playwright.config.ts`); to run
it against your own server, set `use.baseURL` to your site and remove the `webServer` block.
