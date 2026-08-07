import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { GTFSValidator, getValidatorInfo } from '@tmlmobilidade/gtfs-validator';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '../..');
const FIXTURES = path.join(HERE, 'fixtures');
const OUT = path.join(HERE, 'results');
const ANALYZER = path.join(REPO, 'target', 'release', 'gtfs-analyzer');
const TODAY = '20260807';

fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(path.join(OUT, 'analyzer'), { recursive: true });
fs.mkdirSync(path.join(OUT, 'tml'), { recursive: true });

const manifest = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'manifest.json'), 'utf8'));

function safeJson(value) {
  return JSON.stringify(value, (_key, v) => typeof v === 'bigint' ? v.toString() : v, 2);
}

function runAnalyzer(id) {
  const feed = path.join(FIXTURES, `${id}.zip`);
  const output = path.join(OUT, 'analyzer', `${id}.json`);
  const started = performance.now();
  const proc = spawnSync(ANALYZER, [
    'validate', feed,
    '--json', '--lang', 'en', '--today', TODAY,
    '--output', output,
  ], { cwd: REPO, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  const elapsedMs = performance.now() - started;
  if (proc.error) throw proc.error;
  if (!fs.existsSync(output)) {
    throw new Error(`Analyzer produced no JSON for ${id}; exit=${proc.status}; stderr=${proc.stderr}`);
  }
  return {
    elapsedMs,
    exitCode: proc.status,
    stderr: proc.stderr,
    report: JSON.parse(fs.readFileSync(output, 'utf8')),
  };
}

async function runTml(id) {
  const feed = path.join(FIXTURES, `${id}.zip`);
  const output = path.join(OUT, 'tml', `${id}.report.json`);
  const started = performance.now();
  try {
    const result = await GTFSValidator(feed, {
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

function analyzerNotices(run) {
  const r = run.report;
  if (!r || r.status !== 'ok' || !Array.isArray(r.notices)) return [];
  return r.notices;
}

function analyzerSig(n) {
  return [n.rule_id, n.rule_class, n.file, n.line, n.field, n.entity_type, n.entity_id, n.observed_value].map(v => v ?? '').join('|');
}

function numericCountMap(value, prefix = '', out = {}) {
  if (!value || typeof value !== 'object') return out;
  for (const [key, val] of Object.entries(value)) {
    const p = prefix ? `${prefix}.${key}` : key;
    if (typeof val === 'number' && /count|errors?|warnings?|infos?|notices?|issues?/i.test(key)) out[p] = val;
    else if (val && typeof val === 'object' && !Array.isArray(val) && prefix.split('.').length < 3) numericCountMap(val, p, out);
  }
  return out;
}

function tmlCounts(run) {
  return numericCountMap(run.result?.summary ?? run.report?.summary ?? {});
}

function collectNoticeLike(value, pathParts = [], out = []) {
  if (Array.isArray(value)) {
    const underIssueCollection = pathParts.some(p => /notice|error|warning|issue|violation/i.test(p));
    if (underIssueCollection) {
      for (const item of value) if (item && typeof item === 'object' && !Array.isArray(item)) out.push(item);
    }
    value.forEach((v, i) => collectNoticeLike(v, [...pathParts, String(i)], out));
    return out;
  }
  if (!value || typeof value !== 'object') return out;
  const keys = Object.keys(value);
  const underIssueCollection = pathParts.some(p => /notice|error|warning|issue|violation/i.test(p));
  if (underIssueCollection && keys.some(k => /code|message|severity|rule|filename|file|field|line/i.test(k))) out.push(value);
  for (const [k, v] of Object.entries(value)) collectNoticeLike(v, [...pathParts, k], out);
  return out;
}

function normalizedObjectSignature(obj) {
  const skip = /time|duration|date|timestamp|elapsed/i;
  const pieces = [];
  for (const key of Object.keys(obj).sort()) {
    const val = obj[key];
    if (skip.test(key)) continue;
    if (val == null || ['string', 'number', 'boolean'].includes(typeof val)) pieces.push(`${key}=${String(val)}`);
  }
  return pieces.join('|');
}

function tmlNoticeSignatures(run) {
  const source = run.report ?? run.result ?? {};
  return [...new Set(collectNoticeLike(source).map(normalizedObjectSignature).filter(Boolean))].sort();
}

function positiveCountDelta(base, mutant) {
  const keys = new Set([...Object.keys(base), ...Object.keys(mutant)]);
  const delta = {};
  for (const key of keys) {
    const d = (mutant[key] ?? 0) - (base[key] ?? 0);
    if (d !== 0) delta[key] = d;
  }
  return delta;
}

function escapeMd(s) {
  return String(s).replaceAll('|', '\\|').replaceAll('\n', ' ');
}

console.log('TML validator info:', await getValidatorInfo());

const analyzerRuns = {};
const tmlRuns = {};
for (const item of manifest) {
  console.log(`\n=== ${item.id}: ${item.description} ===`);
  analyzerRuns[item.id] = runAnalyzer(item.id);
  tmlRuns[item.id] = await runTml(item.id);
  console.log('Analyzer notices:', analyzerNotices(analyzerRuns[item.id]).length, `(${analyzerRuns[item.id].elapsedMs.toFixed(1)} ms)`);
  console.log('TML summary:', tmlRuns[item.id].result?.summary ?? tmlRuns[item.id].error, `(${tmlRuns[item.id].elapsedMs.toFixed(1)} ms)`);
}

const baseAnalyzerSigs = new Set(analyzerNotices(analyzerRuns.baseline).map(analyzerSig));
const baseTmlCounts = tmlCounts(tmlRuns.baseline);
const baseTmlSigs = new Set(tmlNoticeSignatures(tmlRuns.baseline));

const rows = [];
for (const item of manifest.filter(x => x.id !== 'baseline')) {
  const aNotices = analyzerNotices(analyzerRuns[item.id]);
  const aNew = aNotices.filter(n => !baseAnalyzerSigs.has(analyzerSig(n)));
  const aNewSpec = aNew.filter(n => n.rule_class === 'SPEC');
  const tCounts = tmlCounts(tmlRuns[item.id]);
  const countDelta = positiveCountDelta(baseTmlCounts, tCounts);
  const tSigs = tmlNoticeSignatures(tmlRuns[item.id]);
  const tNewSigs = tSigs.filter(s => !baseTmlSigs.has(s));
  const tPositiveCount = Object.values(countDelta).some(v => v > 0);
  const tDetected = tPositiveCount || tNewSigs.length > 0;

  rows.push({
    id: item.id,
    description: item.description,
    analyzerDetectedSpec: aNewSpec.length > 0,
    analyzerNewSpecRules: [...new Set(aNewSpec.map(n => n.rule_id))].sort(),
    analyzerNewRulesAll: [...new Set(aNew.map(n => n.rule_id))].sort(),
    analyzerMs: Number(analyzerRuns[item.id].elapsedMs.toFixed(2)),
    tmlOk: tmlRuns[item.id].ok,
    tmlDetectedPreliminary: tDetected,
    tmlCountDelta: countDelta,
    tmlNewNoticeSignatures: tNewSigs.slice(0, 20),
    tmlMs: Number(tmlRuns[item.id].elapsedMs.toFixed(2)),
  });
}

const result = {
  generatedAt: new Date().toISOString(),
  analyzerToday: TODAY,
  tmlInfo: await getValidatorInfo(),
  baseline: {
    analyzerNoticeCount: analyzerNotices(analyzerRuns.baseline).length,
    analyzerRules: [...new Set(analyzerNotices(analyzerRuns.baseline).map(n => n.rule_id))].sort(),
    tmlOk: tmlRuns.baseline.ok,
    tmlCounts: baseTmlCounts,
    tmlNoticeSignatureCount: baseTmlSigs.size,
  },
  cases: rows,
  note: 'TML detected is preliminary: positive summary-count delta or a new notice-like record versus the paired baseline. Raw TML reports are retained for semantic adjudication.',
};

fs.writeFileSync(path.join(OUT, 'shootout.json'), safeJson(result) + '\n');

const lines = [
  '# GTFS Validator Shootout — discovery run',
  '',
  `Analyzer date fixed at \`${TODAY}\`. Each mutant is compared against one identical baseline feed.`,
  '',
  '| Case | Analyzer SPEC detection | Analyzer new SPEC rules | TML preliminary detection | TML count delta |',
  '|---|:---:|---|:---:|---|',
];
for (const r of rows) {
  lines.push(`| ${escapeMd(r.id)} | ${r.analyzerDetectedSpec ? 'YES' : 'NO'} | ${escapeMd(r.analyzerNewSpecRules.join(', ') || '—')} | ${r.tmlDetectedPreliminary ? 'YES' : 'NO'} | ${escapeMd(JSON.stringify(r.tmlCountDelta))} |`);
}
lines.push('', '> TML result is preliminary until the raw TML notice schema is semantically mapped.');
fs.writeFileSync(path.join(OUT, 'report.md'), lines.join('\n') + '\n');

console.log('\n' + lines.join('\n'));
