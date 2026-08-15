#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "${ROOT}"

if [ ! -f .fulmen/app.yaml ]; then
  echo "❌ Missing .fulmen/app.yaml" >&2
  exit 1
fi

BINARY_NAME=$(awk '
  $1 == "binary_name:" { print $2; exit }
' .fulmen/app.yaml)
ENV_PREFIX=$(awk '
  $1 == "env_prefix:" { print $2; exit }
' .fulmen/app.yaml)

if [ -z "${BINARY_NAME}" ] || [ -z "${ENV_PREFIX}" ]; then
  echo "❌ Could not read binary_name / env_prefix from .fulmen/app.yaml" >&2
  exit 1
fi

echo "🔍 Scanning cmd/ and internal/ for hardcoded identity strings (${BINARY_NAME}, ${ENV_PREFIX})..."
echo "   Allowed: .fulmen/app.yaml and internal/assets/appidentity/app.yaml"

SEARCH=$(command -v rg || true)
HITS=$(mktemp)
trap 'rm -f "${HITS}"' EXIT

scan() {
  local pattern=$1
  if [ -n "${SEARCH}" ]; then
    "${SEARCH}" -n --hidden \
      -g '!internal/assets/appidentity/**' \
      -g '!**/target/**' \
      -g '!**/.git/**' \
      "${pattern}" cmd internal \
      >>"${HITS}" 2>/dev/null || true
  else
    grep -RIn --exclude-dir=assets --exclude-dir=target --exclude-dir=.git \
      "${pattern}" cmd internal >>"${HITS}" 2>/dev/null || true
    # grep --exclude-dir=assets still walks internal/assets if nested; filter.
    if [ -s "${HITS}" ]; then
      grep -v 'internal/assets/appidentity/' "${HITS}" >"${HITS}.f" || true
      mv "${HITS}.f" "${HITS}"
    fi
  fi
}

scan "${BINARY_NAME}"
scan "${ENV_PREFIX}"

if [ -s "${HITS}" ]; then
  echo "❌ Found hardcoded identity references that should come from App Identity:"
  cat "${HITS}"
  echo
  echo "   ACTION: Load values from .fulmen/app.yaml (see internal/appid)"
  exit 1
fi

echo "✅ No hardcoded breed references found in cmd/ or internal/ sources"
