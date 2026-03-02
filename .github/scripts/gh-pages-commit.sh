#!/usr/bin/env bash
# Shared commit-and-push for gh-pages deployments.
# Must be run from inside the gh-pages-deploy directory.
#
# Usage: gh-pages-commit.sh "Commit message"
set -e

COMMIT_MSG="${1:?Usage: gh-pages-commit.sh \"commit message\"}"

git add -A
if git diff --cached --quiet; then
  echo "No changes to deploy"
else
  git commit -m "$COMMIT_MSG"
  git push origin gh-pages
fi
