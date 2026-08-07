import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { GtfsValidator, getValidatorInfo } from '@tmlmobilidade/gtfs-validator';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '../..');
const FIXTURES = path.join(HERE, 'fixtures');
const OUT = path.join(HERE, 'results');
const ANALYZER = path.join(REPO, 'target', 'release', 'gtfs-analyzer');
const MOBILITYDATA_JAR = path.join(HERE, 'mobilitydata-cli.jar');
const TODAY = '20260807';
const MOBILITYDATA_VERSION = '8.0.1';
const TML_VERSION = '20260624.1438.43';

fs.rmSync(OUT, { recursive: true, force: true });
for (const dir of ['analyzer', 'tml', 'mobilitydata']) {
  fs.mkdirSync(path.join(OUT, dir), { recursive: true });
}

const manifest = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'manifest.json'), 'utf8'));

function safeJson(value) {
  return JSON.stringify(value, (_key, v) => typeof v === 'bigint' ? v.toString() : v, 2);
}

function classifyProcessFailure(text) {
  if (/panic:|runtime error|segmentation violation|SIGSEGV|index out of range|nil pointer/i.test(text)) return 'CRASH';
  if (/Exception in thread|OutOfMemoryError|StackOverflowError|SIGABRT|core dumped/i.test(text)) return 'CRASH';
  return 'ERROR';
}

function analyzerFindings(report) {
  if (!report || report.status !== 'ok' || !Array.isArray(report.notices)) return [];
  return report.notices.map(n => ({
    code: n.rule_id ?? null,
    severity: n.severity ?? null,
    class: n.rule_class ?? null,
    file: n.file ?? null,
    field: n.field ?? null,
    line: n.line ?? null,
    message: n.message ?? null,
    raw: n,
  }));
}

function analyzerSig(n) {
  return [n.code, n.file, n.field, n.line, n.message].map(v => v ?? '').join('|');
}

