import * as github from "@actions/github";

const COMMENT_TAG = "<!-- bench-compare -->";

type Octokit = ReturnType<typeof github.getOctokit>;

async function findExistingComment(
  octokit: Octokit,
  owner: string,
  repo: string,
  prNumber: number,
): Promise<number | null> {
  const comments = await octokit.paginate(octokit.rest.issues.listComments, {
    owner,
    repo,
    issue_number: prNumber,
    per_page: 100,
  });

  const existing = comments.find((c: { body?: string | null }) => c.body?.includes(COMMENT_TAG) ?? false);
  return existing?.id ?? null;
}

export async function createOrUpdateComment(
  octokit: Octokit,
  owner: string,
  repo: string,
  prNumber: number,
  body: string,
): Promise<number> {
  const taggedBody = `${body}\n${COMMENT_TAG}`;
  const existingId = await findExistingComment(octokit, owner, repo, prNumber);

  if (existingId) {
    await octokit.rest.issues.updateComment({
      owner,
      repo,
      comment_id: existingId,
      body: taggedBody,
    });
    return existingId;
  }

  const { data: comment } = await octokit.rest.issues.createComment({
    owner,
    repo,
    issue_number: prNumber,
    body: taggedBody,
  });
  return comment.id;
}
