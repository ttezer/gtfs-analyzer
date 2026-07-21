import type { Locale } from './i18n';
import {
  ruleClassForLocale,
  severityForLocale,
  tForLocale,
  tMsgForLocale,
  tRemediationForLocale,
} from './i18n';
import { escHtml } from './escape';
import type { Notice, R9Item, RuleClass, Severity, ValidationResult } from './types';

export type ExecutivePriority = 'P0' | 'P1' | 'P2';

export interface ExecutiveFinding {
  priority: ExecutivePriority;
  ruleId: string;
  title: string;
  severity: Severity;
  severityLabel: string;
  ruleClass: RuleClass;
  classLabel: string;
  actualCount: number;
  evidence: string;
  impact: string;
  action: string;
  scoreDelta: number;
  publishScoreDelta: number;
  effort: number;
}

export interface ExecutiveReportModel {
  locale: Locale;
  fileName: string;
  generatedAt: string;
  appVersion: string;
  engineMode: string;
  publishable: boolean;
  scores: {
    overall: number;
    publish: number;
    spec: number;
    interop: number;
    quality: number;
    analytics: number;
  };
  profile: {
    stops: number;
    routes: number;
    trips: number;
    shapes: number;
    activeServiceDays: number;
    averageDailyTrips: number;
    feedRange: string;
    serviceRange: string;
    files: number;
  };
  totals: {
    actual: number;
    displayed: number;
    ruleTypes: number;
    capped: boolean;
    bySeverity: Array<{ code: Severity; label: string; count: number }>;
    byClass: Array<{ code: RuleClass; label: string; count: number }>;
  };
  findings: ExecutiveFinding[];
  insights: string[];
  topRules: Array<{ ruleId: string; title: string; severity: string; classLabel: string; count: number }>;
  fileStats: Array<{ name: string; rows: number }>;
}

interface BuildOptions {
  result: ValidationResult;
  fileName: string;
  generatedAt: string;
  locale: Locale;
  appVersion: string;
  engineMode: string;
}

const MAX_FINDINGS = 8;
const MAX_TOP_RULES = 8;
const MAX_FILE_STATS = 20;

