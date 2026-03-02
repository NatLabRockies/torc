#!/usr/bin/env bash
# Shared setup for gh-pages deployments.
# Configures git and clones (or initializes) the gh-pages branch into ./gh-pages-deploy.
#
# Required environment variables:
#   GITHUB_TOKEN  — access token for pushing to the repository
#   GITHUB_REPOSITORY — owner/repo (e.g., NatLabRockies/torc)
set -e

REPO_URL="https://x-access-token:${GITHUB_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"

git config --global user.name "github-actions[bot]"
git config --global user.email "github-actions[bot]@users.noreply.github.com"

git clone --depth 1 --branch gh-pages "$REPO_URL" gh-pages-deploy 2>/dev/null || {
  # Branch doesn't exist yet — create it
  mkdir gh-pages-deploy
  cd gh-pages-deploy
  git init
  git checkout -b gh-pages
  git remote add origin "$REPO_URL"
  cd ..
}

cd gh-pages-deploy
touch .nojekyll
