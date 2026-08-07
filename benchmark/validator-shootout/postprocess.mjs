import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUT = path.join(HERE, 'results');
const sourcePath = path.join(OUT, 'shootout-100x3.json');
const result = JSON.parse(fs.readFileSync(sourcePath, 'utf8'));

function systemErrorsHaveNotices(value) {
  if (!value) return false;
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value !== 'object') return Boolean(value);
  if (Array.isArray(value.notices)) return value.notices.length > 0;
  return Object.values(value).some(systemErrorsHaveNotices);
}

function statusCounts(engineKey) {
  const statuses = ['DETECTED', 'DETECTED_OTHER', 'MISSED', 'CRASH', 'ERROR', 'INTERNAL_ERROR'];
  return Object.fromEntries(statuses.map(s => [s, result.cases.filter(r => r[engineKey] === s).length]));
}

function groupStats(engineKey) {
  const byGroup = new Map();
  for (const row of result.cases) {
    if (!byGroup.has(row.group)) byGroup.set(row.group, []);
    byGroup.get(row.group).push(row[engineKey]);
  }
  const detail = {};
  for (const [group, ss] of [...byGroup.entries()].sort()) {
    const detected = ss.filter(s => s === 'DETECTED' || s === 'DETECTED_OTHER').length;
    detail[group] = {
      cases: ss.length,
      detected,
      missed: ss.filter(s => s === 'MISSED').length,
      crash: ss.filter(s => s === 'CRASH').length,
      error: ss.filter(s => s === 'ERROR' || s === 'INTERNAL_ERROR').length,
      allDetected: detected === ss.length,
    };
  }
  return {
    groups: Object.keys(detail).length,
    fullyDetected: Object.values(detail).filter(x => x.allDetected).length,
    detail,
  };
}

function severityRank(s) {
  const x = String(s ?? '').toUpperCase();
  if (x === 'ERROR' || x === 'CRITICAL') return 4;
  if (x === 'WARNING' || x === 'HIGH') return 3;
  if (x === 'INFO' || x === 'MEDIUM') return 2;
  if (x === 'IGNORE' || x === 'LOW') return 1;
  return 0;
}

function strongestSeverity(findings) {
  let best = null;
  for (const f of findings ?? []) {
    if (severityRank(f.severity) > severityRank(best)) best = String(f.severity ?? '').toUpperCase();
  }
  return best;
}

// Correct two pilot-era expected-rule labels. STP_004 is parse failure; numeric
// out-of-range latitude is STP_003 in the current Analyzer rule documentation.
for (const id of ['stop_lat_91', 'stop_lat_minus91']) {
  const row = result.cases.find(r => r.id === id);
  if (!row) continue;
  row.expectedAnalyzerRule = 'STP_003';
  row.expectedRuleDetected = row.analyzerNewFindings.some(f => f.code === 'STP_003');
  row.expectedRuleInBaseline = false;
  row.analyzerStatus = row.expectedRuleDetected ? 'DETECTED' : row.analyzerStatus;
  row.postprocessNote = 'expected Analyzer rule corrected from STP_004 to STP_003 (numeric range violation)';
}

// A typed fatal validation result is a positive recognition, not a miss/crash.
for (const row of result.cases) {
  if (row.analyzerStatus !== 'MISSED') continue;
  const rawPath = path.join(OUT, 'analyzer', `${row.id}.json`);
  if (!fs.existsSync(rawPath)) continue;
  const raw = JSON.parse(fs.readFileSync(rawPath, 'utf8'));
  if (raw?.status === 'fatal' && typeof raw?.code === 'string') {
    row.analyzerStatus = 'DETECTED';
    row.analyzerFatal = { code: raw.code, message: raw.message ?? null };
    row.postprocessNote = `${row.postprocessNote ? row.postprocessNote + '; ' : ''}typed fatal validation result counted as DETECTED`;
  }
}

// The runner initially treated {"notices":[]} in system_errors.json as an error
// merely because the object had a key. Reclassify from actual notice content.
for (const row of result.cases) {
  if (row.mobilityDataStatus !== 'INTERNAL_ERROR') continue;
  const runtimeFinding = (row.mobilityDataNewFindings ?? []).some(f => /^runtime_exception_/i.test(f.code ?? ''));
  const realSystemError = systemErrorsHaveNotices(row.mobilityDataSystemErrors);
  if (!runtimeFinding && !realSystemError) {
    row.mobilityDataStatus = (row.mobilityDataNewFindings ?? []).length > 0 ? 'DETECTED' : 'MISSED';
  }
}

