# Security Policy

## Security model

GTFS Analyzer is **fully client-side**. Uploaded GTFS `.zip` files are processed entirely
in the user's browser via WebAssembly and are **never transmitted to any server**. The
deployed site (GitHub Pages) serves only static assets and has no backend, database, or
user data store.

Because of this design, the most relevant security concerns are:

- Safe handling of untrusted feed content in the browser (e.g., HTML/script injection via
  feed-supplied values such as stop names or file names).
- Integrity of the build/deploy pipeline (the live site is built from source on deploy).

## Supported versions

The project is pre-1.0 and rolls forward: security fixes land on the latest release
(`main`) only. There are no maintenance branches for older versions — if you are on an
earlier release, upgrade to the latest.

| Version         | Supported |
|-----------------|-----------|
| 0.7.x (latest)  | ✅        |
| < 0.7           | ❌        |

## Reporting a vulnerability

Please report security issues **privately** — do not open a public issue for an
unfixed vulnerability.

1. **Preferred:** Use GitHub's private vulnerability reporting on this repository
   (**Security → Report a vulnerability**). This keeps the report confidential until a
   fix is available.
2. **Alternative:** Email **ttezer@gmail.com** with details.

Please include:
- A description of the issue and its impact.
- Steps to reproduce (a minimal GTFS feed or payload if applicable).
- Affected version / commit.

We aim to acknowledge reports within a few days. Once a fix is ready, we will coordinate
disclosure and credit reporters who wish to be named.