const REPORT_TEXT = {
  tr: {
    reportTitle: 'GTFS Analyzer Yönetici Raporu',
    reportSubtitle: 'Yayın hazırlığı, veri kalitesi ve uygulanabilir iyileştirme planı',
    generated: 'Oluşturulma', version: 'GTFS Analyzer sürümü', engine: 'Analiz motoru', printReport: 'Yazdır / PDF Kaydet',
    confidential: 'Karar desteği için hazırlanmıştır',
    executiveSummary: 'Yönetici Özeti',
    ready: 'Yayına hazır', blocked: 'Yayın engelli',
    summaryReady: 'Analiz, yayını engelleyen bir bulgu tespit etmedi. Öncelikli kalite iyileştirmeleri yine de planlanmalıdır.',
    summaryBlocked: 'Feed, yayın öncesinde giderilmesi gereken engelleyici bulgular içeriyor. Önce P0 maddelerini kapatın ve analizi yeniden çalıştırın.',
    actualFindings: 'Gerçek bulgu örneği', ruleTypes: 'Etkilenen kural', selectedActions: 'Öncelikli aksiyon',
    scoreNote: 'Skorlar GTFS feed’ini değerlendirir; GTFS Analyzer ürününün performansını veya doğruluğunu puanlamaz.',
    feedProfile: 'Feed Profili ve Skorlar', feedProfileIntro: 'Bu görünüm, feed ölçeğini ve yayın/kalite risklerini tek sayfada özetler.',
    stops: 'Durak', routes: 'Hat', trips: 'Sefer', shapes: 'Shape', activeDays: 'Aktif servis günü', dailyTrips: 'Ort. günlük sefer',
    feedRange: 'Feed geçerlilik aralığı', serviceRange: 'Gerçek servis aralığı', files: 'Dosya',
    publishScore: 'Yayın Skoru', overallScore: 'Genel Skor', spec: 'Spec', interop: 'Interop', quality: 'Quality', analytics: 'Analytics',
    prioritizedFindings: 'Öncelikli Bulgular', prioritizedIntro: 'R1 yayın engelleri ile R9 etki/efor sıralaması birleştirilmiştir. Aynı kural tek aksiyon altında toplanır.',
    evidence: 'Kanıt', why: 'Neden önemli', action: 'Önerilen aksiyon', affected: 'Etkilenen örnek', effort: 'Efor', scoreImpact: 'Skor etkisi',
    noFindings: 'Önceliklendirilecek bulgu bulunamadı.',
    diagnosticCoverage: 'Tanısal Kapsam', coverageIntro: 'GTFS Analyzer; spesifikasyon uygunluğu, sistemler arası uyumluluk, veri kalitesi ve analitik kullanılabilirliği birlikte inceler.',
    feedInsights: 'Feed’e Özel İçgörüler', noInsight: 'Ek yapısal risk sinyali tespit edilmedi.',
    remediationPlan: 'İyileştirme Planı', remediationIntro: 'Aksiyonlar bağımlılık ve yayın etkisine göre aşamalandırılmıştır.',
    phase: 'Aşama', objective: 'Hedef', exit: 'Tamamlanma ölçütü',
    phase0: 'P0 · Yayın engelleri', phase0Obj: 'Kritik yapısal ve zorunlu alan sorunlarını giderin.', phase0Exit: 'R1 yayın engeli kalmaz; analiz yeniden çalıştırılır.',
    phase1: 'P1 · Uyumluluk ve yayılım', phase1Obj: 'Çok sayıda kaydı veya tüketiciyi etkileyen sorunları düzeltin.', phase1Exit: 'Yüksek etkili R9 maddeleri kapanır ve örnek veri doğrulanır.',
    phase2: 'P2 · Kalite geliştirme', phase2Obj: 'Sunum, erişilebilirlik ve analitik kaliteyi iyileştirin.', phase2Exit: 'Kalan bulgular kabul edilir veya sahip/tarih ile planlanır.',
    appendix: 'Teknik Ek', appendixIntro: 'Aşağıdaki sayılar gösterim sınırı yerine mümkün olduğunda gerçek kural toplamlarını kullanır.',
    severityDistribution: 'Önem Dağılımı', classDistribution: 'Analiz Sınıfı Dağılımı', topRules: 'En Yüksek Hacimli Kurallar', fileInventory: 'Dosya Envanteri',
    rule: 'Kural', severity: 'Önem', class: 'Sınıf', count: 'Sayı', file: 'Dosya', rows: 'Satır',
    cappedNote: 'Arayüzde gösterilen {displayed} örneğe karşılık gerçek toplam {actual} olarak hesaplandı.',
    offline: 'Bu rapor tarayıcıda, yüklenen feed ve GTFS Analyzer sonuçları kullanılarak yerel olarak oluşturulmuştur; harici bir API gerekmez.',
    unknown: 'Bilinmiyor', notAvailable: 'Mevcut değil',
    impactP0: 'Yayınlanabilirliği doğrudan engeller ve tüketici sistemlerinde feed’in reddedilmesine yol açabilir.',
    impactP1: 'Sistemler arası uyumluluğu veya çok sayıda kaydın güvenilir kullanımını etkileyebilir.',
    impactP2: 'Yolcu deneyimi, bakım kolaylığı veya analitik sonuçların kalitesini düşürebilir.',
    insightRouteTrip: 'Hat sayısı sefer sayısıyla birebir aynı ({count}). Bu, sefer başına ayrı hat üretildiğine işaret edebilir; route modellemesini gözden geçirin.',
    insightNoShapes: '{trips} sefer olmasına rağmen shape bulunmuyor. Harita gösterimi, mesafe ve güzergah tabanlı analizler sınırlı kalabilir.',
    insightNoService: 'Geçerli bir gerçek servis tarih aralığı hesaplanamadı. Takvim ve istisna kayıtlarını doğrulayın.',
    insightCapped: 'En az bir kural gösterim sınırını aşıyor. Bu rapor öncelik ve toplamlar için gerçek toplu sayıları kullanır.',
    levelStrong: 'Güçlü', levelWatch: 'İzlenmeli', levelAction: 'Aksiyon gerekli',
  },
  en: {
    reportTitle: 'GTFS Analyzer Executive Report',
    reportSubtitle: 'Publication readiness, data quality, and an actionable improvement plan',
    generated: 'Generated', version: 'GTFS Analyzer version', engine: 'Analysis engine', printReport: 'Print / Save PDF',
    confidential: 'Prepared for decision support',
    executiveSummary: 'Executive Summary',
    ready: 'Ready to publish', blocked: 'Publication blocked',
    summaryReady: 'The analysis found no publication blocker. Prioritized quality improvements should still be planned.',
    summaryBlocked: 'The feed contains blockers that must be resolved before publication. Close P0 items first and rerun the analysis.',
    actualFindings: 'Actual finding instances', ruleTypes: 'Affected rules', selectedActions: 'Priority actions',
    scoreNote: 'Scores assess the GTFS feed; they do not rate the performance or accuracy of GTFS Analyzer.',
    feedProfile: 'Feed Profile and Scores', feedProfileIntro: 'This view summarizes feed scale and publication/quality risk on one page.',
    stops: 'Stops', routes: 'Routes', trips: 'Trips', shapes: 'Shapes', activeDays: 'Active service days', dailyTrips: 'Avg. daily trips',
    feedRange: 'Feed validity range', serviceRange: 'Actual service range', files: 'Files',
    publishScore: 'Publish Score', overallScore: 'Overall Score', spec: 'Spec', interop: 'Interop', quality: 'Quality', analytics: 'Analytics',
    prioritizedFindings: 'Prioritized Findings', prioritizedIntro: 'R1 publication blockers are combined with R9 impact/effort ranking. Each rule is consolidated into one action.',
    evidence: 'Evidence', why: 'Why it matters', action: 'Recommended action', affected: 'Affected instances', effort: 'Effort', scoreImpact: 'Score impact',
    noFindings: 'No findings require prioritization.',
    diagnosticCoverage: 'Diagnostic Coverage', coverageIntro: 'GTFS Analyzer evaluates specification compliance, interoperability, data quality, and analytical usability together.',
    feedInsights: 'Feed-Specific Insights', noInsight: 'No additional structural risk signal was identified.',
    remediationPlan: 'Remediation Plan', remediationIntro: 'Actions are phased by dependency and publication impact.',
    phase: 'Phase', objective: 'Objective', exit: 'Exit criterion',
    phase0: 'P0 · Publication blockers', phase0Obj: 'Resolve critical structural and required-field issues.', phase0Exit: 'No R1 blocker remains; rerun the analysis.',
    phase1: 'P1 · Interoperability and propagation', phase1Obj: 'Fix issues affecting many records or consuming systems.', phase1Exit: 'High-impact R9 items are closed and sample data is verified.',
    phase2: 'P2 · Quality improvement', phase2Obj: 'Improve presentation, accessibility, and analytical quality.', phase2Exit: 'Remaining findings are accepted or assigned an owner and date.',
    appendix: 'Technical Appendix', appendixIntro: 'The counts below use actual rule totals, rather than display caps, whenever available.',
    severityDistribution: 'Severity Distribution', classDistribution: 'Analysis Class Distribution', topRules: 'Highest-Volume Rules', fileInventory: 'File Inventory',
    rule: 'Rule', severity: 'Severity', class: 'Class', count: 'Count', file: 'File', rows: 'Rows',
    cappedNote: 'The actual total is {actual}, compared with {displayed} instances retained for display.',
    offline: 'This report was generated locally in the browser from the uploaded feed and GTFS Analyzer results; no external API is required.',
    unknown: 'Unknown', notAvailable: 'Not available',
    impactP0: 'Directly blocks publication and may cause consuming systems to reject the feed.',
    impactP1: 'May affect interoperability or the reliable use of a large number of records.',
    impactP2: 'May reduce rider experience, maintainability, or the quality of analytical outputs.',
    insightRouteTrip: 'Route count exactly matches trip count ({count}). This may indicate one route per trip; review the route model.',
    insightNoShapes: 'There are {trips} trips but no shapes. Map display, distance, and route-based analysis may remain limited.',
    insightNoService: 'No valid actual service date range could be calculated. Verify calendar and exception records.',
    insightCapped: 'At least one rule exceeds the display cap. This report uses actual aggregate counts for totals and priorities.',
    levelStrong: 'Strong', levelWatch: 'Monitor', levelAction: 'Action required',
  },
  ja: {
    reportTitle: 'GTFS Analyzer エグゼクティブレポート',
    reportSubtitle: '公開準備状況、データ品質、実行可能な改善計画',
    generated: '作成日時', version: 'GTFS Analyzer バージョン', engine: '解析エンジン', printReport: '印刷 / PDF保存',
    confidential: '意思決定支援用',
    executiveSummary: 'エグゼクティブサマリー',
    ready: '公開可能', blocked: '公開不可',
    summaryReady: '解析では公開を妨げる問題は検出されませんでした。優先度の高い品質改善は引き続き計画してください。',
    summaryBlocked: '公開前に解決すべきブロッカーがあります。まずP0項目を解消し、解析を再実行してください。',
    actualFindings: '実際の指摘件数', ruleTypes: '該当ルール', selectedActions: '優先アクション',
    scoreNote: 'スコアはGTFSフィードを評価するものであり、GTFS Analyzer自体の性能や精度を評価するものではありません。',
    feedProfile: 'フィード概要とスコア', feedProfileIntro: 'フィード規模と公開・品質リスクを1ページにまとめます。',
    stops: '停留所', routes: '路線', trips: '便', shapes: 'シェープ', activeDays: '運行日数', dailyTrips: '1日平均便数',
    feedRange: 'フィード有効期間', serviceRange: '実サービス期間', files: 'ファイル',
    publishScore: '公開スコア', overallScore: '総合スコア', spec: '仕様', interop: '相互運用性', quality: '品質', analytics: '分析',
    prioritizedFindings: '優先度付き指摘', prioritizedIntro: 'R1の公開ブロッカーとR9の影響度・工数順位を統合し、同一ルールを1つのアクションにまとめています。',
    evidence: '根拠', why: '重要な理由', action: '推奨アクション', affected: '影響件数', effort: '工数', scoreImpact: 'スコア影響',
    noFindings: '優先対応が必要な指摘はありません。',
    diagnosticCoverage: '診断範囲', coverageIntro: 'GTFS Analyzerは、仕様準拠、相互運用性、データ品質、分析での利用可能性を総合的に確認します。',
    feedInsights: 'フィード固有の洞察', noInsight: '追加の構造的リスクシグナルは検出されませんでした。',
    remediationPlan: '改善計画', remediationIntro: '依存関係と公開への影響に基づいて段階化しています。',
    phase: 'フェーズ', objective: '目的', exit: '完了条件',
    phase0: 'P0 · 公開ブロッカー', phase0Obj: '重大な構造問題と必須フィールド問題を解決します。', phase0Exit: 'R1ブロッカーがなくなった状態で解析を再実行します。',
    phase1: 'P1 · 相互運用性と波及', phase1Obj: '多数のレコードや利用システムに影響する問題を修正します。', phase1Exit: '影響度の高いR9項目を解消し、サンプルデータを検証します。',
    phase2: 'P2 · 品質改善', phase2Obj: '表示、アクセシビリティ、分析品質を改善します。', phase2Exit: '残りの指摘を受容するか、担当者と期限を設定します。',
    appendix: '技術付録', appendixIntro: '以下は可能な限り表示上限ではなく、ルールごとの実件数を使用しています。',
    severityDistribution: '重要度分布', classDistribution: '解析クラス分布', topRules: '件数の多いルール', fileInventory: 'ファイル一覧',
    rule: 'ルール', severity: '重要度', class: 'クラス', count: '件数', file: 'ファイル', rows: '行数',
    cappedNote: '表示用に保持された{displayed}件に対し、実際の合計は{actual}件です。',
    offline: 'このレポートはアップロードされたフィードとGTFS Analyzerの結果からブラウザ内でローカル生成され、外部APIを必要としません。',
    unknown: '不明', notAvailable: '該当なし',
    impactP0: '公開を直接妨げ、利用システムでフィードが拒否される可能性があります。',
    impactP1: '相互運用性または多数のレコードの信頼できる利用に影響する可能性があります。',
    impactP2: '利用者体験、保守性、分析結果の品質を低下させる可能性があります。',
    insightRouteTrip: '路線数と便数が完全に一致しています（{count}）。便ごとに路線を作成している可能性があるため、路線モデルを確認してください。',
    insightNoShapes: '{trips}便ありますがシェープがありません。地図表示、距離、経路ベースの解析が制限される可能性があります。',
    insightNoService: '有効な実サービス期間を算出できませんでした。calendarと例外レコードを確認してください。',
    insightCapped: '少なくとも1つのルールが表示上限を超えています。本レポートは合計と優先順位に実集計値を使用します。',
    levelStrong: '良好', levelWatch: '要監視', levelAction: '対応必要',
  },
} as const;

