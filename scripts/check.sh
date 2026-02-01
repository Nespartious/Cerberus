#!/usr/bin/env bash
# Cerberus Local Code Checks
# Run this before committing to catch issues early
# Usage: ./scripts/check.sh [--quick|--full]

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🐺 Cerberus Code Checks"
echo "========================"

MODE=${1:-"--quick"}

# Function to print status
check_pass() { echo -e "${GREEN}✓${NC} $1"; }
check_fail() { echo -e "${RED}✗${NC} $1"; exit 1; }
check_warn() { echo -e "${YELLOW}⚠${NC} $1"; }

# 1. Format check
echo ""
echo "📝 Checking code formatting..."
if cargo fmt --all -- --check 2>/dev/null; then
    check_pass "Code is formatted correctly"
else
    check_fail "Run 'cargo fmt --all' to fix formatting"
fi

# 2. Clippy (catches AI hallucinations and common bugs)
echo ""
echo "🔍 Running Clippy lints..."
CLIPPY_FLAGS="-D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::unimplemented"

if [ "$MODE" = "--full" ]; then
    CLIPPY_FLAGS="$CLIPPY_FLAGS -W clippy::pedantic -A clippy::module_name_repetitions -A clippy::must_use_candidate"
fi

if cargo clippy --all-targets --all-features -- $CLIPPY_FLAGS 2>/dev/null; then
    check_pass "Clippy checks passed"
else
    check_fail "Clippy found issues - fix them before committing"
fi

# 3. Build check
echo ""
echo "🔨 Checking debug build..."
if cargo build --all-targets 2>/dev/null; then
    check_pass "Debug build successful"
else
    check_fail "Debug build failed"
fi

# 4. Full mode extras
if [ "$MODE" = "--full" ]; then
    echo ""
    echo "🚀 Checking release build..."
    if cargo build --release 2>/dev/null; then
        check_pass "Release build successful"
    else
        check_fail "Release build failed"
    fi
    
    echo ""
    echo "📚 Checking documentation..."
    if RUSTDOCFLAGS="-D warnings" cargo doc --no-deps 2>/dev/null; then
        check_pass "Documentation builds correctly"
    else
        check_warn "Documentation has warnings"
    fi
    
    echo ""
    echo "🔒 Checking for unused dependencies..."
    if command -v cargo-machete &> /dev/null; then
        cargo machete || check_warn "Some dependencies may be unused"
    else
        check_warn "cargo-machete not installed, skipping"
    fi
fi

echo ""
echo "========================"
echo -e "${GREEN}All checks passed!${NC} ✨"
echo ""
