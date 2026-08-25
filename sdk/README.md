# gtfs-sdk

`gtfs-sdk` exposes the GTFS Analyzer Rust/WebAssembly validation engine to
Browser and Node.js applications as a public TypeScript SDK.

The SDK accepts a GTFS ZIP as bytes, runs the same K1–K7 validator pipeline as
the Analyzer application, and returns typed notices, reports, scores, feed
metrics, and lookup indexes.

```sh
npm install gtfs-sdk
```

The package requires Node.js 20+ in Node environments. In browsers, pass a
`Uint8Array` or `ArrayBuffer`; the SDK does not upload the feed anywhere.
Notice titles, messages, and remediation text returned by the SDK are English.

## Quick start

### Node.js

```ts
import { readFile } from 'node:fs/promises';
import { validateGtfs } from 'gtfs-sdk';

const zipBytes = await readFile('./feed.zip');
const result = await validateGtfs(zipBytes, {
  // Set this for reproducible results. If omitted, the local calendar date is used.
  today: '2026-08-20',
});

console.log('status:', result.validation_status);
console.log('score:', result.reports.r5.score);
console.log('notices:', result.notices.length);
```

### Browser

```ts
import { validateGtfs } from 'gtfs-sdk';

export async function validateUpload(file: File) {
  const zipBytes = new Uint8Array(await file.arrayBuffer());
  return validateGtfs(zipBytes, { today: '2026-08-20' });
}
```

## One-shot validation

`validateGtfs(input, options?)` initializes the bundled serial WebAssembly
engine, parses the ZIP, runs K1–K7, and resolves directly to a `ValidationResult`.

```ts
const result = await validateGtfs(zipBytes, {
  today: 20260820, // YYYYMMDD number or YYYY-MM-DD string
  config: {
    gtfs_jp_profile: 'v3',
    max_speed_bus_kmh: 110,
    min_transfer_time_sec: 240,
  },
});
```

Options:

| Option | Type | Behavior |
| --- | --- | --- |
| `today` | `number \| YYYY-MM-DD \| YYYYMMDD` | Date used by calendar and freshness checks. Defaults to the local date. |
| `config` | `ValidatorConfigDelta` | A delta over validator defaults. Unknown keys and out-of-range values are rejected. |

`initialize()` is exported for hosts that want to warm up WebAssembly manually,
but `validateGtfs()` and `createValidatorSession()` call it automatically.
Importing the SDK alone does not load the WASM module; the generated glue and
binary are loaded lazily on the first initialization or validation call.

## Understanding the result

The result has this top-level shape:

```ts
interface ValidationResult {
  validation_status?: 'COMPLETE' | 'PARTIAL';
  partial?: PartialReport;
  notices: Notice[];
  reports: ReportSet;
  metrics: FeedMetrics;
  name_index: NameIndex;
  capped_totals: Record<string, number>;
}
```

### Notices

`notices` is the detailed, user-facing finding list. A notice contains the
rule, severity, affected entity, source location, observed/expected values, and
remediation text.

```ts
const blockers = result.notices.filter((notice) =>
  notice.severity === 'CRITICAL' || notice.severity === 'HIGH',
);

for (const notice of blockers) {
  console.log({
    rule: notice.rule_id,
    severity: notice.severity,
    entity: notice.entity_id,
    file: notice.file,
    line: notice.line,
    message: notice.message,
    remediation: notice.remediation,
  });
}
```

Important notice fields:

| Field | Meaning |
| --- | --- |
| `id` | Stable notice instance identifier. |
| `rule_id` | Rule identifier such as `GEO_006` or `CAL_010`. |
| `severity` | `CRITICAL`, `HIGH`, `MEDIUM`, `LOW`, or `INFO`. |
| `rule_class` | `SPEC`, `INTEROP`, `QUALITY`, or `ANALYTICS`. |
| `entity_type` / `entity_id` | Entity category and GTFS identifier, when available. |
| `file` / `line` / `field` | Source location, when available. |
| `observed_value` / `expected_value` | Values used to explain the finding. |
| `details` | Additional string-valued rule details, when available. |
| `title` / `message` | Short and full human-readable explanations. |
| `remediation` | Suggested correction. |
| `blocks` | Reports or outcomes affected by the notice. |
| `base_effort` | Relative correction effort used by prioritization. |

### Reports

`reports` groups the same findings into consumer-oriented report views:

| Report | Contents |
| --- | --- |
| `r1` | Publishability, coverage completeness, and blocker notice IDs. |
| `r2`, `r3`, `r4` | Report item lists linking notices to report categories. |
| `r5` | Overall, publishability, specification, interoperability, quality, and analytics scores. |
| `r7`, `r8` | Additional report item lists linking notices to report categories. |
| `r9` | Prioritized rule-level improvement items, labels, score deltas, affected instances, and effort. |

The score summary is available without scanning the notice list:

```ts
const { score, pub_score, spec_score, interop_score, quality_score, analytics_score } =
  result.reports.r5;

console.log({ score, pub_score, spec_score, interop_score, quality_score, analytics_score });
```

To connect a report item back to its full explanation, use its `notice_id`:

```ts
const noticeById = new Map(result.notices.map((notice) => [notice.id, notice]));
const linkedFindings = result.reports.r2.items
  .map((item) => noticeById.get(item.notice_id))
  .filter(Boolean);
```

### Metrics and indexes

`metrics` contains feed-level counts and summary values, including stop, route,
trip, and shape counts; active service days; average daily trips; feed and service
date ranges; notice counts by rule class; overall score; and per-file statistics.
For GTFS-JP feeds, `is_gtfs_jp` reports detection and `gtfs_jp_profile` reports the
selected `auto`, `v3`, or `v4` validation scope. The profile field is not an
automatic claim about the feed's official GTFS-JP version.

`name_index` contains lookup maps for building UIs without reparsing the ZIP:

- `stops`, `routes`, `trips`: IDs to display names or headsigns
- `trip_routes`, `trip_directions`, `trip_shapes`: relationships between entities
- `trip_stops`, `shape_trips`, `route_shapes`: reverse and collection lookups
- `stop_coords`: `stop_id -> [latitude, longitude]`
- `shape_coords`: `shape_id -> [[latitude, longitude], ...]`
- `trip_first_dep`: first departure lookup by trip
- `shape_routes`: shape-to-route relationships
- `map_data_deferred`: whether large-feed geometry was deferred

For large feeds, `map_data_deferred` can be `true` and `shape_coords` can be
empty in the serialized result. When using a session, call `getShapeCoords(shapeId)`
to retrieve one geometry on demand while the cache is alive.

`partial` is present when the validator could not complete every check. Always
inspect `validation_status` and `partial` before treating a score as complete.
`capped_totals` records totals that were capped by resource or aggregation limits.

## Sessions, progress, and reruns

Use a session when you need progress callbacks, per-file information, on-demand
shape geometry, or repeated K6–K7 runs with different settings.

```ts
import { createValidatorSession } from 'gtfs-sdk';

const session = await createValidatorSession({ today: '2026-08-20' });

try {
  const firstRun = await session.validate(zipBytes, {
    callbacks: {
      onFileList: (files) => console.log('ZIP files:', files),
      onFileDone: (file) => console.log('parsed:', file.name, file.rows),
      onStageDone: (stage, elapsedMs) => console.log(stage, elapsedMs),
    },
  });

  // Session methods wrap the ValidationResult as `.result`.
  console.log(firstRun.result.reports.r5.score);
  console.log(firstRun.files, firstRun.fileStats, firstRun.engineMode);

  const shape = session.getShapeCoords('shape-1');

  // Rerun uses the prepared cache and re-runs K6–K7 without reparsing the ZIP.
  const rerun = await session.rerun({
    config: { min_transfer_time_sec: 300 },
    callbacks: { onStageDone: (stage, elapsedMs) => console.log(stage, elapsedMs) },
  });
  console.log(rerun.result.reports.r5.score, shape.length);
} finally {
  session.dispose();
}
```

The callback order is normally `onFileList`, file-level `onFileDone` callbacks,
then stage callbacks for `K1` through `K7`. A `rerun()` reports the stages that
are executed by the cached path. `dispose()` releases the cache and is safe to
call more than once; calls after disposal throw.

## Configuration reference

The `config` object is merged over the defaults. Only known keys are accepted.
The values below are the default thresholds used by the `0.9.7` validator engine.

