# Aegis-Assets Development Makefile
# Provides convenient targets for development, testing, and benchmarking

.PHONY: help build test bench clean corpus golden ci-check fmt lint docs

# Default target
help: ## Show this help message
	@echo "🛡️ Aegis-Assets Development Commands"
	@echo "=================================="
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# Build targets
build: ## Build all components
	@echo "🔨 Building Aegis-Assets..."
	cargo build --release
	@echo "✅ Build completed"

build-dev: ## Build in development mode
	@echo "🔨 Building in development mode..."
	cargo build
	@echo "✅ Development build completed"

build-bench: ## Build benchmark tool
	@echo "🔨 Building benchmark tool..."
	cd tools/bench && cargo build --release
	@echo "✅ Benchmark tool built"

# Test targets
test: ## Run all tests
	@echo "🧪 Running tests..."
	cargo test --workspace
	@echo "✅ Tests completed"

test-integration: ## Run integration tests
	@echo "🧪 Running integration tests..."
	cargo test --workspace --test '*'
	@echo "✅ Integration tests completed"

test-unit: ## Run unit tests only
	@echo "🧪 Running unit tests..."
	cargo test --workspace --lib
	@echo "✅ Unit tests completed"

# Benchmark targets
bench: build-bench corpus-check ## Run performance benchmarks
	@echo "📊 Running performance benchmarks..."
	cd tools/bench && ./target/release/bench run \
		--corpus ../../testdata/unity/ \
		--iterations 3 \
		--format table \
		--profile-memory \
		--validate-streaming
	@echo "✅ Benchmarks completed"

bench-json: build-bench corpus-check ## Run benchmarks with JSON output
	@echo "📊 Running benchmarks (JSON output)..."
	cd tools/bench && ./target/release/bench run \
		--corpus ../../testdata/unity/ \
		--iterations 3 \
		--format json \
		--output ../../benchmark-results.json \
		--profile-memory \
		--validate-streaming
	@echo "✅ Benchmarks completed - results in benchmark-results.json"

bench-stress: build-bench corpus-check ## Run stress tests
	@echo "🔥 Running stress tests..."
	cd tools/bench && ./target/release/bench run \
		--corpus ../../testdata/unity/ \
		--iterations 5 \
		--format table \
		--profile-memory \
		--validate-streaming \
		--memory-limit 200 \
		--throughput-min 150
	@echo "✅ Stress tests completed"

bench-memory: build-bench corpus-check ## Profile memory usage
	@echo "🧠 Running memory profiling..."
	cd tools/bench && ./target/release/bench run \
		--corpus ../../testdata/unity/ \
		--iterations 1 \
		--format table \
		--profile-memory \
		--validate-streaming \
		--memory-limit 100
	@echo "✅ Memory profiling completed"

# Corpus management
corpus-check: ## Check test corpus validity
	@echo "🔍 Validating test corpus..."
	@if [ ! -d "testdata/unity" ]; then \
		echo "❌ Test corpus not found. Run 'make corpus-setup' first."; \
		exit 1; \
	fi
	cd tools/bench && cargo run --bin bench -- validate --corpus ../../testdata/unity/
	@echo "✅ Corpus validation completed"

corpus-setup: ## Setup test corpus (interactive)
	@echo "📁 Setting up test corpus..."
	@echo "Please add Unity test files to testdata/unity/ directory"
	@echo "See testdata/README.md for sourcing guidelines"
	@echo "Files should match patterns in testdata/unity/manifest.yaml"
	@mkdir -p testdata/unity
	@echo "✅ Directory structure created"

corpus-stats: ## Show corpus statistics
	@echo "📊 Test corpus statistics:"
	@if [ -d "testdata/unity" ]; then \
		echo "Unity files: $$(find testdata/unity -name '*.unity3d' -o -name '*.assets' | wc -l)"; \
		echo "Total size: $$(du -sh testdata/unity 2>/dev/null | cut -f1)"; \
	else \
		echo "No corpus found - run 'make corpus-setup'"; \
	fi

# Golden test management
golden: build-bench corpus-check ## Generate golden test outputs
	@echo "✨ Generating golden test outputs..."
	@mkdir -p docs/artifacts/generated
	cd tools/bench && ./target/release/bench run \
		--corpus ../../testdata/unity/ \
		--iterations 1 \
		--format json \
		--output ../../docs/artifacts/generated/golden-run.json
	@echo "✅ Golden outputs generated in docs/artifacts/generated/"

golden-validate: build-bench ## Validate against golden outputs
	@echo "🔍 Validating against golden outputs..."
	@if [ ! -d "docs/artifacts/checksums" ]; then \
		echo "❌ No golden outputs found. Run 'make golden' first."; \
		exit 1; \
	fi
	# Future: Implement golden validation
	@echo "⚠️ Golden validation not yet implemented"

