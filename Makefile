.PHONY: all help bootstrap bootstrap-force hooks-ensure tools sync dependencies verify-dependencies version-bump lint test build build-all clean fmt version check-all precommit prepush run install test-cov
.PHONY: sync-embedded-identity verify-embedded-identity test-standalone-binary
.PHONY: validate-app-identity doctor
.PHONY: version-set version-bump-major version-bump-minor version-bump-patch release-check release-prepare release-build

# Identity is sourced from .fulmen/app.yaml (the only hardcoded identity file).
BINARY_NAME := $(shell awk '$$1 == "binary_name:" { print $$2; exit }' .fulmen/app.yaml 2>/dev/null)
VERSION := $(shell cat VERSION 2>/dev/null || echo "dev")
COMMIT := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_DATE := $(shell date -u +"%Y-%m-%dT%H:%M:%SZ")

CARGO ?= cargo
CARGOFLAGS ?=

BINDIR ?=
BINDIR_RESOLVE = \
	BINDIR="$(BINDIR)"; \
	if [ -z "$$BINDIR" ]; then \
		OS_RAW="$$(uname -s 2>/dev/null || echo unknown)"; \
		case "$$OS_RAW" in \
			MINGW*|MSYS*|CYGWIN*) \
				if [ -n "$$USERPROFILE" ]; then \
					if command -v cygpath >/dev/null 2>&1; then \
						BINDIR="$$(cygpath -u "$$USERPROFILE")/bin"; \
					else \
						BINDIR="$$USERPROFILE/bin"; \
					fi; \
				elif [ -n "$$HOME" ]; then \
					BINDIR="$$HOME/bin"; \
				else \
					BINDIR="./bin"; \
				fi ;; \
			*) \
				if [ -n "$$HOME" ]; then \
					BINDIR="$$HOME/.local/bin"; \
				else \
					BINDIR="./bin"; \
				fi ;; \
		esac; \
	fi

GONEAT_VERSION ?= v0.5.16

SFETCH_RESOLVE = \
	$(BINDIR_RESOLVE); \
	SFETCH=""; \
	if [ -x "$$BINDIR/sfetch" ]; then SFETCH="$$BINDIR/sfetch"; fi; \
	if [ -z "$$SFETCH" ]; then SFETCH="$$(command -v sfetch 2>/dev/null || true)"; fi

GONEAT_RESOLVE = \
	$(BINDIR_RESOLVE); \
	GONEAT=""; \
	if [ -x "$$BINDIR/goneat" ]; then GONEAT="$$BINDIR/goneat"; fi; \
	if [ -z "$$GONEAT" ]; then GONEAT="$$(command -v goneat 2>/dev/null || true)"; fi; \
	if [ -z "$$GONEAT" ]; then echo "❌ goneat not found. Run 'make bootstrap' first."; exit 1; fi

GONEAT_OPTIONAL = \
	$(BINDIR_RESOLVE); \
	GONEAT=""; \
	if [ -x "$$BINDIR/goneat" ]; then GONEAT="$$BINDIR/goneat"; fi; \
	if [ -z "$$GONEAT" ]; then GONEAT="$$(command -v goneat 2>/dev/null || true)"; fi

all: fmt test