| Key | Default | Unit / purpose |
| --- | ---: | --- |
| `source_url` | `null` | External feed URL metadata. |
| `gtfs_jp_profile` | `auto` | Explicit GTFS-JP scope: `auto`, `v3`, or `v4`; the feed version is never inferred. |
| `stop_name_best_practices` | `false` | Enable language-dependent stop-name checks. |
| `max_speed_bus_kmh` | `120` | Bus speed ceiling. |
| `max_speed_tram_kmh` | `100` | Tram speed ceiling. |
| `max_speed_metro_kmh` | `150` | Metro speed ceiling. |
| `max_speed_rail_kmh` | `300` | Rail speed ceiling. |
| `max_speed_ferry_kmh` | `80` | Ferry speed ceiling. |
| `max_speed_cablecar_kmh` | `30` | Cable car/funicular speed ceiling. |
| `min_transfer_time_sec` | `180` | Minimum transfer time. |
| `max_transfer_distance_m` | `500` | Maximum transfer distance. |
| `max_shape_jump_km` | `10` | Consecutive shape-point jump threshold. |
| `max_shape_jump_km_rail` | `30` | Rail shape-point jump threshold. |
| `stop_too_close_m` | `5` | Duplicate/too-close stop threshold. |
| `stop_far_from_shape_m` | `100` | Stop-to-shape distance threshold. |
| `stop_far_from_shape_m_rail` | `200` | Rail stop-to-shape threshold. |
| `stop_far_from_parent_m` | `150` | Stop-to-parent distance threshold. |
| `feed_expiry_warning_days` | `30` | Calendar expiry warning window. |
| `feed_info_expiry_warning_days` | `7` | `feed_info` expiry warning window. |
| `service_gap_days` | `7` | Short-service threshold. |
| `big_gap_days` | `14` | Large service gap threshold. |
| `upcoming_service_days` | `7` | Upcoming service search window. |
| `max_trip_duration_hours` | `24` | Non-rail trip duration ceiling. |
| `max_trip_duration_hours_rail` | `48` | Rail trip duration ceiling. |
| `min_trip_duration_sec` | `60` | Minimum trip duration. |
| `max_headway_warning_min` | `240` | Non-rail headway warning threshold. |
| `max_headway_warning_min_rail` | `720` | Rail headway warning threshold. |
| `service_day_window_hours_rail` | `48` | Rail service-day time window. |
| `bunching_threshold_min` | `2` | Bunching threshold. |
| `rail_stop_distance_km` | `500` | Rail stop-distance threshold. |
| `max_trips_per_route` | `500` | Route trip-count warning threshold. |
| `duration_outlier_sigma` | `3.5` | Duration outlier sensitivity. |
| `headway_outlier_sigma` | `2.5` | Headway outlier sensitivity. |
| `service_day_start_hour` | `3` | Service-day normalization start hour. |
| `max_calendar_future_years` | `3` | Maximum future calendar horizon. |
| `rural_route_ids` | `[]` | Routes exempt from automatic sparse-service warnings. |
| `calendar_override_rules` | `[]` | Explicit calendar base/override relationships. |

```ts
const result = await validateGtfs(zipBytes, {
  config: {
    max_speed_bus_kmh: 110,
    max_transfer_distance_m: 300,
    rural_route_ids: ['RURAL-1', 'RURAL-2'],
    calendar_override_rules: [{
      route_id: 'R1',
      base_service_ids: ['WEEKDAY'],
      override_service_ids: ['HOLIDAY'],
      start_date: 20261224,
      end_date: 20261231,
    }],
  },
});
```

The engine validates numeric ranges and types. A misspelled key, invalid JSON
shape, or out-of-range value throws a `ValidationError` with code `InvalidInput`.

## Errors

Fatal validation and input failures throw `ValidationError`:

```ts
import { ValidationError, validateGtfs } from 'gtfs-sdk';

try {
  await validateGtfs(zipBytes);
} catch (error) {
  if (error instanceof ValidationError) {
    console.error(error.code, error.message);
    if (error.detail) console.error('engine detail:', error.detail);
  }
}
```

`message` is a stable English summary suitable for user-facing UI. `detail` is
optional and preserves the lower-level engine diagnostic for logs and support.

Error codes:

| Code | Meaning |
| --- | --- |
| `ZipUnreadable` | Input is not a readable GTFS ZIP. |
| `Utf8Critical` | A critical text file is not valid UTF-8. |
| `NoRequiredFiles` | Required GTFS files are missing. |
| `CsvMalformed` | A CSV file cannot be parsed. |
| `DecompressionLimit` | ZIP decompression exceeded a safety limit. |
| `ResourceLimit` | A memory, size, or runtime safety limit was reached. |
| `InvalidInput` | Invalid date, config, or SDK usage. |

## TypeScript types

The package exports the result model as named types, so applications can type
their own stores and adapters without importing internal WASM bindings:

```ts
import type {
  FeedMetrics,
  Notice,
  ReportSet,
  ValidationResult,
} from 'gtfs-sdk';
```

Runnable examples are included in the package repository:

- [`examples/node.mjs`](examples/node.mjs) — Node.js file validation and JSON summary
- [`examples/browser.ts`](examples/browser.ts) — Browser `File` validation and rule filtering

## Versioning

`getVersion()` returns the SDK version and the validator engine version it embeds:

```ts
import { getVersion } from 'gtfs-sdk';

getVersion(); // { sdk: '0.1.5', engine: '0.9.7' }
```

The generated `gtfs-wasm` binding is an internal implementation detail and is not
part of the public API. The bundled SDK engine is serial by default; the Analyzer
UI supplies its selected threaded or memory64 engine through the adapter contract
without exposing those bindings as public API.

The SDK release uses Rust `opt-level=2` and keeps `wasm-opt -O3`. This is
intentionally separate from the application's and CLI's `opt-level=3` profile:
the SDK's WASM is inside the npm tarball, so the package build keeps measurable
size headroom without changing the engine code or the application's release
profile. The package-size gate is checked in CI.

## License

MIT
