# Issue #117 — stop URL specificity audit

Date: 2026-08-09  
Decision: keep `STP_034` and `STP_035` as low-priority Quality rules.

## Semantics

MobilityData's `same_stop_and_agency_url` and `same_stop_and_route_url` signals
identify a `stop_url` that repeats an agency or route URL. GTFS describes
`stop_url` as a page with information about the stop; it also says that it
should be different from the agency and route URLs. This is useful guidance,
but it is not a publish-blocking Spec constraint, so the Analyzer keeps these
findings in `Quality` and lowers their severity to `Düşük`/`Low`.

## Corpus evidence

The issue's 1000-feed snapshot reported 214
`same_stop_and_agency_url` notices and 296
`same_stop_and_route_url` notices. The checked-in local parity corpus is a
smaller/currently filtered snapshot: its aggregate has 349 route-URL notices
across two feeds (348 in `mdb-2875`, 1 in `mdb-3185`) and no agency-URL row.
`mdb-2875` is a useful stress case: 348 MobilityData rows collapse to 58
Analyzer stop-level findings before this change because one stop can match
several route records. That confirms the need to treat the unit of reporting
explicitly instead of equating raw notice counts.

The samples were reviewed as follows:

| Feed | Signal | Observation | Classification |
|---|---|---|---|
| `mdb-2875` | same stop / route URL | many stops use the operator's generic `/routes/` page across multiple routes | actionable low-quality reuse; aggregate by normalized URL and stop |
| `mdb-3185` | same stop / route URL | one stop URL is the route's specific page, but is still not stop-specific | actionable low-quality reuse |
| issue snapshot | same stop / agency URL | reported by the 1000-feed audit; no matching row is present in the checked-in filtered parity extract | retain coverage and validate with synthetic fixtures |

No finding is promoted to Spec solely because MobilityData emits it. A generic
or intentionally shared URL may be a product decision, so consumers can
dismiss the low-priority Quality signal.

## Comparison contract

The implementation uses a conservative HTTP(S) URL identity:

- scheme and host are case-insensitive;
- an empty root path and `/` are equivalent;
- explicit default ports (`http:80`, `https:443`) are equivalent to omission;
- query strings, fragments, `/path` versus `/path/`, credentials, non-default
  ports, and percent-encoding remain significant;
- invalid/non-HTTP(S) values are skipped here and remain the responsibility of
  the URL-format rule `STP_042`;
- all stops sharing one normalized URL produce one finding with
  `stop_count`, up to five `representative_stop_ids`, and `normalized_url`.

This deliberately avoids treating redirects, DNS aliases, or application-level
canonical URLs as equal. Unit tests cover exact equality, normalization,
query/path/encoding distinctions, and aggregate reporting. The existing
emit-proof fixtures continue to cover exact positive `STP_034` and `STP_035`
rows.

## References

- GTFS Schedule Reference, `stops.stop_url`: <https://gtfs.org/documentation/schedule/reference/#stopstxt>
- GTFS Schedule Best Practices: <https://gtfs.org/documentation/schedule/schedule-best-practices/>
- MobilityData rule index: <https://gtfs-validator.mobilitydata.org/rules.html>