help: ## Show this help message
	@printf '%s\n' '$(BINARY_NAME) - Available Make Targets' '' \
		'Required targets (Makefile Standard):' \
		'  help                    - Show this help message' \
		'  bootstrap               - Install external tools (sfetch, goneat) and crate deps' \
		'  bootstrap-force         - Force reinstall external tools' \
		'  tools                   - Verify external tools are available' \
		'  dependencies            - Generate SBOM when goneat is installed' \
		'  lint                    - Run lint/format/style checks' \
		'  test                    - Run all tests' \
		'  build                   - Build the current-platform binary' \
		'  build-all               - Build a release binary' \
		'  clean                   - Remove build artifacts and caches' \
		'  fmt                     - Format Rust (and goneat format when available)' \
		'  version                 - Print current version' \
		'  version-set             - Set version to a specific value' \
		'  version-bump-major      - Bump major version' \
		'  version-bump-minor      - Bump minor version' \
		'  version-bump-patch      - Bump patch version' \
		'  release-check           - Run release checklist validation' \
		'  release-prepare         - Prepare for release' \
		'  release-build           - Build a release binary into dist/release' \
		'  check-all               - Run fmt, identity verify, lint, and test' \
		'  precommit               - Run pre-commit checks' \
		'  prepush                 - Run pre-push checks' \
		'' \
		'CDRL / identity:' \
		'  validate-app-identity   - Detect hardcoded breed references in source' \
		'  doctor                  - Validate CDRL refit completeness' \
		'  sync-embedded-identity  - Copy .fulmen/app.yaml into the embedded mirror' \
		'  verify-embedded-identity - Verify the embedded identity mirror' \
		'' \
		'Additional targets:' \
		'  run                     - Run the server in development mode' \
		'  test-cov                - Run tests with coverage (if cargo-llvm-cov is installed)' \
		'  test-standalone-binary  - Verify the binary runs outside the repo' \
		''

bootstrap: ## Install external tools (sfetch, goneat) and crate dependencies
	@echo "Installing external tools..."
	@$(SFETCH_RESOLVE); if [ -z "$$SFETCH" ]; then echo "❌ sfetch not found (required trust anchor)."; echo ""; echo "Install sfetch, verify it, then re-run bootstrap:"; echo "  curl -sSfL https://github.com/3leaps/sfetch/releases/latest/download/install-sfetch.sh | bash"; echo "  sfetch --self-verify"; echo ""; exit 1; fi
	@$(BINDIR_RESOLVE); mkdir -p "$$BINDIR"; echo "→ sfetch self-verify (trust anchor):"; $(SFETCH_RESOLVE); $$SFETCH --self-verify
	@$(BINDIR_RESOLVE); if [ "$(FORCE)" = "1" ] || [ "$(FORCE)" = "true" ]; then rm -f "$$BINDIR/goneat" "$$BINDIR/goneat.exe"; fi; if [ "$(FORCE)" = "1" ] || [ "$(FORCE)" = "true" ] || ! command -v goneat >/dev/null 2>&1; then echo "→ Installing goneat $(GONEAT_VERSION) to user bin dir..."; $(SFETCH_RESOLVE); $(BINDIR_RESOLVE); $$SFETCH --repo fulmenhq/goneat --tag $(GONEAT_VERSION) --dest-dir "$$BINDIR"; OS_RAW="$$(uname -s 2>/dev/null || echo unknown)"; case "$$OS_RAW" in MINGW*|MSYS*|CYGWIN*) if [ -f "$$BINDIR/goneat.exe" ] && [ ! -f "$$BINDIR/goneat" ]; then mv "$$BINDIR/goneat.exe" "$$BINDIR/goneat"; fi ;; esac; else echo "→ goneat already installed, skipping (use FORCE=1 to reinstall)"; fi; $(GONEAT_RESOLVE); echo "→ goneat: $$($$GONEAT --version 2>&1 | head -n1 || true)"; echo "→ Installing foundation tools via goneat doctor..."; $$GONEAT doctor tools --scope foundation --install --install-package-managers --yes --no-cooling
	@echo "→ Fetching Rust crate dependencies..."; $(CARGO) fetch
	@$(MAKE) hooks-ensure
	@$(BINDIR_RESOLVE); echo "✅ Bootstrap completed. Ensure $$BINDIR is on PATH"

bootstrap-force: ## Force reinstall external tools
	@$(MAKE) bootstrap FORCE=1

