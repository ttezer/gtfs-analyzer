# gtfs-sdk

`gtfs-sdk`, GTFS Analyzer'ın Rust/WebAssembly validation engine'ini Browser ve Node.js uygulamalarına açan public TypeScript SDK'dır.

```sh
npm install gtfs-sdk
```

```ts
import { validateGtfs } from 'gtfs-sdk';

const result = await validateGtfs(zipBytes, { today: '2026-08-20' });
console.log(result.reports.r5.score);
```

For progress callbacks and cache-backed reruns, use a session:

```ts
import { createValidatorSession } from 'gtfs-sdk';

const session = await createValidatorSession({ today: '2026-08-20' });
const firstRun = await session.validate(zipBytes, {
  callbacks: { onStageDone: (stage) => console.log(stage) },
});
const rerun = await session.rerun({ config: { /* config delta */ } });
session.dispose();
```

The generated `gtfs-wasm` binding is an internal implementation detail and is not part of the public API. The bundled SDK engine is serial by default; the Analyzer UI supplies its selected threaded or memory64 engine through the adapter contract without exposing those bindings as public API.
