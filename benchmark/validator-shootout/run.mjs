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

function analyzerNotices(run) {
  const r = run.report;
  if (!r || r.status !== 'ok' || !Array.isArray(r.notices)) return [];
  return r.notices;
}

function tmlSummary(run) {
  if (run.result?.summary && Array.isArray(run.result.summary.messages)) return run.result.summary;
  if (run.report && Array.isArray(run.report.messages)) return run.report;
  return null;
}

function tmlMessageSig(m) {
  return [m.rule_id, m.file_name, m.field, m.severity, m.message].map(v => v ?? '').join('|');
}

function escapeMd(s) {
  return String(s).replaceAll('|', '\\|').replaceAll('\n', ' ');
}

const tmlInfo = await getValidatorInfo();
console.log('TML validator info:', tmlInfo);

const analyzerRuns = {};
const tmlRuns = {};
for (const item of manifest) {
  console.log(`\n=== ${item.id}: ${item.description} ===`);
  analyzerRuns[item.id] = runAnalyzer(item.id);
  tmlRuns[item.id] = await runTml(item.id);
  console.log('Analyzer notices:', analyzerNotices(analyzerRuns[item.id]).length, `(${analyzerRuns[item.id].elapsedMs.toFixed(1)} ms)`);
  console.log('TML summary:', tmlSummary(tmlRuns[item.id]) ?? tmlRuns[item.id].error, `(${tmlRuns[item.id].elapsedMs.toFixed(1)} ms)`);
}

const baseTmlSummary = tmlSummary(tmlRuns.baseline);
const baseTmlMessageSigs = new Set((baseTmlSummary?.messages ?? []).map(tmlMessageSig));

const rows = [];
for (const item of manifest.filter(x => x.id !== 'baseline')) {
  const aNotices = analyzerNotices(analyzerRuns[item.id]);
  const expectedRule = item.expected_analyzer_rule;
  const expectedNotices = aNotices.filter(n => n.rule_id === expectedRule);

  const tr = tmlRuns[item.id];
  const ts = tmlSummary(tr);
  const newTmlMessages = (ts?.messages ?? []).filter(m => !baseTmlMessageSigs.has(tmlMessageSig(m)));
  const crashed = !tr.ok && /panic:|runtime error|index out of range/i.test(`${tr.error?.message ?? ''}\n${tr.error?.stderr ?? ''}`);
  const tmlStatus = crashed ? 'CRASH' : (!tr.ok ? 'ERROR' : (newTmlMessages.length > 0 ? 'DETECTED' : 'MISSED'));

  rows.push({
    id: item.id,
    description: item.description,
    expectedAnalyzerRule: expectedRule,
    analyzerStatus: expectedNotices.length > 0 ? 'DETECTED' : 'MISSED',
    analyzerMatchingNotices: expectedNotices.map(n => ({
      rule_id: n.rule_id,
      rule_class: n.rule_class,
      file: n.file,
      line: n.line,
      field: n.field,
      message: n.message,
    })),
    analyzerMs: Number(analyzerRuns[item.id].elapsedMs.toFixed(2)),
    tmlStatus,
    tmlNewMessages: newTmlMessages,
    tmlError: tr.ok ? null : tr.error,
    tmlMs: Number(tr.elapsedMs.toFixed(2)),
  });
}

const count = (who, status) => rows.filter(r => r[who] === status).length;
const result = {
  generatedAt: new Date().toISOString(),
  analyzerToday: TODAY,
  tmlInfo,
  baseline: {
    analyzerNoticeCount: analyzerNotices(analyzerRuns.baseline).length,
    analyzerRules: [...new Set(analyzerNotices(analyzerRuns.baseline).map(n => n.rule_id))].sort(),
    tmlStatus: tmlRuns.baseline.ok ? 'OK' : 'ERROR',
    tmlSummary: baseTmlSummary,
  },
  totals: {
    cases: rows.length,
    analyzerDetected: count('analyzerStatus', 'DETECTED'),
    analyzerMissed: count('analyzerStatus', 'MISSED'),
    tmlDetected: count('tmlStatus', 'DETECTED'),
    tmlMissed: count('tmlStatus', 'MISSED'),
    tmlCrash: count('tmlStatus', 'CRASH'),
    tmlError: count('tmlStatus', 'ERROR'),
  },
  cases: rows,
  note: 'Detection is semantic at fixture level. Analyzer is matched against the predeclared expected rule. TML is DETECTED only when the mutation creates a new structured message relative to the identical baseline; process panic is classified separately as CRASH.',
};

fs.writeFileSync(path.join(OUT, 'shootout.json'), safeJson(result) + '\n');

const lines = [
  '# GTFS Validator Shootout — controlled fixtures',
  '',
  `Analyzer date fixed at \`${TODAY}\`. Each mutant differs from the baseline by one intended mutation.`,
  '',
  `**Totals:** Analyzer ${result.totals.analyzerDetected}/${rows.length} detected; TML ${result.totals.tmlDetected}/${rows.length} detected, ${result.totals.tmlMissed} missed, ${result.totals.tmlCrash} crashed, ${result.totals.tmlError} other errors.`,
  '',
  '| Case | Expected Analyzer rule | Analyzer | TML | TML new rule(s) |',
  '|---|---|:---:|:---:|---|',
];
for (const r of rows) {
  const tmlRules = [...new Set(r.tmlNewMessages.map(m => m.rule_id))].join(', ') || '—';
  lines.push(`| ${escapeMd(r.id)} | ${escapeMd(r.expectedAnalyzerRule)} | ${r.analyzerStatus} | ${r.tmlStatus} | ${escapeMd(tmlRules)} |`);
}
lines.push('', '> CRASH means the validator process terminated without a structured validation result.');
fs.writeFileSync(path.join(OUT, 'report.md'), lines.join('\n') + '\n');

console.log('\n' + lines.join('\n'));