function runAnalyzer(id) {
  const feed = path.join(FIXTURES, `${id}.zip`);
  const output = path.join(OUT, 'analyzer', `${id}.json`);
  const started = performance.now();
  const proc = spawnSync(ANALYZER, [
    'validate', feed,
    '--json', '--lang', 'en', '--today', TODAY,
    '--output', output,
  ], { cwd: REPO, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
  const elapsedMs = performance.now() - started;
  const report = fs.existsSync(output) ? JSON.parse(fs.readFileSync(output, 'utf8')) : null;
  const failureText = `${proc.stderr ?? ''}\n${proc.stdout ?? ''}`;
  let processStatus = 'OK';
  if (!report) processStatus = classifyProcessFailure(failureText);
  return {
    elapsedMs,
    exitCode: proc.status,
    stdout: proc.stdout,
    stderr: proc.stderr,
    processStatus,
    report,
    findings: analyzerFindings(report),
  };
}

async function runTml(id) {
  const feed = path.join(FIXTURES, `${id}.zip`);
  const output = path.join(OUT, 'tml', `${id}.report.json`);
  const started = performance.now();
  try {
    const result = await GtfsValidator(feed, {
      lang: 'en',
      timeout: 120_000,
      out_file: output,
    });
    const elapsedMs = performance.now() - started;
    const report = fs.existsSync(output) ? JSON.parse(fs.readFileSync(output, 'utf8')) : null;
    fs.writeFileSync(path.join(OUT, 'tml', `${id}.wrapper.json`), safeJson(result) + '\n');
    return { elapsedMs, ok: true, result, report, error: null };
  } catch (error) {
    const elapsedMs = performance.now() - started;
    const report = fs.existsSync(output) ? JSON.parse(fs.readFileSync(output, 'utf8')) : null;
    const serial = {
      name: error?.name ?? null,
      message: error?.message ?? String(error),
      code: error?.code ?? null,
      stdout: error?.stdout ?? null,
      stderr: error?.stderr ?? null,
    };
    fs.writeFileSync(path.join(OUT, 'tml', `${id}.error.json`), safeJson(serial) + '\n');
    return { elapsedMs, ok: false, result: null, report, error: serial };
  }
}

function tmlSummary(run) {
  if (run.result?.summary && Array.isArray(run.result.summary.messages)) return run.result.summary;
  if (run.report && Array.isArray(run.report.messages)) return run.report;
  return null;
}

function tmlFindings(run) {
  const summary = tmlSummary(run);
  return (summary?.messages ?? []).map(m => ({
    code: m.rule_id ?? null,
    severity: m.severity ?? null,
    file: m.file_name ?? null,
    field: m.field ?? null,
    rows: m.rows ?? null,
    message: m.message ?? null,
    raw: m,
  }));
}

function tmlSig(n) {
  return [n.code, n.severity, n.file, n.field, n.message, safeJson(n.rows ?? [])].join('|');
}

function findMobilityNoticeGroups(node, out = [], seen = new Set()) {
  if (!node || typeof node !== 'object') return out;
  if (seen.has(node)) return out;
  seen.add(node);
  if (!Array.isArray(node) && typeof node.code === 'string') {
    const hasNoticeShape =
      'severity' in node || 'totalNotices' in node || 'sampleNotices' in node ||
      'noticeCode' in node || 'total_notices' in node;
    if (hasNoticeShape) out.push(node);
  }
  if (Array.isArray(node)) {
    for (const x of node) findMobilityNoticeGroups(x, out, seen);
  } else {
    for (const v of Object.values(node)) findMobilityNoticeGroups(v, out, seen);
  }
  return out;
}

function mobilityGroups(report) {
  const groups = findMobilityNoticeGroups(report);
  const byKey = new Map();
  for (const g of groups) {
    const code = g.code ?? g.noticeCode ?? 'UNKNOWN';
    const severity = g.severity ?? null;
    const total = Number(g.totalNotices ?? g.total_notices ?? g.count ?? 1);
    const key = `${code}|${severity ?? ''}`;
    const prev = byKey.get(key);
    if (!prev || total > prev.total) {
      byKey.set(key, {
        code,
        severity,
        total: Number.isFinite(total) ? total : 1,
        sampleNotices: g.sampleNotices ?? g.sample_notices ?? null,
        raw: g,
      });
    }
  }
  return [...byKey.values()];
}

function runMobilityData(id) {
  const feed = path.join(FIXTURES, `${id}.zip`);
  const outputDir = path.join(OUT, 'mobilitydata', id);
  fs.rmSync(outputDir, { recursive: true, force: true });
  const started = performance.now();
  const proc = spawnSync('java', [
    '-jar', MOBILITYDATA_JAR,
    '-i', feed,
    '-o', outputDir,
  ], { cwd: REPO, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
  const elapsedMs = performance.now() - started;
  const reportPath = path.join(outputDir, 'report.json');
  const systemErrorsPath = path.join(outputDir, 'system_errors.json');
  const report = fs.existsSync(reportPath) ? JSON.parse(fs.readFileSync(reportPath, 'utf8')) : null;
  const systemErrors = fs.existsSync(systemErrorsPath)
    ? JSON.parse(fs.readFileSync(systemErrorsPath, 'utf8'))
    : null;
  const failureText = `${proc.stderr ?? ''}\n${proc.stdout ?? ''}\n${safeJson(systemErrors)}`;
  let processStatus = 'OK';
  if (!report) processStatus = classifyProcessFailure(failureText);
  return {
    elapsedMs,
    exitCode: proc.status,
    stdout: proc.stdout,
    stderr: proc.stderr,
    processStatus,
    report,
    systemErrors,
    groups: mobilityGroups(report),
  };
}

function deltaBySignature(current, baseline, sigFn) {
  const baseCounts = new Map();
  for (const x of baseline) {
    const sig = sigFn(x);
    baseCounts.set(sig, (baseCounts.get(sig) ?? 0) + 1);
  }
  const used = new Map();
  const out = [];
  for (const x of current) {
    const sig = sigFn(x);
    const n = used.get(sig) ?? 0;
    const b = baseCounts.get(sig) ?? 0;
    if (n >= b) out.push(x);
    used.set(sig, n + 1);
  }
  return out;
}

function mobilityDelta(current, baseline) {
  const base = new Map(baseline.map(g => [`${g.code}|${g.severity ?? ''}`, g.total]));
  return current
    .map(g => ({ ...g, delta: g.total - (base.get(`${g.code}|${g.severity ?? ''}`) ?? 0) }))
    .filter(g => g.delta > 0);
}

function escapeMd(s) {
  return String(s ?? '').replaceAll('|', '\\|').replaceAll('\n', ' ');
}

function statusFrom({ processStatus, newFindings, internalError = false }) {
  if (processStatus === 'CRASH') return 'CRASH';
  if (processStatus === 'ERROR') return 'ERROR';
  if (internalError) return 'INTERNAL_ERROR';
  return newFindings.length > 0 ? 'DETECTED' : 'MISSED';
}

const tmlInfo = await getValidatorInfo();
console.log('TML validator info:', tmlInfo);
console.log('MobilityData version:', MOBILITYDATA_VERSION);

const runs = { analyzer: {}, tml: {}, mobilitydata: {} };
for (const item of manifest) {
  console.log(`\n=== ${item.id}: ${item.description} ===`);
  runs.analyzer[item.id] = runAnalyzer(item.id);
  runs.tml[item.id] = await runTml(item.id);
  runs.mobilitydata[item.id] = runMobilityData(item.id);
  console.log(
    `Analyzer ${runs.analyzer[item.id].findings.length} findings (${runs.analyzer[item.id].elapsedMs.toFixed(1)} ms); ` +
    `TML ${tmlFindings(runs.tml[item.id]).length} findings (${runs.tml[item.id].elapsedMs.toFixed(1)} ms); ` +
    `MobilityData ${runs.mobilitydata[item.id].groups.length} notice groups (${runs.mobilitydata[item.id].elapsedMs.toFixed(1)} ms)`
  );
}

const baseAnalyzer = runs.analyzer.baseline.findings;
const baseTml = tmlFindings(runs.tml.baseline);
const baseMobility = runs.mobilitydata.baseline.groups;

const rows = [];
for (const item of manifest.filter(x => !x.control)) {
  const ar = runs.analyzer[item.id];
  const tr = runs.tml[item.id];
  const mr = runs.mobilitydata[item.id];

  const aDelta = deltaBySignature(ar.findings, baseAnalyzer, analyzerSig);
  const tFindings = tmlFindings(tr);
  const tDelta = deltaBySignature(tFindings, baseTml, tmlSig);
  const mDelta = mobilityDelta(mr.groups, baseMobility);

  const tFailureText = `${tr.error?.message ?? ''}\n${tr.error?.stderr ?? ''}`;
  const tProcessStatus = tr.ok ? 'OK' : classifyProcessFailure(tFailureText);

  const mdInternal = mDelta.some(g => /^runtime_exception_/i.test(g.code)) ||
    (Array.isArray(mr.systemErrors) && mr.systemErrors.length > 0) ||
    (mr.systemErrors && !Array.isArray(mr.systemErrors) && Object.keys(mr.systemErrors).length > 0);

  const expectedRule = item.expected_analyzer_rule;
  const expectedRuleDetected = expectedRule
    ? ar.findings.some(n => n.code === expectedRule)
    : null;
  const expectedRuleInBaseline = expectedRule
    ? baseAnalyzer.some(n => n.code === expectedRule)
    : null;

  let analyzerStatus = statusFrom({ processStatus: ar.processStatus, newFindings: aDelta });
  if (expectedRule && ar.processStatus === 'OK') {
    if (expectedRuleDetected && !expectedRuleInBaseline) analyzerStatus = 'DETECTED';
    else if (aDelta.length > 0) analyzerStatus = 'DETECTED_OTHER';
    else analyzerStatus = 'MISSED';
  }

  rows.push({
    id: item.id,
    group: item.group,
    description: item.description,
    expectedAnalyzerRule: expectedRule,
    expectedRuleDetected,
    expectedRuleInBaseline,
    analyzerStatus,
    analyzerNewFindings: aDelta.map(({ raw, ...x }) => x),
    analyzerMs: Number(ar.elapsedMs.toFixed(2)),
    tmlStatus: statusFrom({ processStatus: tProcessStatus, newFindings: tDelta }),
    tmlNewFindings: tDelta.map(({ raw, ...x }) => x),
    tmlMs: Number(tr.elapsedMs.toFixed(2)),
    mobilityDataStatus: statusFrom({
      processStatus: mr.processStatus,
      newFindings: mDelta,
      internalError: mdInternal,
    }),
    mobilityDataNewFindings: mDelta.map(({ raw, ...x }) => x),
    mobilityDataMs: Number(mr.elapsedMs.toFixed(2)),
    mobilityDataSystemErrors: mr.systemErrors,
  });
}

function engineTotals(statusKey) {
  const statuses = ['DETECTED', 'DETECTED_OTHER', 'MISSED', 'CRASH', 'ERROR', 'INTERNAL_ERROR'];
  return Object.fromEntries(statuses.map(s => [s, rows.filter(r => r[statusKey] === s).length]));
}

function groupStats(statusKey) {
  const byGroup = new Map();
  for (const row of rows) {
    if (!byGroup.has(row.group)) byGroup.set(row.group, []);
    byGroup.get(row.group).push(row[statusKey]);
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

const controls = {};
for (const item of manifest.filter(x => x.control)) {
  const tr = runs.tml[item.id];
  const mr = runs.mobilitydata[item.id];
  const ar = runs.analyzer[item.id];
  controls[item.id] = {
    analyzer: {
      processStatus: ar.processStatus,
      findingCount: ar.findings.length,
    },
    tml: {
      processStatus: tr.ok ? 'OK' : classifyProcessFailure(`${tr.error?.message ?? ''}\n${tr.error?.stderr ?? ''}`),
      findingCount: tmlFindings(tr).length,
      error: tr.error,
    },
    mobilitydata: {
      processStatus: mr.processStatus,
      noticeGroupCount: mr.groups.length,
      systemErrors: mr.systemErrors,
    },
  };
}

const result = {
  generatedAt: new Date().toISOString(),
  baseCommit: process.env.GITHUB_SHA ?? null,
  versions: {
    analyzer: 'workspace build',
    analyzerToday: TODAY,
    tml: TML_VERSION,
    tmlInfo,
    mobilitydata: MOBILITYDATA_VERSION,
  },
  methodology: {
    mutantCases: rows.length,
    semanticGroups: new Set(rows.map(r => r.group)).size,
    controls: manifest.filter(x => x.control).map(x => x.id),
    detection: 'A case is DETECTED when the single-mutant feed adds at least one structured finding relative to the identical baseline. Known Analyzer rules from the 30-case pilot are additionally checked by rule id. Process termination without a structured report is CRASH. MobilityData runtime_exception notices/system_errors are INTERNAL_ERROR.',
  },
  baseline: {
    analyzerFindingCount: baseAnalyzer.length,
    analyzerCodes: [...new Set(baseAnalyzer.map(x => x.code))].sort(),
    tmlFindingCount: baseTml.length,
    tmlSummary: tmlSummary(runs.tml.baseline),
    mobilityDataNoticeGroups: baseMobility.map(({ raw, ...x }) => x),
    mobilityDataSystemErrors: runs.mobilitydata.baseline.systemErrors,
  },
  totals: {
    analyzer: engineTotals('analyzerStatus'),
    tml: engineTotals('tmlStatus'),
    mobilitydata: engineTotals('mobilityDataStatus'),
  },
  groupStats: {
    analyzer: groupStats('analyzerStatus'),
    tml: groupStats('tmlStatus'),
    mobilitydata: groupStats('mobilityDataStatus'),
  },
  controls,
  cases: rows,
};

fs.writeFileSync(path.join(OUT, 'shootout-100x3.json'), safeJson(result) + '\n');

const aT = result.totals.analyzer;
const tT = result.totals.tml;
const mT = result.totals.mobilitydata;
const lines = [
  '# GTFS Validator Shootout — 100 controlled single-mutant fixtures × 3 engines',
  '',
  `Base checkout: \`${result.baseCommit ?? 'unknown'}\`. Analyzer date fixed at \`${TODAY}\`.`,
  `TML: \`${TML_VERSION}\`. MobilityData: \`${MOBILITYDATA_VERSION}\`.`,
  '',
  `**100 mutant cases across ${result.methodology.semanticGroups} semantic groups.**`,
  '',
  '| Engine | Detected | Detected-other | Missed | Crash | Internal/other error | Fully detected semantic groups |',
  '|---|---:|---:|---:|---:|---:|---:|',
  `| GTFS Analyzer | ${aT.DETECTED} | ${aT.DETECTED_OTHER} | ${aT.MISSED} | ${aT.CRASH} | ${aT.ERROR + aT.INTERNAL_ERROR} | ${result.groupStats.analyzer.fullyDetected}/${result.groupStats.analyzer.groups} |`,
  `| TML | ${tT.DETECTED} | ${tT.DETECTED_OTHER} | ${tT.MISSED} | ${tT.CRASH} | ${tT.ERROR + tT.INTERNAL_ERROR} | ${result.groupStats.tml.fullyDetected}/${result.groupStats.tml.groups} |`,
  `| MobilityData | ${mT.DETECTED} | ${mT.DETECTED_OTHER} | ${mT.MISSED} | ${mT.CRASH} | ${mT.ERROR + mT.INTERNAL_ERROR} | ${result.groupStats.mobilitydata.fullyDetected}/${result.groupStats.mobilitydata.groups} |`,
  '',
  '## Cases',
  '',
  '| # | Case | Group | Analyzer | TML | MobilityData | Analyzer rule check |',
  '|---:|---|---|:---:|:---:|:---:|---|',
];

let idx = 0;
for (const r of rows) {
  idx += 1;
  const ruleCheck = r.expectedAnalyzerRule
    ? `${r.expectedAnalyzerRule}: ${r.expectedRuleDetected ? 'yes' : 'NO'}${r.expectedRuleInBaseline ? ' (baseline already had rule)' : ''}`
    : 'delta-only';
  lines.push(
    `| ${idx} | ${escapeMd(r.id)} | ${escapeMd(r.group)} | ${r.analyzerStatus} | ${r.tmlStatus} | ${r.mobilityDataStatus} | ${escapeMd(ruleCheck)} |`
  );
}

lines.push(
  '',
  '## Paired controls',
  '',
  '```json',
  safeJson(controls),
  '```',
  '',
  '> Case-level totals are not a claim of complete validator recall. The suite contains 100 selected controlled mutations across 24 semantic groups; group-level results are reported to prevent repeated boundary variants from being mistaken for independent rule coverage.',
);

fs.writeFileSync(path.join(OUT, 'report-100x3.md'), lines.join('\n') + '\n');
console.log('\n' + lines.slice(0, 25).join('\n'));
console.log(`\nFull report: ${path.join(OUT, 'report-100x3.md')}`);
