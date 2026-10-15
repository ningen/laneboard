#!/usr/bin/env bash
# Creates public GitHub repo "laneboard" and pushes main.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! gh auth status >/dev/null 2>&1; then
  echo "Not logged in. Run: gh auth login --hostname github.com --web"
  exit 1
fi

OWNER="$(gh api user -q .login)"
REPO="laneboard"
FULL="$OWNER/$REPO"

if gh repo view "$FULL" >/dev/null 2>&1; then
  echo "Repo already exists: https://github.com/$FULL"
else
  gh repo create "$REPO" --public --source=. --remote=github --push \
    --description "Laneboard — Tauri kanban for humans + CLI for agents (shared SQLite)"
  echo "Created https://github.com/$FULL"
fi

# Ensure github remote points at the public repo and push
if git remote get-url github >/dev/null 2>&1; then
  git remote set-url github "https://github.com/$FULL.git"
else
  git remote add github "https://github.com/$FULL.git"
fi

git push -u github main
echo "Pushed main → https://github.com/$FULL"
gh repo view "$FULL" --web || true