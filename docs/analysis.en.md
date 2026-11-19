# Analysis Criteria, Scores and Notice Limits

🇬🇧 **English** · [Back to README](../README.en.md)

## Analysis Thresholds

Validation thresholds can be customized from the **Analysis Thresholds** section on the upload screen. Changed values take effect on the next ZIP upload; the reset button restores defaults.

### Rule Classes and Authority Source

Every rule falls into one of four classes. The class reflects the finding's **authority source** (its basis of legitimacy), so a user can tell at a glance whether a finding is a real GTFS Spec violation or an interoperability/quality/analytics signal:

- **Spec** — only cases that the official **GTFS Schedule Reference** explicitly requires, forbids, or renders invalid (required / conditionally required / conditionally forbidden fields, enum values, foreign keys, uniqueness, format constraints). No other source produces `Spec`.
- **Interop** — compatibility signals with consumer/validator behavior such as MobilityData, Google Transit, or a regional profile (e.g., GTFS-JP).
- **Quality** — GTFS best-practice, data quality, readability, consistency, and production-quality checks.
- **Analytics** — statistical, operational, performance, or analysis-oriented signals.

Each rule also carries a machine-readable **authority source** (`authority_source`) field (`GTFS_SPEC`, `MOBILITYDATA_PARITY`, `REGIONAL_PROFILE`, `PROJECT_QUALITY`, etc.). Invariant: **the `Spec` class is legitimate only with `authority_source = GTFS_SPEC`**; parity with MobilityData/Guru/Google, best-practice, or project-specific heuristics is not on its own proof of Spec.

### Optional Profiles and Source URL

Setting `stop_name_best_practices=true` in the config delta enables the language-dependent `STP_040` and `STP_041` checks; they are disabled by default because of their false-positive risk. URL-based integrations may provide `source_url` metadata, allowing `ARC_028` to verify that the permanent publishing URL contains a `.zip` filename. Upload-only validation skips this check. The core engine never requests URLs found inside a feed; HTTP availability checks require a separate, explicitly opt-in online adapter.

### Coordinating shape distance fields

If a trip uses `shape_dist_traveled` in `stop_times.txt` but some points of its referenced shape lack the same field in `shapes.txt`, the analyzer emits `SHP_030` (Quality · Medium). Both fields are individually optional in GTFS, so this is not a Spec publish blocker; it is a shape-level compatibility signal that consumers may be unable to place stops reliably on the shape. The finding includes the affected-trip count and representative trip IDs.

A one-point shape that is actually referenced by a trip is reported as `SHP_006` at Low · Quality, with `shape_id` and `shape_point_count=1` in the details. A two-point straight segment is valid. An unused one-point shape is reported only by `SHP_018`.

### Stop URL specificity

`STP_034` and `STP_035` compare `stop_url` with agency and route URLs using a conservative syntactic identity and report low-priority Quality findings. Scheme/host case, the root `/`, and explicit HTTP 80/HTTPS 443 default ports are equivalent; query strings, fragments, path trailing slashes, and percent-encoding remain significant. Stops sharing one normalized URL are reported in a single aggregate finding with the affected-stop count and representative IDs in `details`.

### Thresholds

Some rules work with adjustable thresholds: the maximum speed per route type (bus 120 km/h, tram 100, metro 150, rail 300, ferry 80, cable car 30), transfer time and distance, shape jumps, stops that are too close (5 m), the distance of a stop to its shape and to its parent station, service gaps, trip duration, headway and the feed expiry warning.

Thresholds are changed in the web app's **Analysis Criteria** panel, with `--config config.json` in the CLI, and with the `config` object in the SDK and the Python package. Every key, default and unit: [configuration reference](../sdk/README.md#configuration-reference).

---

## Scores

### Publish Score (0–100)

Measures whether the feed is publishable per the official GTFS Schedule Reference. The score **starts at 100**; each publication-blocking issue deducts a penalty proportional to the rule's weight and remediation cost.

**How the score is computed:**
- Only `Spec` class issues at `Critical` severity (the official GTFS spec gate) affect the Publish Score. `Interop` compatibility signals are reported separately (Interop Score / R8).
- If the same rule fires multiple times, the penalty is capped at **2×**; a single issue cannot drive the score to zero.
- **0–40:** Feed is likely unusable. Blocker errors present.
- **40–70:** Partial issues; some applications may reject the feed.
- **70–90:** Usable, but attention is needed.
- **90–100:** Ready to publish.

### Overall Score (0–100)

Weighted average of all four analysis classes: Spec×40% + Interop×30% + Quality×20% + Analytics×10%. Reflects both specification compliance and operational data quality. A feed can be publishable while still having a low Overall Score.

**How the score is computed:**
- Issues from all four classes (Spec, Interop, Quality, Analytics) affect this score according to their weights.
- Missing optional fields, inconsistent service patterns, and accessibility gaps are reflected here via the Quality and Analytics components.
- **0–60:** Significant quality issues; passenger experience may be affected.
- **60–80:** Moderate quality; improvements recommended.
- **80–100:** Good data quality.

> **Note:** The Publish Score and the Overall Score serve different purposes and are computed with different formulas. A feed with a high Publish Score but a low Overall Score works technically, but issues such as missing accessibility information or incorrect route names affect passengers.

---

## Notice Limits

In large feeds the same rule can fire thousands of times. Unlimited notice lists strain browser memory and reduce readability. A two-tier cap is applied:

| Limit | Value | Scope |
|---|---|---|
| Per rule (default) | 500 | All rules |
| Per rule (high) | 2,000 | `TRP_020`, `OPR_007`, `STP_016`, `STP_017` |
| Total (all rules) | — | No feed-wide 100,000 stop limit |

Rules in the high-cap list naturally produce large counts in real feeds (e.g., one headway record per trip). When a rule hits its cap, the actual violation count appears in the **Total** column of the Fix Queue; on the All Findings tab, selecting that rule filter shows a yellow warning banner.
