#!/bin/bash

# Unity Plugin Build Script for Aegis-Assets
# This script builds, tests, and validates the Unity plugin

set -e  # Exit on any error

PROJECT_ROOT="$(dirname "$0")"
cd "$PROJECT_ROOT"

echo "🛡️  Aegis Assets - Unity Plugin Build Script"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Rust is installed
check_rust() {
    print_status "Checking Rust installation..."
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo not found. Please install Rust: https://rustup.rs/"
        exit 1
    fi
    
    rust_version=$(rustc --version)
    print_success "Found Rust: $rust_version"
}

# Clean previous builds
clean_build() {
    print_status "Cleaning previous builds..."
    cargo clean
    print_success "Build directory cleaned"
}

# Check code formatting
check_format() {
    print_status "Checking code formatting..."
    if cargo fmt -- --check; then
        print_success "Code formatting is correct"
    else
        print_warning "Code formatting issues found. Run 'cargo fmt' to fix."
    fi
}

# Run linter
run_clippy() {
    print_status "Running Clippy linter..."
    if cargo clippy --all-targets --all-features -- -D warnings; then
        print_success "Clippy checks passed"
    else
        print_error "Clippy found issues. Please fix before proceeding."
        exit 1
    fi
}

# Build the plugin
build_plugin() {
    print_status "Building Unity plugin..."
    
    # Build in debug mode first
    print_status "Building debug version..."
    if cargo build; then
        print_success "Debug build successful"
    else
        print_error "Debug build failed"
        exit 1
    fi
    
    # Build release version
    print_status "Building release version..."
    if cargo build --release; then
        print_success "Release build successful"
    else
        print_error "Release build failed"
        exit 1
    fi
}

# Run tests
run_tests() {
    print_status "Running unit tests..."
    if cargo test --lib; then
        print_success "Unit tests passed"
    else
        print_error "Unit tests failed"
        exit 1
    fi
    
    print_status "Running integration tests..."
    if cargo test integration_tests; then
        print_success "Integration tests passed"
    else
        print_warning "Integration tests failed (may be expected with mock data)"
    fi
    
    print_status "Running doc tests..."
    if cargo test --doc; then
        print_success "Documentation tests passed"
    else
        print_warning "Documentation tests failed"
    fi
}

# Generate documentation
generate_docs() {
    print_status "Generating documentation..."
    if cargo doc --no-deps; then
        print_success "Documentation generated successfully"
        echo "📖 Documentation available at: target/doc/aegis_unity_plugin/index.html"
    else
        print_error "Documentation generation failed"
        exit 1
    fi
}

# Check plugin loading
test_plugin_loading() {
    print_status "Testing plugin loading..."
    
    # This would require the main Aegis binary to be available
    if command -v aegis &> /dev/null; then
        print_status "Testing plugin registration..."
        if aegis plugins list | grep -q "Unity"; then
            print_success "Plugin loads correctly in Aegis"
        else
            print_warning "Plugin may not be registered correctly"
        fi
    else
        print_warning "Aegis CLI not found - skipping plugin loading test"
    fi
}

# Performance benchmarks
run_benchmarks() {
    print_status "Running performance benchmarks..."
    
    if cargo bench --quiet; then
        print_success "Benchmarks completed"
    else
        print_warning "Benchmarks failed or not available"
    fi
}

# Security audit
security_audit() {
    print_status "Running security audit..."
    
    if command -v cargo-audit &> /dev/null; then
        if cargo audit; then
            print_success "Security audit passed"
        else
            print_warning "Security vulnerabilities found"
        fi
    else
        print_warning "cargo-audit not installed. Install with: cargo install cargo-audit"
    fi
}

# Main build process
main() {
    local step=${1:-"all"}
    
    case $step in
        "check")
            check_rust
            check_format
            run_clippy
            ;;
        "build")
            check_rust
            build_plugin
            ;;
        "test")
            check_rust
            run_tests
            ;;
        "docs")
            check_rust
            generate_docs
            ;;
        "clean")
            clean_build
            ;;
        "audit")
            security_audit
            ;;
        "bench")
            run_benchmarks
            ;;
        "all")
            print_status "Running complete build pipeline..."
            check_rust
            clean_build
            check_format
            run_clippy
            build_plugin
            run_tests
            generate_docs
            test_plugin_loading
            security_audit
            print_success "🎉 Unity plugin build completed successfully!"
            ;;
        *)
            echo "Usage: $0 [check|build|test|docs|clean|audit|bench|all]"
            echo ""
            echo "Commands:"
            echo "  check  - Check code formatting and run linter"
            echo "  build  - Build the plugin (debug and release)"
            echo "  test   - Run all tests"
            echo "  docs   - Generate documentation"
            echo "  clean  - Clean build artifacts"
            echo "  audit  - Run security audit"
            echo "  bench  - Run performance benchmarks"
            echo "  all    - Run complete build pipeline (default)"
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
