import { readFile } from 'node:fs/promises';
import { ValidationError, validateGtfs } from 'gtfs-sdk';

const filePath = process.argv[2] ?? './feed.zip';
const zipBytes = await readFile(filePath);

try {
  const result = await validateGtfs(zipBytes, {
    today: process.env.GTFS_TODAY || undefined,
  });

  const highPriority = result.notices.filter((notice) =>
    notice.severity === 'CRITICAL' || notice.severity === 'HIGH',
  );

  console.log(JSON.stringify({
    status: result.validation_status,
    score: result.reports.r5.score,
    publishable: result.reports.r1.publishable,
    noticeCount: result.notices.length,
    highPriorityCount: highPriority.length,
    metrics: result.metrics,
    partial: result.partial ?? null,
  }, null, 2));
} catch (error) {
  if (error instanceof ValidationError) {
    console.error(`${error.code}: ${error.message}`);
  } else {
    console.error(error);
  }
  process.exitCode = 1;
}