hooks-ensure: ## Ensure git hooks are installed (idempotent)
	@$(BINDIR_RESOLVE); \
	GONEAT=""; \
	if [ -x "$$BINDIR/goneat" ]; then GONEAT="$$BINDIR/goneat"; fi; \
	if [ -z "$$GONEAT" ]; then GONEAT="$$(command -v goneat 2>/dev/null || true)"; fi; \
	if [ -d ".git" ] && [ -n "$$GONEAT" ] && [ ! -x ".git/hooks/pre-commit" ]; then \
		echo "🔗 Installing git hooks with goneat..."; \
		$$GONEAT hooks install 2>/dev/null || true; \
	fi

tools: ## Verify external tools are available
	@echo "Verifying external tools..."
	@command -v rustc >/dev/null && rustc --version || (echo "❌ rustc missing"; exit 1)
	@command -v cargo >/dev/null && cargo --version || (echo "❌ cargo missing"; exit 1)
	@$(GONEAT_OPTIONAL); if [ -n "$$GONEAT" ]; then echo "✅ goneat: $$($$GONEAT --version 2>&1 | head -n1)"; else echo "ℹ️ goneat not installed (optional DX; run make bootstrap)"; fi
	@echo "✅ Toolchain verified"

sync: ## Helper-shim only; does not pull Crucible git
	@echo "⚠️ This workhorse does not consume Crucible git directly"
	@echo "→ SSOT assets are embedded in rsfulmen (helper shim)"
	@echo "✅ Sync target satisfied via rsfulmen shim (no-op git sync)"

dependencies: ## Generate SBOM for supply-chain security
	@echo "Generating Software Bill of Materials (SBOM)..."
	@$(GONEAT_OPTIONAL); \
	if [ -n "$$GONEAT" ]; then \
		mkdir -p sbom; \
		$$GONEAT dependencies --sbom --sbom-output sbom/$(BINARY_NAME).cdx.json; \
		echo "✅ SBOM generated at sbom/$(BINARY_NAME).cdx.json"; \
	else \
		echo "ℹ️ goneat not installed; skip SBOM (run make bootstrap)"; \
	fi

verify-dependencies: ## Alias for dependencies
	@$(MAKE) dependencies

install: ## Install dependencies (alias for bootstrap)
	@$(MAKE) bootstrap

run: ## Run server in development mode
	@$(CARGO) run --bin $(BINARY_NAME) -- serve --verbose

version-bump: ## Bump version (usage: make version-bump TYPE=patch|minor|major)
	@if [ -z "$(TYPE)" ]; then \
		echo "❌ TYPE not specified. Usage: make version-bump TYPE=patch|minor|major"; \
		exit 1; \
	fi
	@$(GONEAT_OPTIONAL); \
	if [ -n "$$GONEAT" ]; then \
		$$GONEAT version bump $(TYPE); \
	else \
		echo "❌ goneat not found; set VERSION with make version-set VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "✅ Version is $$(cat VERSION)"

version-set: ## Set version to specific value (usage: make version-set VERSION=x.y.z)
	@if [ -z "$(VERSION)" ]; then \
		echo "❌ VERSION not specified. Usage: make version-set VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "$(VERSION)" > VERSION
	@echo "✅ Version set to $(VERSION)"

version-bump-major: ## Bump major version
	@$(MAKE) version-bump TYPE=major

version-bump-minor: ## Bump minor version
	@$(MAKE) version-bump TYPE=minor

version-bump-patch: ## Bump patch version
	@$(MAKE) version-bump TYPE=patch

release-check: ## Run release checklist validation
	@$(MAKE) check-all
	@echo "✅ Release check passed"

release-prepare: ## Prepare for release
	@$(MAKE) check-all
	@echo "✅ Release preparation complete"

sync-embedded-identity: ## Sync embedded identity mirror from .fulmen/app.yaml
	@./scripts/sync-embedded-identity.sh

verify-embedded-identity: ## Verify embedded identity mirror is in sync
	@./scripts/verify-embedded-identity.sh