result.totals = {
  analyzer: statusCounts('analyzerStatus'),
  tml: statusCounts('tmlStatus'),
  mobilitydata: statusCounts('mobilityDataStatus'),
};
result.groupStats = {
  analyzer: groupStats('analyzerStatus'),
  tml: groupStats('tmlStatus'),
  mobilitydata: groupStats('mobilityDataStatus'),
};

result.severityBreakdown = {
  tml: {},
  mobilitydata: {},
};
for (const row of result.cases) {
  if (row.tmlStatus === 'DETECTED') {
    const s = strongestSeverity(row.tmlNewFindings) ?? 'UNKNOWN';
    result.severityBreakdown.tml[s] = (result.severityBreakdown.tml[s] ?? 0) + 1;
  }
  if (row.mobilityDataStatus === 'DETECTED' || row.mobilityDataStatus === 'INTERNAL_ERROR') {
    const s = strongestSeverity(row.mobilityDataNewFindings) ?? 'UNKNOWN';
    result.severityBreakdown.mobilitydata[s] = (result.severityBreakdown.mobilitydata[s] ?? 0) + 1;
  }
}

result.postprocessing = {
  applied: true,
  notes: [
    'MobilityData system_errors.json with an empty notices array is not an internal error.',
    'Analyzer typed fatal validation responses are counted as detected violations.',
    'stop_lat ±91 fixtures map to STP_003 (range) rather than STP_004 (parse).',
    'MobilityData raw report.json bytes are preserved when NaN/Infinity normalization is needed for strict JSON parsing.',
  ],
};

const correctedJson = path.join(OUT, 'shootout-100x3.corrected.json');
fs.writeFileSync(correctedJson, JSON.stringify(result, null, 2) + '\n');

const a = result.totals.analyzer;
const t = result.totals.tml;
const m = result.totals.mobilitydata;
const lines = [
  '# GTFS Validator Shootout — corrected 100×3 summary',
  '',
  `Base checkout: \`${result.baseCommit ?? 'unknown'}\`. Analyzer date: \`${result.versions.analyzerToday}\`.`,
  `TML: \`${result.versions.tml}\`. MobilityData: \`${result.versions.mobilitydata}\`.`,
  '',
  `**100 selected single-mutant fixtures across ${result.methodology.semanticGroups} semantic groups.**`,
  '',
  '| Engine | Detected | Missed | Crash | Internal/other error | Fully detected groups |',
  '|---|---:|---:|---:|---:|---:|',
  `| GTFS Analyzer | ${a.DETECTED + a.DETECTED_OTHER} | ${a.MISSED} | ${a.CRASH} | ${a.ERROR + a.INTERNAL_ERROR} | ${result.groupStats.analyzer.fullyDetected}/${result.groupStats.analyzer.groups} |`,
  `| TML | ${t.DETECTED + t.DETECTED_OTHER} | ${t.MISSED} | ${t.CRASH} | ${t.ERROR + t.INTERNAL_ERROR} | ${result.groupStats.tml.fullyDetected}/${result.groupStats.tml.groups} |`,
  `| MobilityData | ${m.DETECTED + m.DETECTED_OTHER} | ${m.MISSED} | ${m.CRASH} | ${m.ERROR + m.INTERNAL_ERROR} | ${result.groupStats.mobilitydata.fullyDetected}/${result.groupStats.mobilitydata.groups} |`,
  '',
  `TML structured-detection strongest severity: ${JSON.stringify(result.severityBreakdown.tml)}.`,
  `MobilityData structured-detection strongest severity: ${JSON.stringify(result.severityBreakdown.mobilitydata)}.`,
  '',
  '## Non-common outcomes',
  '',
  '| Case | Group | Analyzer | TML | MobilityData |',
  '|---|---|:---:|:---:|:---:|',
];
for (const row of result.cases) {
  if (row.analyzerStatus === 'DETECTED' && row.tmlStatus === 'DETECTED' && row.mobilityDataStatus === 'DETECTED') continue;
  lines.push(`| ${row.id} | ${row.group} | ${row.analyzerStatus} | ${row.tmlStatus} | ${row.mobilityDataStatus} |`);
}
lines.push(
  '',
  '> These figures are results for this selected fixture suite, not complete validator-wide recall percentages. Repeated boundary variants are grouped into semantic groups to avoid overstating breadth.',
  '',
  '> MobilityData v8.0.1 emitted bare NaN/Infinity-style values in JSON for applicable floating-point fixtures. Exact raw reports are retained as report.raw.json; normalized report.json exists only for strict parser compatibility.',
);

fs.writeFileSync(path.join(OUT, 'report-100x3.corrected.md'), lines.join('\n') + '\n');
console.log(lines.slice(0, 30).join('\n'));
