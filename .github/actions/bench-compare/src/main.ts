import * as core from "@actions/core";
import * as github from "@actions/github";
import { compareBenchmarks } from "./criterion.js";
import { createOrUpdateComment } from "./comment.js";
import {
  buildFullComment,
  formatBenchmarkTable,
  formatBinarySizeTable,
  formatRssTable,
  formatTestCountTable,
} from "./format.js";

export async function run(): Promise<void> {
  try {
    const criterionDir = core.getInput("criterion-dir", { required: true });
    const prBaseline = core.getInput("pr-baseline");
    const baseBaseline = core.getInput("base-baseline");
    const prBinarySizes: Record<string, number> = JSON.parse(core.getInput("pr-binary-sizes"));
    const baseBinarySizes: Record<string, number> = JSON.parse(core.getInput("base-binary-sizes"));
    const prTestCount = parseInt(core.getInput("pr-test-count"), 10);
    const baseTestCount = parseInt(core.getInput("base-test-count"), 10);
    const prPeakRssKb = parseInt(core.getInput("pr-peak-rss-kb"), 10);
    const basePeakRssKb = parseInt(core.getInput("base-peak-rss-kb"), 10);
    const threshold = parseFloat(core.getInput("threshold"));
    const token = core.getInput("token");

    core.info(`Comparing criterion baselines: ${baseBaseline} vs ${prBaseline}`);
    core.info(`Criterion directory: ${criterionDir}`);
    core.info(`Threshold: ${threshold}%`);

    const benchResults = await compareBenchmarks(criterionDir, baseBaseline, prBaseline);

    core.info(`Found ${benchResults.length} benchmark(s)`);
    for (const r of benchResults) {
      core.info(`  ${r.name}: ${r.diffPct.toFixed(2)}%`);
    }

    const maxRegression = benchResults.reduce((max, r) => Math.max(max, r.diffPct), 0);
    const hasRegression = maxRegression > threshold;

    const benchmarkTable = formatBenchmarkTable(benchResults, threshold);
    const sizeTable = formatBinarySizeTable(prBinarySizes, baseBinarySizes);
    const rssTable = formatRssTable(prPeakRssKb, basePeakRssKb);
    const testTable = formatTestCountTable(prTestCount, baseTestCount);

    const commentBody = buildFullComment(benchmarkTable, sizeTable, rssTable, testTable, threshold, maxRegression);

    await core.summary.addRaw(commentBody).write();
    core.info("Job summary written");

    const prNumber = github.context.payload.pull_request?.number;

    if (prNumber && token) {
      const octokit = github.getOctokit(token);
      const { owner, repo } = github.context.repo;

      const commentId = await createOrUpdateComment(octokit, owner, repo, prNumber, commentBody);

      core.info(`PR comment posted/updated: ${commentId}`);
      core.setOutput("comment-id", commentId.toString());
    } else {
      core.warning("No PR number or token — skipping PR comment");
    }

    core.setOutput("has-regression", hasRegression.toString());
    core.setOutput("max-regression", maxRegression.toFixed(2));

    if (hasRegression) {
      core.setFailed(`Benchmark regression of ${maxRegression.toFixed(1)}% exceeds threshold of ${threshold}%`);
    }
  } catch (error) {
    if (error instanceof Error) {
      core.setFailed(error.message);
    } else {
      core.setFailed("An unexpected error occurred");
    }
  }
}