validate-app-identity: ## Detect hardcoded breed references
	@./scripts/validate-app-identity.sh

doctor: ## Validate CDRL refit completeness
	@./scripts/cdrl-doctor.sh

release-build: sync-embedded-identity ## Build a release binary into dist/release
	@echo "→ Building release artifact for $(BINARY_NAME) v$(VERSION)..."
	@mkdir -p dist/release
	@$(CARGO) build --release --bin $(BINARY_NAME)
	@cp target/release/$(BINARY_NAME) dist/release/$(BINARY_NAME)
	@echo "✅ Release build complete: dist/release/$(BINARY_NAME)"

build: sync-embedded-identity ## Build binary for current platform
	@echo "→ Building $(BINARY_NAME) v$(VERSION)..."
	@mkdir -p bin
	@$(CARGO) build --bin $(BINARY_NAME) $(CARGOFLAGS)
	@cp target/debug/$(BINARY_NAME) bin/$(BINARY_NAME)
	@echo "✓ Binary built: bin/$(BINARY_NAME)"

test-standalone-binary: build ## Verify built binary runs outside repo
	@echo "→ Standalone binary check (outside repo)..."
	@cp "bin/$(BINARY_NAME)" "/tmp/$(BINARY_NAME)"
	@"/tmp/$(BINARY_NAME)" version >/dev/null
	@"/tmp/$(BINARY_NAME)" --help >/dev/null
	@echo "✅ Standalone binary check passed"

build-all: ## Build a release binary (single host triple)
	@echo "→ Building release binary..."
	@mkdir -p bin
	@$(CARGO) build --release --bin $(BINARY_NAME)
	@cp target/release/$(BINARY_NAME) bin/$(BINARY_NAME)
	@echo "✓ Release binary built in bin/"

version: ## Print current version
	@echo "$(VERSION)"

test: sync-embedded-identity ## Run all tests
	@echo "Running test suite..."
	@$(CARGO) test --workspace --all-targets

test-cov: ## Run tests with coverage when cargo-llvm-cov is installed
	@if command -v cargo-llvm-cov >/dev/null 2>&1; then \
		$(CARGO) llvm-cov --workspace --html --output-dir coverage; \
		echo "✓ Coverage report: coverage/html/index.html"; \
	else \
		echo "ℹ️ cargo-llvm-cov not installed; running make test"; \
		$(MAKE) test; \
	fi

lint: ## Run lint checks
	@echo "Running rustfmt check..."
	@$(CARGO) fmt --all -- --check
	@echo "Running clippy..."
	@$(CARGO) clippy --workspace --all-targets -- -D warnings
	@$(GONEAT_OPTIONAL); if [ -n "$$GONEAT" ]; then echo "Running goneat assess..."; $$GONEAT assess --categories lint; else echo "ℹ️ goneat not installed; skipped goneat assess"; fi
	@echo "✅ Lint checks passed"

fmt: ## Format Rust sources (and goneat format when available)
	@echo "Formatting with rustfmt..."
	@$(CARGO) fmt --all
	@$(GONEAT_OPTIONAL); if [ -n "$$GONEAT" ]; then echo "Formatting with goneat..."; $$GONEAT format; fi
	@$(MAKE) sync-embedded-identity
	@echo "✅ Formatting completed"

check-all: fmt verify-embedded-identity validate-app-identity lint test ## Run all quality checks
	@echo "✅ All quality checks passed"

precommit: ## Run pre-commit validation
	@$(MAKE) fmt
	@$(MAKE) lint
	@echo "✅ Pre-commit checks passed"

prepush: ## Run pre-push validation
	@$(MAKE) check-all
	@echo "✅ Pre-push checks passed"

clean: ## Clean build artifacts and reports
	@echo "Cleaning artifacts..."
	@$(CARGO) clean
	rm -rf bin/ dist/ coverage/ coverage.out coverage.html
	@echo "✅ Clean completed"