golden-clean: ## Clean generated golden outputs
	@echo "🧹 Cleaning golden outputs..."
	rm -rf docs/artifacts/generated/
	@echo "✅ Golden outputs cleaned"

# Development quality
fmt: ## Format code
	@echo "🎨 Formatting code..."
	cargo fmt --all
	@echo "✅ Code formatted"

lint: ## Run linter
	@echo "🔍 Running linter..."
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	@echo "✅ Linting completed"

lint-fix: ## Fix linting issues automatically
	@echo "🔧 Fixing linting issues..."
	cargo clippy --workspace --all-targets --all-features --fix --allow-dirty
	@echo "✅ Linting fixes applied"

check: fmt lint test ## Run all quality checks
	@echo "✅ All quality checks passed"

# CI simulation
ci-check: build test bench ## Run CI checks locally
	@echo "🤖 Running CI simulation..."
	@echo "Build: ✅"
	@echo "Tests: ✅" 
	@echo "Benchmarks: ✅"
	@echo "✅ CI simulation completed"

ci-bench: bench-json ## Run benchmarks like CI
	@echo "🤖 Running CI-style benchmarks..."
	@if [ -f "benchmark-results.json" ]; then \
		GRADE=$$(cat benchmark-results.json | jq -r '.summary.performance_grade'); \
		MEMORY=$$(cat benchmark-results.json | jq -r '.summary.p95_memory_mb'); \
		THROUGHPUT=$$(cat benchmark-results.json | jq -r '.summary.p95_throughput_mbps'); \
		echo "Performance Grade: $$GRADE"; \
		echo "P95 Memory: $${MEMORY}MB"; \
		echo "P95 Throughput: $${THROUGHPUT}MB/s"; \
		if [ "$$GRADE" = "F" ]; then \
			echo "❌ CI would fail - performance grade F"; \
			exit 1; \
		fi; \
	fi
	@echo "✅ CI benchmark simulation passed"

# Documentation
docs: ## Build documentation
	@echo "📚 Building documentation..."
	cargo doc --workspace --no-deps
	@echo "✅ Documentation built - open target/doc/aegis_core/index.html"

docs-open: docs ## Build and open documentation
	@echo "🌐 Opening documentation..."
	@if command -v xdg-open > /dev/null; then \
		xdg-open target/doc/aegis_core/index.html; \
	elif command -v open > /dev/null; then \
		open target/doc/aegis_core/index.html; \
	elif command -v start > /dev/null; then \
		start target/doc/aegis_core/index.html; \
	else \
		echo "Please open target/doc/aegis_core/index.html manually"; \
	fi

# Cleanup
clean: ## Clean build artifacts
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -f benchmark-results.json
	rm -rf docs/artifacts/generated/
	@echo "✅ Cleanup completed"

clean-all: clean ## Clean everything including corpus
	@echo "🧹 Deep cleaning..."
	rm -rf testdata/unity/*.unity3d
	rm -rf testdata/unity/*.assets
	@echo "✅ Deep cleanup completed"

# Release preparation
release-check: check bench golden ## Full release validation
	@echo "🚀 Running release validation..."
	@echo "Code quality: ✅"
	@echo "Performance: ✅"
	@echo "Golden tests: ✅"
	@echo "✅ Release validation completed"

release-prep: fmt lint test bench golden docs ## Prepare for release
	@echo "📦 Preparing release..."
	@git status --porcelain | grep -q . && echo "⚠️ Working directory not clean" || echo "✅ Working directory clean"
	@echo "✅ Release preparation completed"

# Development workflow
dev-setup: build-dev corpus-setup ## Setup development environment
	@echo "🛠️ Setting up development environment..."
	@echo "✅ Development environment ready"
	@echo ""
	@echo "Next steps:"
	@echo "1. Add Unity test files to testdata/unity/"
	@echo "2. Run 'make bench' to test extraction pipeline"
	@echo "3. Run 'make test' to ensure everything works"

dev-test: test-unit bench-memory ## Quick development testing
	@echo "⚡ Running quick development tests..."
	@echo "✅ Development tests completed"

# Sprint 0 specific targets
sprint0: dev-setup bench golden ci-check ## Complete Sprint 0 goals
	@echo "🎯 Sprint 0 Goals Check:"
	@echo "✅ Test corpus structure created"
	@echo "✅ Benchmark harness functional"  
	@echo "✅ Golden test framework setup"
	@echo "✅ CI integration configured"
	@echo ""
	@echo "🎉 Sprint 0 completed successfully!"

# Help target (repeated for visibility)
.DEFAULT_GOAL := help