type ReportText = { [Key in keyof typeof REPORT_TEXT.en]: string };

const INTL_LOCALE: Record<Locale, string> = { tr: 'tr-TR', en: 'en-US', ja: 'ja-JP' };
const SEVERITIES: Severity[] = ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO'];
const CLASSES: RuleClass[] = ['SPEC', 'INTEROP', 'QUALITY', 'ANALYTICS'];

function replace(text: string, params: Record<string, string | number>): string {
  let value = text;
  for (const [key, replacement] of Object.entries(params)) value = value.split(`{${key}}`).join(String(replacement));
  return value;
}

function dateRange(start: number | null, end: number | null, missing: string): string {
  const date = (value: number): string => {
    const s = String(value).padStart(8, '0');
    return `${s.slice(0, 4)}-${s.slice(4, 6)}-${s.slice(6, 8)}`;
  };
  return start != null && end != null ? `${date(start)} — ${date(end)}` : missing;
}

function priorityFor(item: R9Item | undefined, blocker: boolean, notice: Notice): ExecutivePriority {
  if (blocker) return 'P0';
  if (notice.severity === 'CRITICAL') return 'P0';
  if (notice.severity === 'HIGH' || item?.labels.some(label =>
    label === 'interop' || label === 'propagation' || label === 'widespread' || label === 'high-impact')) return 'P1';
  return 'P2';
}

function priorityRank(priority: ExecutivePriority): number {
  return priority === 'P0' ? 0 : priority === 'P1' ? 1 : 2;
}

