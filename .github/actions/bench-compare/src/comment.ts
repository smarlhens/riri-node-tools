import type * as github from '@actions/github';

const COMMENT_TAG = '<!-- bench-compare -->';

type Octokit = ReturnType<typeof github.getOctokit>;

const findExistingComment = async (
  octokit: Octokit,
  owner: string,
  repository: string,
  pullRequestNumber: number,
): Promise<number | null> => {
  const comments = await octokit.paginate(octokit.rest.issues.listComments, {
    issue_number: pullRequestNumber,
    owner,
    per_page: 100,
    repo: repository,
  });

  const existing = comments.find((comment: { body?: string | null }) => comment.body?.includes(COMMENT_TAG) ?? false);
  return existing?.id ?? null;
};

export const createOrUpdateComment = async (
  octokit: Octokit,
  owner: string,
  repository: string,
  pullRequestNumber: number,
  body: string,
): Promise<number> => {
  const taggedBody = `${body}\n${COMMENT_TAG}`;
  const existingId = await findExistingComment(octokit, owner, repository, pullRequestNumber);

  if (existingId) {
    await octokit.rest.issues.updateComment({
      body: taggedBody,
      comment_id: existingId,
      owner,
      repo: repository,
    });
    return existingId;
  }

  const { data: comment } = await octokit.rest.issues.createComment({
    body: taggedBody,
    issue_number: pullRequestNumber,
    owner,
    repo: repository,
  });
  return comment.id;
};
