# gtfs-sdk

`gtfs-sdk` exposes the GTFS Analyzer Rust/WebAssembly validation engine to Browser
and Node.js applications as a public TypeScript SDK.

```sh
npm install gtfs-sdk
```

## One-shot validation

```ts
import { validateGtfs } from 'gtfs-sdk';

const result = await validateGtfs(zipBytes, { today: '2026-08-20' });
console.log(result.reports.r5.score);
```

## Sessions: progress callbacks and cache-backed reruns

A session keeps the parsed feed in the WASM cache, so `rerun()` re-runs only K6–K7
without reparsing the ZIP.

Note the shape difference: `validateGtfs()` resolves to a `ValidationResult`, while
session methods resolve to a `SessionResult` that **wraps** it as `.result` alongside
the ZIP listing and per-file stats.

```ts
import { createValidatorSession } from 'gtfs-sdk';

const session = await createValidatorSession({ today: '2026-08-20' });

const firstRun = await session.validate(zipBytes, {
  callbacks: { onStageDone: (stage, elapsedMs) => console.log(stage, elapsedMs) },
});
console.log(firstRun.result.reports.r5.score); // note: .result
console.log(firstRun.files, firstRun.fileStats, firstRun.engineMode);

const rerun = await session.rerun({ config: { /* config delta */ } });
console.log(rerun.result.reports.r5.score);

session.dispose();
```

Call `dispose()` when you are done; the session owns the WASM cache until then, and
any call after disposal throws.

## Errors

Fatal engine failures and invalid input throw `ValidationError`, which carries a
`code` alongside the message.

```ts
import { ValidationError } from 'gtfs-sdk';

try {
  await validateGtfs(zipBytes);
} catch (error) {
  if (error instanceof ValidationError) console.error(error.code, error.message);
}
```

## Versioning

`getVersion()` returns both the SDK version and the validator engine version it
embeds:

```ts
import { getVersion } from 'gtfs-sdk';

getVersion(); // { sdk: '0.1.1', engine: '0.9.7' }
```

## Notes

The generated `gtfs-wasm` binding is an internal implementation detail and is not
part of the public API. The bundled SDK engine is serial by default; the Analyzer UI
supplies its selected threaded or memory64 engine through the adapter contract
without exposing those bindings as public API.

## License

MIT