export function buildExecutiveReportModel(options: BuildOptions): ExecutiveReportModel {
  const { result, fileName, generatedAt, locale, appVersion, engineMode } = options;
  const copy = REPORT_TEXT[locale];
  const number = new Intl.NumberFormat(INTL_LOCALE[locale]);
  const noticesById = new Map(result.notices.map(notice => [notice.id, notice]));
  const noticeByRule = new Map<string, Notice>();
  for (const notice of result.notices) if (!noticeByRule.has(notice.rule_id)) noticeByRule.set(notice.rule_id, notice);
  const blockerIds = new Set(result.reports.r1.blocker_notice_ids);
  const blockerRules = new Set(result.reports.r1.blocker_notice_ids.map(id => noticesById.get(id)?.rule_id).filter((id): id is string => Boolean(id)));
  const r9ByRule = new Map(result.reports.r9.items.map(item => [item.rule_id, item]));

  const actualByRule = new Map<string, number>();
  for (const notice of result.notices) actualByRule.set(notice.rule_id, (actualByRule.get(notice.rule_id) ?? 0) + 1);
  for (const [ruleId, count] of Object.entries(result.capped_totals ?? {})) actualByRule.set(ruleId, count);

  const actualBySeverity = new Map<Severity, number>(SEVERITIES.map(code => [code, 0]));
  const actualByClass = new Map<RuleClass, number>(CLASSES.map(code => [code, 0]));
  for (const [ruleId, count] of actualByRule) {
    const representative = noticeByRule.get(ruleId);
    if (!representative) continue;
    actualBySeverity.set(representative.severity, (actualBySeverity.get(representative.severity) ?? 0) + count);
    actualByClass.set(representative.rule_class, (actualByClass.get(representative.rule_class) ?? 0) + count);
  }

  const orderedRules: string[] = [];
  for (const id of result.reports.r1.blocker_notice_ids) {
    const ruleId = noticesById.get(id)?.rule_id;
    if (ruleId && !orderedRules.includes(ruleId)) orderedRules.push(ruleId);
  }
  for (const item of result.reports.r9.items) if (!orderedRules.includes(item.rule_id)) orderedRules.push(item.rule_id);
  for (const notice of result.notices) if (!orderedRules.includes(notice.rule_id)) orderedRules.push(notice.rule_id);

  const findings = orderedRules.flatMap((ruleId): ExecutiveFinding[] => {
    const notice = noticeByRule.get(ruleId);
    if (!notice) return [];
    const item = r9ByRule.get(ruleId);
    const priority = priorityFor(item, blockerRules.has(ruleId) || blockerIds.has(notice.id), notice);
    const action = tRemediationForLocale(locale, notice) || copy.notAvailable;
    return [{
      priority,
      ruleId,
      title: tForLocale(locale, `rule.${ruleId}`),
      severity: notice.severity,
      severityLabel: severityForLocale(locale, notice.severity),
      ruleClass: notice.rule_class,
      classLabel: ruleClassForLocale(locale, notice.rule_class),
      actualCount: actualByRule.get(ruleId) ?? 1,
      evidence: tMsgForLocale(locale, notice),
      impact: priority === 'P0' ? copy.impactP0 : priority === 'P1' ? copy.impactP1 : copy.impactP2,
      action,
      scoreDelta: item?.score_delta ?? 0,
      publishScoreDelta: item?.pub_score_delta ?? 0,
      effort: item?.fix_effort ?? notice.base_effort,
    }];
  }).sort((a, b) => {
    const priority = priorityRank(a.priority) - priorityRank(b.priority);
    if (priority !== 0) return priority;
    const r9 = (r9ByRule.get(b.ruleId)?.priority_score ?? 0) - (r9ByRule.get(a.ruleId)?.priority_score ?? 0);
    return r9 !== 0 ? r9 : b.actualCount - a.actualCount || a.ruleId.localeCompare(b.ruleId);
  }).slice(0, MAX_FINDINGS);

  const insights: string[] = [];
  if (result.metrics.route_count > 1000 && result.metrics.route_count === result.metrics.trip_count) {
    insights.push(replace(copy.insightRouteTrip, { count: number.format(result.metrics.route_count) }));
  }
  if (result.metrics.trip_count > 0 && result.metrics.shape_count === 0) {
    insights.push(replace(copy.insightNoShapes, { trips: number.format(result.metrics.trip_count) }));
  }
  if (result.metrics.service_start_date == null || result.metrics.service_end_date == null) insights.push(copy.insightNoService);
  const actualTotal = [...actualByRule.values()].reduce((sum, value) => sum + value, 0);
  if (actualTotal > result.notices.length) insights.push(copy.insightCapped);

  const topRules = [...actualByRule.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .flatMap(([ruleId, count]) => {
      const notice = noticeByRule.get(ruleId);
      return notice ? [{
        ruleId,
        title: tForLocale(locale, `rule.${ruleId}`),
        severity: severityForLocale(locale, notice.severity),
        classLabel: ruleClassForLocale(locale, notice.rule_class),
        count,
      }] : [];
    })
    .slice(0, MAX_TOP_RULES);

  return {
    locale, fileName, generatedAt, appVersion, engineMode,
    publishable: result.reports.r1.publishable,
    scores: {
      overall: result.reports.r5.score,
      publish: result.reports.r5.pub_score,
      spec: result.reports.r5.spec_score,
      interop: result.reports.r5.interop_score,
      quality: result.reports.r5.quality_score,
      analytics: result.reports.r5.analytics_score,
    },
    profile: {
      stops: result.metrics.stop_count,
      routes: result.metrics.route_count,
      trips: result.metrics.trip_count,
      shapes: result.metrics.shape_count,
      activeServiceDays: result.metrics.active_service_days,
      averageDailyTrips: result.metrics.avg_daily_trips,
      feedRange: dateRange(result.metrics.feed_start_date, result.metrics.feed_end_date, copy.notAvailable),
      serviceRange: dateRange(result.metrics.service_start_date, result.metrics.service_end_date, copy.notAvailable),
      files: result.metrics.file_stats.length,
    },
    totals: {
      actual: actualTotal,
      displayed: result.notices.length,
      ruleTypes: actualByRule.size,
      capped: actualTotal > result.notices.length,
      bySeverity: SEVERITIES.map(code => ({ code, label: severityForLocale(locale, code), count: actualBySeverity.get(code) ?? 0 })),
      byClass: CLASSES.map(code => ({ code, label: ruleClassForLocale(locale, code), count: actualByClass.get(code) ?? 0 })),
    },
    findings,
    insights: insights.slice(0, 4),
    topRules,
    fileStats: [...result.metrics.file_stats]
      .sort((a, b) => b.rows - a.rows || a.name.localeCompare(b.name))
      .slice(0, MAX_FILE_STATS)
      .map(file => ({ name: file.name, rows: file.rows })),
  };
}

function scoreLevel(score: number, copy: ReportText): string {
  return score >= 85 ? copy.levelStrong : score >= 70 ? copy.levelWatch : copy.levelAction;
}

export function buildExecutiveReportHtml(model: ExecutiveReportModel): string {
  const copy = REPORT_TEXT[model.locale];
  const number = new Intl.NumberFormat(INTL_LOCALE[model.locale]);
  const decimal = new Intl.NumberFormat(INTL_LOCALE[model.locale], { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  const e = (value: string): string => escHtml(value);
  const n = (value: number): string => number.format(value);
  const d = (value: number): string => decimal.format(value);
  const score = (label: string, value: number, tone: 'green' | 'blue' | 'cyan' | 'amber' | 'violet' | 'red'): string => `
    <div class="score score-${tone}">
      <div class="score-ring" style="--score:${Math.max(0, Math.min(100, value))}"><span>${d(value)}</span></div>
      <div><strong>${e(label)}</strong><small>${e(scoreLevel(value, copy))}</small></div>
    </div>`;
  const metric = (label: string, value: string): string => `<div class="metric"><strong>${e(value)}</strong><span>${e(label)}</span></div>`;
  const findingCards = model.findings.map((finding, index) => `
    <article class="finding finding-${finding.priority.toLowerCase()}">
      <header>
        <span class="priority priority-${finding.priority.toLowerCase()}">${finding.priority}</span>
        <div><p class="eyebrow">${String(index + 1).padStart(2, '0')} · ${e(finding.ruleId)} · ${e(finding.severityLabel)} · ${e(finding.classLabel)}</p><h3>${e(finding.title)}</h3></div>
        <strong class="finding-count">${n(finding.actualCount)}<small>${e(copy.affected)}</small></strong>
      </header>
      <div class="finding-grid">
        <section class="finding-evidence"><h4>${e(copy.evidence)}</h4><p>${e(finding.evidence)}</p></section>
        <section><h4>${e(copy.why)}</h4><p>${e(finding.impact)}</p></section>
        <section><h4>${e(copy.action)}</h4><p>${e(finding.action)}</p></section>
      </div>
      <footer>${e(copy.effort)}: <strong>${d(finding.effort)}</strong><span>·</span>${e(copy.scoreImpact)}: <strong>+${d(finding.scoreDelta)}</strong>${finding.publishScoreDelta > 0 ? ` <span>·</span> ${e(copy.publishScore)}: <strong>+${d(finding.publishScoreDelta)}</strong>` : ''}</footer>
    </article>`);
  const distribution = (rows: Array<{ code: string; label: string; count: number }>): string => rows.map(row => {
    const pct = model.totals.actual > 0 ? Math.max(1, row.count / model.totals.actual * 100) : 0;
    return `<div class="bar-row bar-${e(row.code.toLowerCase())}"><span>${e(row.label)}</span><div><i style="width:${pct.toFixed(2)}%"></i></div><strong>${n(row.count)}</strong></div>`;
  }).join('');
  const findingChunks: string[][] = [];
  for (let index = 0; index < findingCards.length; index += 2) findingChunks.push(findingCards.slice(index, index + 2));
  if (findingChunks.length === 0) findingChunks.push([`<p class="empty">${e(copy.noFindings)}</p>`]);
  const pageNumber = (value: number): string => String(value).padStart(2, '0');
  const findingPages = findingChunks.map((cards, index) => `
  <section class="page">
    <div class="section-head"><div><p class="eyebrow">03 · ${index + 1}/${findingChunks.length}</p><h2>${e(copy.prioritizedFindings)}</h2><p class="muted">${e(copy.prioritizedIntro)}</p></div><div class="brand"><span class="brand-mark">GA</span>GTFS Analyzer</div></div>
    ${cards.join('')}
    <div class="page-number">${pageNumber(4 + index)}</div>
  </section>`).join('');
  const coveragePage = 4 + findingChunks.length;
  const appendixPage = coveragePage + 1;
  const inventoryPage = appendixPage + 1;

  return `<!doctype html>
<html lang="${model.locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>${e(copy.reportTitle)} — ${e(model.fileName)}</title>
  <style id="executive-print-layout">
    :root{--ink:#14213d;--muted:#5f6b7a;--line:#dce3eb;--soft:#f4f7fa;--blue:#076b91;--cyan:#10a4b8;--green:#12a36d;--amber:#e7a117;--red:#d94646;--violet:#7458b3}*{box-sizing:border-box}html{font-size:10.5pt}body{margin:0;color:var(--ink);font-family:Inter,"Noto Sans",system-ui,-apple-system,"Segoe UI",sans-serif;background:#e9eff4;line-height:1.45}.page{width:210mm;min-height:297mm;margin:8mm auto;padding:17mm 17mm 15mm;background:white;box-shadow:0 8px 30px #23364b28;position:relative}.brand{display:flex;align-items:center;gap:10px;font-weight:850;letter-spacing:.02em}.brand-mark{width:34px;height:34px;border-radius:9px;background:linear-gradient(135deg,var(--blue),var(--cyan));display:grid;place-items:center;color:white}.page-number{position:absolute;bottom:9mm;right:17mm;color:#8793a2;font-size:8pt}.eyebrow{margin:0;color:var(--blue);font-size:8pt;font-weight:800;letter-spacing:.12em;text-transform:uppercase}h1{font-size:31pt;line-height:1.06;margin:20mm 0 4mm;max-width:155mm}h2{font-size:20pt;line-height:1.15;margin:5mm 0 2mm}h3{font-size:12pt;line-height:1.25;margin:1mm 0}h4{font-size:8pt;text-transform:uppercase;letter-spacing:.08em;color:var(--blue);margin:0 0 1mm}p{margin:0 0 3mm}.muted,.intro{color:var(--muted)}.intro{font-size:11pt;max-width:165mm}.cover{overflow:hidden}.cover:before{content:"";position:absolute;width:100mm;height:100mm;border-radius:50%;right:-35mm;top:-30mm;background:linear-gradient(145deg,#08799f,#11afba)}.cover:after{content:"";position:absolute;width:54mm;height:54mm;border-radius:50%;right:-14mm;top:20mm;border:12mm solid #e2f1f4}.cover .brand,.cover h1,.cover .intro,.cover-meta,.cover .status-panel{position:relative;z-index:1}.cover-meta{margin-top:17mm;border-top:1px solid var(--line);padding-top:6mm;display:grid;grid-template-columns:1.4fr 1fr;gap:4mm 10mm}.cover-meta strong{display:block}.cover-meta span{display:block;color:var(--muted);font-size:8pt;margin-bottom:1mm}.status-panel{margin-top:17mm;padding:8mm;border-radius:5mm;background:${model.publishable ? '#eaf8f2' : '#fff0f0'};border-left:2mm solid ${model.publishable ? 'var(--green)' : 'var(--red)'}}.status-panel strong{display:block;font-size:18pt;color:${model.publishable ? 'var(--green)' : 'var(--red)'};margin-bottom:2mm}.kpi-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:4mm;margin-top:5mm}.kpi{padding:5mm;border:1px solid var(--line);border-radius:4mm}.kpi strong{font-size:21pt;display:block}.kpi span{color:var(--muted);font-size:8.5pt}.note{margin-top:6mm;padding:4mm 5mm;border-radius:3mm;background:#eef7fa;color:#315468;font-size:8.5pt}.section-head{display:flex;justify-content:space-between;align-items:flex-end;border-bottom:1px solid var(--line);padding-bottom:4mm;margin-bottom:7mm}.section-head .brand{font-size:8.5pt}.metrics{display:grid;grid-template-columns:repeat(3,1fr);gap:3mm}.metric{padding:5mm;border-radius:4mm;background:var(--soft)}.metric strong{font-size:18pt;display:block}.metric span{font-size:8pt;color:var(--muted)}.ranges{display:grid;grid-template-columns:1fr 1fr;gap:4mm;margin:5mm 0}.ranges div{padding:4mm;border:1px solid var(--line);border-radius:3mm}.ranges span{display:block;color:var(--muted);font-size:8pt}.scores-main{display:grid;grid-template-columns:1fr 1fr;gap:5mm;margin:5mm 0}.score{display:flex;align-items:center;gap:4mm;padding:4mm;border:1px solid var(--line);border-radius:4mm}.score strong,.score small{display:block}.score small{color:var(--muted)}.score-ring{--score:0;width:24mm;height:24mm;border-radius:50%;display:grid;place-items:center;background:conic-gradient(var(--blue) calc(var(--score)*1%),#e8edf2 0);position:relative}.score-ring:after{content:"";position:absolute;inset:3.3mm;border-radius:50%;background:white}.score-ring span{position:relative;z-index:1;font-weight:850}.score-grid{display:grid;grid-template-columns:repeat(4,1fr);gap:3mm}.score-grid .score{display:block;text-align:center}.score-grid .score-ring{width:19mm;height:19mm;margin:0 auto 2mm}.finding{border:1px solid var(--line);border-radius:4mm;padding:4mm;margin:0 0 3mm;break-inside:avoid}.finding header{display:grid;grid-template-columns:auto 1fr auto;align-items:start;gap:4mm}.priority{display:grid;place-items:center;width:12mm;height:12mm;border-radius:3mm;color:white;font-weight:900}.priority-p0{background:var(--red)}.priority-p1{background:var(--amber)}.priority-p2{background:var(--blue)}.finding-count{text-align:right;font-size:16pt}.finding-count small{display:block;color:var(--muted);font-size:7pt;font-weight:500}.finding-grid{display:grid;grid-template-columns:1fr 1fr 1.2fr;gap:4mm;margin-top:3mm;padding-top:3mm;border-top:1px solid var(--line)}.finding-grid p{font-size:8.2pt}.finding footer{color:var(--muted);font-size:7.8pt;display:flex;gap:2mm}.coverage{display:grid;grid-template-columns:1fr 1fr;gap:4mm}.coverage-card{padding:5mm;border-radius:4mm;background:var(--soft);border-top:1.5mm solid var(--blue)}.coverage-card:nth-child(2){border-color:var(--cyan)}.coverage-card:nth-child(3){border-color:var(--green)}.coverage-card:nth-child(4){border-color:var(--violet)}.coverage-card strong{font-size:16pt}.coverage-card span{display:block;color:var(--muted);font-size:8.5pt}.insights{margin-top:7mm}.insights li{margin-bottom:3mm;padding-left:2mm}.appendix-grid{display:grid;grid-template-columns:1fr 1fr;gap:7mm}.bar-row{display:grid;grid-template-columns:26mm 1fr 16mm;gap:3mm;align-items:center;margin:2.5mm 0;font-size:8.5pt}.bar-row div{height:2.5mm;background:#e8edf2;border-radius:2mm;overflow:hidden}.bar-row i{display:block;height:100%;background:linear-gradient(90deg,var(--blue),var(--cyan));border-radius:2mm}.bar-row strong{text-align:right}.footer-note{margin-top:8mm;padding-top:4mm;border-top:1px solid var(--line);font-size:8pt;color:var(--muted)}.empty{padding:10mm;background:var(--soft);border-radius:4mm}.capped{color:#8b5a00;background:#fff8df;padding:3mm;border-radius:3mm;font-size:8pt;margin-top:4mm}@page{size:A4;margin:0}@media print{body{background:white}.page{margin:0;box-shadow:none;page-break-after:always}.page:last-child{page-break-after:auto}}@media screen and (max-width:850px){.page{width:100%;min-height:auto;margin:0;padding:24px}.cover:before,.cover:after{display:none}.finding-grid,.appendix-grid{grid-template-columns:1fr}.score-grid{grid-template-columns:1fr 1fr}}
    *{-webkit-print-color-adjust:exact!important;print-color-adjust:exact!important}
    .page{overflow:hidden;border-top:2.5mm solid var(--blue)}
    .section-head{align-items:center;border:0;border-left:2mm solid var(--blue);padding:3mm 4mm;background:linear-gradient(90deg,#edf8fb 0,#fff 72%);border-radius:0 3mm 3mm 0}
    .kpi:nth-child(1){border-top:1.5mm solid var(--red);background:#fff5f5}.kpi:nth-child(2){border-top:1.5mm solid var(--blue);background:#f1f8fb}.kpi:nth-child(3){border-top:1.5mm solid var(--violet);background:#f7f4fb}
    .metric{border:1px solid var(--line);border-left:1.5mm solid var(--blue);background:#f4f9fb}.metric:nth-child(2){border-left-color:var(--cyan)}.metric:nth-child(3){border-left-color:var(--violet)}.metric:nth-child(4){border-left-color:var(--amber)}.metric:nth-child(5){border-left-color:var(--green)}.metric:nth-child(6){border-left-color:var(--red)}
    .score{--tone:var(--blue);--tone-bg:#f1f8fb;border:1px solid var(--line);border-top:1.5mm solid var(--tone);background:var(--tone-bg)}
    .score-ring{background:conic-gradient(var(--tone) calc(var(--score)*1%),#e6ebf0 0)}
    .score-green{--tone:var(--green);--tone-bg:#effaf5}.score-blue{--tone:var(--blue);--tone-bg:#f1f8fb}.score-cyan{--tone:var(--cyan);--tone-bg:#effafa}.score-amber{--tone:var(--amber);--tone-bg:#fff8e8}.score-violet{--tone:var(--violet);--tone-bg:#f7f4fb}.score-red{--tone:var(--red);--tone-bg:#fff3f3}
    .finding{--priority:var(--blue);--priority-bg:#f3f8fb;border:1px solid var(--line);border-left:2mm solid var(--priority);background:var(--priority-bg);box-shadow:none}
    .finding-p0{--priority:var(--red);--priority-bg:#fff6f6}.finding-p1{--priority:var(--amber);--priority-bg:#fffbef}.finding-p2{--priority:var(--blue);--priority-bg:#f3f8fb}
    .finding-grid{grid-template-columns:1fr 1fr;gap:3mm}.finding-grid section{padding:3mm;border-radius:2.5mm;background:#fff;border:1px solid #e3e9ef}.finding-grid .finding-evidence{grid-column:1/-1;background:#f7f9fb}.finding footer{padding-top:2mm;color:#445268}
    .coverage-card{border:1px solid var(--line);border-left:2mm solid var(--blue);border-top:0;background:#f4f9fb}.coverage-card:nth-child(2){border-left-color:var(--cyan);background:#f1fafa}.coverage-card:nth-child(3){border-left-color:var(--green);background:#f1faf5}.coverage-card:nth-child(4){border-left-color:var(--violet);background:#f7f4fb}
    .plan-cards{display:grid;grid-template-columns:1fr;gap:3mm;margin-top:5mm}.phase-card{padding:4mm 5mm;border:1px solid var(--line);border-left:2mm solid var(--blue);border-radius:3mm;background:#f6f9fb}.phase-card strong{display:block;font-size:11pt;margin-bottom:2mm}.phase-card span{display:block;color:var(--muted);font-size:7pt;font-weight:800;letter-spacing:.08em;text-transform:uppercase}.phase-card p{margin:1mm 0 2mm;font-size:8.5pt}.phase-p0{border-left-color:var(--red);background:#fff5f5}.phase-p1{border-left-color:var(--amber);background:#fffaed}.phase-p2{border-left-color:var(--blue);background:#f2f8fb}
    .bar-row i{background:var(--blue)}.bar-critical i{background:var(--red)}.bar-high i{background:#ed6a32}.bar-medium i{background:var(--amber)}.bar-low i{background:var(--green)}.bar-info i{background:var(--blue)}.bar-spec i{background:var(--blue)}.bar-interop i{background:var(--cyan)}.bar-quality i{background:var(--violet)}.bar-analytics i{background:var(--amber)}
    .rule-list{display:grid;grid-template-columns:1fr 1fr;gap:3mm;margin-top:3mm}.rule-item{display:grid;grid-template-columns:1fr auto;gap:2mm;padding:3mm;border:1px solid var(--line);border-left:1.5mm solid var(--violet);border-radius:3mm;background:#faf8fd;break-inside:avoid}.rule-item div{min-width:0}.rule-item code{display:inline-block;color:var(--violet);font-weight:900;margin-right:2mm}.rule-item div strong{font-size:8pt}.rule-item p{grid-column:1/-1;margin:0;display:flex;gap:2mm}.rule-item p span{font-size:7pt;color:#5a4a76;background:#eee9f7;padding:.5mm 1.5mm;border-radius:99px}.rule-item b{font-size:11pt;color:var(--violet)}
    .file-list{display:grid;grid-template-columns:1fr 1fr;gap:3mm}.file-list article{display:flex;justify-content:space-between;align-items:center;gap:4mm;padding:4mm;border:1px solid var(--line);border-left:1.5mm solid var(--cyan);border-radius:3mm;background:#f2fafa}.file-list code{font-size:8pt;color:#155a66}.file-list strong{white-space:nowrap;color:var(--blue)}.file-list small{font-weight:500;color:var(--muted)}
    @page{size:A4;margin:0}
    @media print{html,body{width:210mm;margin:0!important;background:#fff!important}body{font-size:10.5pt}.page{width:210mm;height:297mm;min-height:297mm;max-height:297mm;margin:0!important;box-shadow:none!important;break-after:page;page-break-after:always;overflow:hidden}.page:last-child{break-after:auto;page-break-after:auto}}
    @media screen and (max-width:850px){.rule-list,.file-list{grid-template-columns:1fr}}
    * { box-sizing: border-box; -webkit-print-color-adjust: exact !important; print-color-adjust: exact !important; }
    .page { overflow: hidden; border-top: 2.5mm solid var(--blue); }
    .print-action { position: fixed; z-index: 20; top: 16px; right: 18px; padding: 10px 16px; border: 0; border-radius: 9px; background: var(--blue); color: #fff; box-shadow: 0 5px 18px #12354b40; font: 700 14px system-ui, sans-serif; cursor: pointer; }
    .status-panel { margin-top: 17mm; padding: 8mm; border-radius: 5mm; background: ${model.publishable ? '#eaf8f2' : '#fff0f0'}; border-left: 2mm solid ${model.publishable ? 'var(--green)' : 'var(--red)'}; }
    .status-panel strong { display: block; margin-bottom: 2mm; color: ${model.publishable ? 'var(--green)' : 'var(--red)'}; font-size: 18pt; }
    .kpi-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 4mm; margin-top: 5mm; }
    .kpi { padding: 5mm; border: 1px solid var(--line); border-radius: 4mm; }
    .kpi strong { display: block; font-size: 21pt; }
    .kpi span { color: var(--muted); font-size: 8.5pt; }
    .kpi:nth-child(1) { border-top: 1.5mm solid var(--red); background: #fff5f5; }
    .kpi:nth-child(2) { border-top: 1.5mm solid var(--blue); background: #f1f8fb; }
    .kpi:nth-child(3) { border-top: 1.5mm solid var(--violet); background: #f7f4fb; }
    .note { margin-top: 6mm; padding: 4mm 5mm; border: 1px solid #c8e2ea; border-radius: 3mm; background: #eef7fa; color: #315468; font-size: 8.5pt; }
    .section-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 7mm; padding: 3mm 4mm; border: 0; border-left: 2mm solid var(--blue); border-radius: 0 3mm 3mm 0; background: linear-gradient(90deg, #edf8fb 0, #fff 72%); }
    .section-head .brand { flex-shrink: 0; font-size: 8.5pt; }
    .metrics { display: grid; grid-template-columns: repeat(3, 1fr); gap: 3mm; }
    .metric { padding: 5mm; border: 1px solid var(--line); border-left: 1.5mm solid var(--blue); border-radius: 4mm; background: #f4f9fb; }
    .metric strong { display: block; font-size: 18pt; }
    .metric span { color: var(--muted); font-size: 8pt; }
    .metric:nth-child(2) { border-left-color: var(--cyan); }
    .metric:nth-child(3) { border-left-color: var(--violet); }
    .metric:nth-child(4) { border-left-color: var(--amber); }
    .metric:nth-child(5) { border-left-color: var(--green); }
    .metric:nth-child(6) { border-left-color: var(--red); }
    .ranges { display: grid; grid-template-columns: 1fr 1fr; gap: 4mm; margin: 5mm 0; }
    .ranges div { padding: 4mm; border: 1px solid var(--line); border-radius: 3mm; background: #fff; }
    .ranges span { display: block; color: var(--muted); font-size: 8pt; }
    .scores-main { display: grid; grid-template-columns: 1fr 1fr; gap: 5mm; margin: 5mm 0; }
    .score-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 3mm; }
    .score { --tone: var(--blue); --tone-bg: #f1f8fb; display: flex; align-items: center; gap: 4mm; padding: 4mm; border: 1px solid var(--line); border-top: 1.5mm solid var(--tone); border-radius: 4mm; background: var(--tone-bg); }
    .score strong, .score small { display: block; }
    .score small { color: var(--muted); }
    .score-ring { width: 24mm; height: 24mm; position: relative; display: grid; place-items: center; flex-shrink: 0; border-radius: 50%; background: conic-gradient(var(--tone) calc(var(--score) * 1%), #e6ebf0 0); }
    .score-ring::after { content: ""; position: absolute; inset: 3.3mm; border-radius: 50%; background: #fff; }
    .score-ring span { position: relative; z-index: 1; font-weight: 850; }
    .score-grid .score { display: block; text-align: center; }
    .score-grid .score-ring { width: 19mm; height: 19mm; margin: 0 auto 2mm; }
    .score-green { --tone: var(--green); --tone-bg: #effaf5; }
    .score-blue { --tone: var(--blue); --tone-bg: #f1f8fb; }
    .score-cyan { --tone: var(--cyan); --tone-bg: #effafa; }
    .score-amber { --tone: var(--amber); --tone-bg: #fff8e8; }
    .score-violet { --tone: var(--violet); --tone-bg: #f7f4fb; }
    .score-red { --tone: var(--red); --tone-bg: #fff3f3; }
    .finding { --priority: var(--blue); --priority-bg: #f3f8fb; margin: 0 0 4mm; padding: 5mm; border: 1px solid var(--line); border-left: 2mm solid var(--priority); border-radius: 4mm; background: var(--priority-bg); break-inside: avoid; }
    .finding-p0 { --priority: var(--red); --priority-bg: #fff6f6; }
    .finding-p1 { --priority: var(--amber); --priority-bg: #fffbef; }
    .finding-p2 { --priority: var(--blue); --priority-bg: #f3f8fb; }
    .finding header { display: grid; grid-template-columns: auto 1fr auto; align-items: start; gap: 4mm; }
    .priority { display: grid; place-items: center; width: 12mm; height: 12mm; border-radius: 3mm; background: var(--priority); color: #fff; font-weight: 900; }
    .finding-count { color: var(--priority); font-size: 16pt; text-align: right; }
    .finding-count small { display: block; color: var(--muted); font-size: 7pt; font-weight: 500; }
    .finding-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 3mm; margin-top: 3mm; padding-top: 3mm; border-top: 1px solid var(--line); }
    .finding-grid section { padding: 3mm; border: 1px solid #e3e9ef; border-radius: 2.5mm; background: #fff; }
    .finding-grid .finding-evidence { grid-column: 1 / -1; background: #f7f9fb; }
    .finding-grid p { margin: 0; font-size: 8.2pt; }
    .finding footer { display: flex; gap: 2mm; padding-top: 2mm; color: #445268; font-size: 7.8pt; }
    .coverage { display: grid; grid-template-columns: 1fr 1fr; gap: 4mm; }
    .coverage-card { padding: 5mm; border: 1px solid var(--line); border-left: 2mm solid var(--blue); border-radius: 4mm; background: #f4f9fb; }
    .coverage-card:nth-child(2) { border-left-color: var(--cyan); background: #f1fafa; }
    .coverage-card:nth-child(3) { border-left-color: var(--green); background: #f1faf5; }
    .coverage-card:nth-child(4) { border-left-color: var(--violet); background: #f7f4fb; }
    .coverage-card strong { display: block; font-size: 16pt; }
    .coverage-card span { color: var(--muted); font-size: 8.5pt; }
    .insights { margin-top: 7mm; }
    .insights li { margin-bottom: 3mm; padding-left: 2mm; }
    .plan-cards { display: grid; grid-template-columns: 1fr; gap: 3mm; margin-top: 5mm; }
    .phase-card { padding: 4mm 5mm; border: 1px solid var(--line); border-left: 2mm solid var(--blue); border-radius: 3mm; background: #f6f9fb; }
    .phase-card strong { display: block; margin-bottom: 2mm; font-size: 11pt; }
    .phase-card span { display: block; color: var(--muted); font-size: 7pt; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }
    .phase-card p { margin: 1mm 0 2mm; font-size: 8.5pt; }
    .phase-p0 { border-left-color: var(--red); background: #fff5f5; }
    .phase-p1 { border-left-color: var(--amber); background: #fffaed; }
    .phase-p2 { border-left-color: var(--blue); background: #f2f8fb; }
    .appendix-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 7mm; }
    .bar-row { display: grid; grid-template-columns: 26mm 1fr 16mm; align-items: center; gap: 3mm; margin: 2.5mm 0; font-size: 8.5pt; }
    .bar-row div { height: 2.5mm; overflow: hidden; border-radius: 2mm; background: #e8edf2; }
    .bar-row i { display: block; height: 100%; border-radius: 2mm; background: var(--blue); }
    .bar-row strong { text-align: right; }
    .bar-critical i { background: var(--red); } .bar-high i { background: #ed6a32; } .bar-medium i { background: var(--amber); } .bar-low i { background: var(--green); }
    .bar-spec i { background: var(--blue); } .bar-interop i { background: var(--cyan); } .bar-quality i { background: var(--violet); } .bar-analytics i { background: var(--amber); }
    .rule-list { display: grid; grid-template-columns: 1fr 1fr; gap: 3mm; margin-top: 3mm; }
    .rule-item { display: grid; grid-template-columns: 1fr auto; gap: 2mm; padding: 3mm; border: 1px solid var(--line); border-left: 1.5mm solid var(--violet); border-radius: 3mm; background: #faf8fd; break-inside: avoid; }
    .rule-item div { min-width: 0; } .rule-item code { display: inline-block; margin-right: 2mm; color: var(--violet); font-weight: 900; } .rule-item div strong { font-size: 8pt; }
    .rule-item p { grid-column: 1 / -1; display: flex; gap: 2mm; margin: 0; } .rule-item p span { padding: .5mm 1.5mm; border-radius: 99px; background: #eee9f7; color: #5a4a76; font-size: 7pt; }
    .rule-item b { color: var(--violet); font-size: 11pt; }
    .file-list { display: grid; grid-template-columns: 1fr 1fr; gap: 3mm; }
    .file-list article { display: flex; align-items: center; justify-content: space-between; gap: 4mm; padding: 4mm; border: 1px solid var(--line); border-left: 1.5mm solid var(--cyan); border-radius: 3mm; background: #f2fafa; }
    .file-list code { color: #155a66; font-size: 8pt; } .file-list strong { color: var(--blue); white-space: nowrap; } .file-list small { color: var(--muted); font-weight: 500; }
    .footer-note { margin-top: 8mm; padding-top: 4mm; border-top: 1px solid var(--line); color: var(--muted); font-size: 8pt; }
    .empty { padding: 10mm; border-radius: 4mm; background: var(--soft); }
    .capped { margin-top: 4mm; padding: 3mm; border: 1px solid #ead58d; border-radius: 3mm; background: #fff8df; color: #8b5a00; font-size: 8pt; }
    @page { size: A4; margin: 0; }
    @media print {
      html, body { width: 210mm; margin: 0 !important; background: #fff !important; }
      .page { width: 210mm; height: 297mm; min-height: 297mm; max-height: 297mm; margin: 0 !important; overflow: hidden; box-shadow: none !important; break-after: page; page-break-after: always; }
      .page:last-child { break-after: auto; page-break-after: auto; }
      .print-action { display: none !important; }
    }
    @media screen and (max-width: 850px) { body { min-width: 210mm; } }
  </style>
</head>
<body>
  <button class="print-action" type="button" onclick="window.print()">${e(copy.printReport)}</button>
  <section class="page cover">
    <div class="brand"><span class="brand-mark">GA</span>GTFS Analyzer</div>
    <p class="eyebrow" style="margin-top:24mm">${e(copy.confidential)}</p>
    <h1>${e(copy.reportTitle)}</h1>
    <p class="intro">${e(copy.reportSubtitle)}</p>
    <div class="cover-meta">
      <div><span>GTFS feed</span><strong>${e(model.fileName)}</strong></div>
      <div><span>${e(copy.generated)}</span><strong>${e(model.generatedAt)}</strong></div>
      <div><span>${e(copy.version)}</span><strong>${e(model.appVersion)}</strong></div>
      <div><span>${e(copy.engine)}</span><strong>${e(model.engineMode || copy.unknown)}</strong></div>
    </div>
    <div class="status-panel"><strong>${e(model.publishable ? copy.ready : copy.blocked)}</strong><p>${e(model.publishable ? copy.summaryReady : copy.summaryBlocked)}</p></div>
    <div class="page-number">01</div>
  </section>

  <section class="page">
    <div class="section-head"><div><p class="eyebrow">01</p><h2>${e(copy.executiveSummary)}</h2></div><div class="brand"><span class="brand-mark">GA</span>GTFS Analyzer</div></div>
    <div class="status-panel" style="margin-top:0"><strong>${e(model.publishable ? copy.ready : copy.blocked)}</strong><p>${e(model.publishable ? copy.summaryReady : copy.summaryBlocked)}</p></div>
    <div class="kpi-grid">
      <div class="kpi"><strong>${n(model.totals.actual)}</strong><span>${e(copy.actualFindings)}</span></div>
      <div class="kpi"><strong>${n(model.totals.ruleTypes)}</strong><span>${e(copy.ruleTypes)}</span></div>
      <div class="kpi"><strong>${n(model.findings.length)}</strong><span>${e(copy.selectedActions)}</span></div>
    </div>
    <div class="note">${e(copy.scoreNote)}</div>
    <h2 style="margin-top:10mm">${e(copy.feedInsights)}</h2>
    ${model.insights.length ? `<ul class="insights">${model.insights.map(insight => `<li>${e(insight)}</li>`).join('')}</ul>` : `<p class="empty">${e(copy.noInsight)}</p>`}
    <div class="page-number">02</div>
  </section>

  <section class="page">
    <div class="section-head"><div><p class="eyebrow">02</p><h2>${e(copy.feedProfile)}</h2><p class="muted">${e(copy.feedProfileIntro)}</p></div><div class="brand"><span class="brand-mark">GA</span>GTFS Analyzer</div></div>
    <div class="metrics">
      ${metric(copy.stops, n(model.profile.stops))}${metric(copy.routes, n(model.profile.routes))}${metric(copy.trips, n(model.profile.trips))}
      ${metric(copy.shapes, n(model.profile.shapes))}${metric(copy.activeDays, n(model.profile.activeServiceDays))}${metric(copy.dailyTrips, d(model.profile.averageDailyTrips))}
    </div>
    <div class="ranges"><div><span>${e(copy.feedRange)}</span><strong>${e(model.profile.feedRange)}</strong></div><div><span>${e(copy.serviceRange)}</span><strong>${e(model.profile.serviceRange)}</strong></div></div>
    <div class="scores-main">${score(copy.publishScore, model.scores.publish, 'green')}${score(copy.overallScore, model.scores.overall, 'amber')}</div>
    <div class="score-grid">${score(copy.spec, model.scores.spec, 'blue')}${score(copy.interop, model.scores.interop, 'cyan')}${score(copy.quality, model.scores.quality, 'violet')}${score(copy.analytics, model.scores.analytics, 'red')}</div>
    <div class="note">${e(copy.scoreNote)}</div>
    <div class="page-number">03</div>
  </section>

  ${findingPages}

  <section class="page">
    <div class="section-head"><div><p class="eyebrow">04</p><h2>${e(copy.diagnosticCoverage)}</h2><p class="muted">${e(copy.coverageIntro)}</p></div><div class="brand"><span class="brand-mark">GA</span>GTFS Analyzer</div></div>
    <div class="coverage">${model.totals.byClass.map(row => `<div class="coverage-card"><strong>${n(row.count)}</strong><span>${e(row.label)}</span></div>`).join('')}</div>
    <h2 style="margin-top:10mm">${e(copy.remediationPlan)}</h2><p class="muted">${e(copy.remediationIntro)}</p>
    <div class="plan-cards">
      <article class="phase-card phase-p0"><strong>${e(copy.phase0)}</strong><span>${e(copy.objective)}</span><p>${e(copy.phase0Obj)}</p><span>${e(copy.exit)}</span><p>${e(copy.phase0Exit)}</p></article>
      <article class="phase-card phase-p1"><strong>${e(copy.phase1)}</strong><span>${e(copy.objective)}</span><p>${e(copy.phase1Obj)}</p><span>${e(copy.exit)}</span><p>${e(copy.phase1Exit)}</p></article>
      <article class="phase-card phase-p2"><strong>${e(copy.phase2)}</strong><span>${e(copy.objective)}</span><p>${e(copy.phase2Obj)}</p><span>${e(copy.exit)}</span><p>${e(copy.phase2Exit)}</p></article>
    </div>
    <div class="page-number">${pageNumber(coveragePage)}</div>
  </section>

  <section class="page">
    <div class="section-head"><div><p class="eyebrow">05</p><h2>${e(copy.appendix)}</h2><p class="muted">${e(copy.appendixIntro)}</p></div><div class="brand"><span class="brand-mark">GA</span>GTFS Analyzer</div></div>
    <div class="appendix-grid"><section><h3>${e(copy.severityDistribution)}</h3>${distribution(model.totals.bySeverity)}</section><section><h3>${e(copy.classDistribution)}</h3>${distribution(model.totals.byClass)}</section></div>
    ${model.totals.capped ? `<p class="capped">${e(replace(copy.cappedNote, { displayed: n(model.totals.displayed), actual: n(model.totals.actual) }))}</p>` : ''}
    <h3 style="margin-top:7mm">${e(copy.topRules)}</h3>
    <div class="rule-list">${model.topRules.map(row => `<article class="rule-item"><div><code>${e(row.ruleId)}</code><strong>${e(row.title)}</strong></div><p><span>${e(row.severity)}</span><span>${e(row.classLabel)}</span></p><b>${n(row.count)}</b></article>`).join('')}</div>
    <div class="page-number">${pageNumber(appendixPage)}</div>
  </section>

  <section class="page">
    <div class="section-head"><div><p class="eyebrow">06</p><h2>${e(copy.fileInventory)} · ${n(model.profile.files)}</h2><p class="muted">${e(copy.appendixIntro)}</p></div><div class="brand"><span class="brand-mark">GA</span>GTFS Analyzer</div></div>
    <div class="file-list">${model.fileStats.map(file => `<article><code>${e(file.name)}</code><strong>${n(file.rows)} <small>${e(copy.rows)}</small></strong></article>`).join('')}</div>
    <p class="footer-note">${e(copy.offline)}</p>
    <div class="page-number">${pageNumber(inventoryPage)}</div>
  </section>
</body>
</html>`;
}
