import * as core from '@actions/core';
import * as github from '@actions/github';

import { createOrUpdateComment } from './comment.js';
import { compareBenchmarks } from './criterion.js';
import {
  buildFullComment,
  formatBenchmarkTable,
  formatBinarySizeTable,
  formatRssTable,
  formatTestCountTable,
} from './format.js';

const RADIX_DECIMAL = 10;
const DECIMAL_PLACES_SHORT = 1;
const DECIMAL_PLACES_LONG = 2;
const INITIAL_MAX_REGRESSION = 0;

export const run = async (): Promise<void> => {
  try {
    const criterionDirectory = core.getInput('criterion-dir', { required: true });
    const prBaseline = core.getInput('pr-baseline');
    const baseBaseline = core.getInput('base-baseline');
    const prBinarySizes: Record<string, number> = JSON.parse(core.getInput('pr-binary-sizes'));
    const baseBinarySizes: Record<string, number> = JSON.parse(core.getInput('base-binary-sizes'));
    const prTestCount = parseInt(core.getInput('pr-test-count'), RADIX_DECIMAL);
    const baseTestCount = parseInt(core.getInput('base-test-count'), RADIX_DECIMAL);
    const prPeakRssKilobytes = parseInt(core.getInput('pr-peak-rss-kb'), RADIX_DECIMAL);
    const basePeakRssKilobytes = parseInt(core.getInput('base-peak-rss-kb'), RADIX_DECIMAL);
    const threshold = parseFloat(core.getInput('threshold'));
    const token = core.getInput('token');

    core.info(`Comparing criterion baselines: ${baseBaseline} vs ${prBaseline}`);
    core.info(`Criterion directory: ${criterionDirectory}`);
    core.info(`Threshold: ${threshold}%`);

    const benchmarkResults = await compareBenchmarks(criterionDirectory, baseBaseline, prBaseline);

    core.info(`Found ${benchmarkResults.length} benchmark(s)`);
    for (const result of benchmarkResults) {
      core.info(`  ${result.name}: ${result.diffPercentage.toFixed(DECIMAL_PLACES_LONG)}%`);
    }

    const maxRegression = benchmarkResults.reduce(
      (max, result) => Math.max(max, result.diffPercentage),
      INITIAL_MAX_REGRESSION,
    );
    const hasRegression = maxRegression > threshold;

    const benchmarkTable = formatBenchmarkTable(benchmarkResults, threshold);
    const sizeTable = formatBinarySizeTable(prBinarySizes, baseBinarySizes);
    const rssTable = formatRssTable(prPeakRssKilobytes, basePeakRssKilobytes);
    const testTable = formatTestCountTable(prTestCount, baseTestCount);

    const commentBody = buildFullComment(benchmarkTable, sizeTable, rssTable, testTable, threshold, maxRegression);

    await core.summary.addRaw(commentBody).write();
    core.info('Job summary written');

    const pullRequestNumber = github.context.payload.pull_request?.number;

    if (pullRequestNumber && token) {
      const octokit = github.getOctokit(token);
      const { owner, repo } = github.context.repo;

      const commentId = await createOrUpdateComment(octokit, owner, repo, pullRequestNumber, commentBody);

      core.info(`PR comment posted/updated: ${commentId}`);
      core.setOutput('comment-id', commentId.toString());
    } else {
      core.warning('No PR number or token — skipping PR comment');
    }

    core.setOutput('has-regression', hasRegression.toString());
    core.setOutput('max-regression', maxRegression.toFixed(DECIMAL_PLACES_LONG));

    if (hasRegression) {
      core.setFailed(
        `Benchmark regression of ${maxRegression.toFixed(DECIMAL_PLACES_SHORT)}% exceeds threshold of ${threshold}%`,
      );
    }
  } catch (error) {
    if (error instanceof Error) {
      core.setFailed(error.message);
    } else {
      core.setFailed('An unexpected error occurred');
    }
  }
};
