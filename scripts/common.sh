#!/usr/bin/env bash
# Shared packaging metadata. Source this from a Bash script with set -euo pipefail.
task_repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_version=$(cat "$task_repo_root/VERSION")
if [[ ! $task_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo 'VERSION must contain one stable version, such as 2.0.0.' >&2
  exit 1
fi
