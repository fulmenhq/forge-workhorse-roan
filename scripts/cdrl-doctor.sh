#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "${ROOT}"

errors=0
warnings=0

echo "🏥 Running CDRL completeness check..."

if [ ! -f .fulmen/app.yaml ]; then
  echo "❌ App Identity: missing .fulmen/app.yaml"
  echo "   ACTION: Restore .fulmen/app.yaml from the template"
  errors=$((errors + 1))
else
  missing=0
  for field in vendor binary_name env_prefix config_name; do
    if ! grep -q "^[[:space:]]*${field}:" .fulmen/app.yaml; then
      echo "❌ App Identity: missing field ${field}"
      missing=1
    fi
  done
  if [ "${missing}" -eq 0 ]; then
    echo "✅ App Identity: Valid (.fulmen/app.yaml)"
  else
    errors=$((errors + 1))
  fi
fi

BINARY_NAME=$(awk '$1 == "binary_name:" { print $2; exit }' .fulmen/app.yaml 2>/dev/null || true)
ENV_PREFIX=$(awk '$1 == "env_prefix:" { print $2; exit }' .fulmen/app.yaml 2>/dev/null || true)
CONFIG_NAME=$(awk '$1 == "config_name:" { print $2; exit }' .fulmen/app.yaml 2>/dev/null || true)

if [ -n "${ENV_PREFIX}" ]; then
  echo "✅ Environment Variables: prefix ${ENV_PREFIX}"
  if [ -f .env ]; then
    if grep -E '^[A-Z0-9_]+=' .env | grep -vq "^${ENV_PREFIX}"; then
      echo "⚠️  Environment Variables: .env contains names that do not use ${ENV_PREFIX}"
      echo "   ACTION: Rename variables in .env to use the App Identity prefix"
      warnings=$((warnings + 1))
    fi
  fi
fi

if [ -n "${CONFIG_NAME}" ]; then
  if [ -d "config/${CONFIG_NAME}" ]; then
    echo "✅ Configuration Paths: Match identity (config_name: ${CONFIG_NAME})"
  else
    echo "⚠️  Configuration Paths: config/${CONFIG_NAME}/ not found"
    echo "   ACTION: mv config/<old> config/${CONFIG_NAME}"
    warnings=$((warnings + 1))
  fi
  if [ -d "schemas/${CONFIG_NAME}" ]; then
    echo "✅ Schema Paths: Match identity (schemas/${CONFIG_NAME})"
  else
    echo "⚠️  Schema Paths: schemas/${CONFIG_NAME}/ not found"
    echo "   ACTION: mv schemas/<old> schemas/${CONFIG_NAME}"
    warnings=$((warnings + 1))
  fi
fi

if [ -f Cargo.toml ]; then
  PKG=$(awk -F'"' '/^name = / { print $2; exit }' Cargo.toml)
  BIN=$(awk -F'"' '/^name = / { n=$2 } END { print n }' Cargo.toml)
  if [ -n "${BINARY_NAME}" ] && [ "${PKG}" = "${BINARY_NAME}" ]; then
    echo "✅ Module Path: Cargo package name matches binary_name (${PKG})"
  else
    echo "⚠️  Module Path: Cargo.toml package '${PKG}' does not match binary_name '${BINARY_NAME}'"
    echo "   ACTION: Update Cargo.toml package and [[bin]] name during refit"
    warnings=$((warnings + 1))
  fi
  if [ -n "${BIN}" ] && [ -n "${BINARY_NAME}" ] && [ "${BIN}" != "${BINARY_NAME}" ]; then
    echo "⚠️  Binary name in Cargo.toml (${BIN}) does not match binary_name (${BINARY_NAME})"
    warnings=$((warnings + 1))
  fi
else
  echo "❌ Module Path: Cargo.toml missing"
  errors=$((errors + 1))
fi

if [ ! -f internal/assets/appidentity/app.yaml ]; then
  echo "❌ Embedded identity mirror missing"
  echo "   ACTION: make sync-embedded-identity"
  errors=$((errors + 1))
elif ! cmp -s .fulmen/app.yaml internal/assets/appidentity/app.yaml; then
  echo "❌ Embedded identity mirror is out of sync"
  echo "   ACTION: make sync-embedded-identity"
  errors=$((errors + 1))
else
  echo "✅ Embedded identity: in sync"
fi

echo
if [ "${errors}" -gt 0 ]; then
  echo "🛑 CDRL refit incomplete - fix errors above"
  exit 2
fi
if [ "${warnings}" -gt 0 ]; then
  echo "⚠️  Doctor completed with warnings"
  exit 1
fi
echo "🎉 CDRL checks passed"
exit 0
