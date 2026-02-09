#!/bin/bash
# Generate code coverage reports using cargo-llvm-cov
#
# Usage:
#   ./scripts/coverage.sh          # Generate summary
#   ./scripts/coverage.sh html     # Generate HTML report and open
#   ./scripts/coverage.sh lcov     # Generate lcov.info for CI
#   ./scripts/coverage.sh json     # Generate JSON report

set -euo pipefail

COMMAND="${1:-summary}"

# Prevent interactive prompts
export CARGO_LLVM_COV_SETUP=no
export CI=true

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if cargo-llvm-cov is installed
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo -e "${RED}Error: cargo-llvm-cov is not installed${NC}"
    echo "Install with: cargo install cargo-llvm-cov"
    exit 1
fi

# Common options
# - Exclude test files from coverage report
# - Include all features (including TUI)
COMMON_OPTS="--all-features --workspace"
IGNORE_REGEX="tests/|/tests\.rs$"

case "$COMMAND" in
    summary)
        echo -e "${GREEN}Generating coverage summary...${NC}"
        cargo llvm-cov $COMMON_OPTS --ignore-filename-regex "$IGNORE_REGEX"
        ;;
    html)
        echo -e "${GREEN}Generating HTML coverage report...${NC}"
        cargo llvm-cov $COMMON_OPTS --ignore-filename-regex "$IGNORE_REGEX" --open
        echo -e "${GREEN}Report opened in browser${NC}"
        ;;
    lcov)
        echo -e "${GREEN}Generating lcov.info...${NC}"
        cargo llvm-cov $COMMON_OPTS --ignore-filename-regex "$IGNORE_REGEX" --lcov --output-path lcov.info
        echo -e "${GREEN}Coverage report written to lcov.info${NC}"
        ;;
    json)
        echo -e "${GREEN}Generating JSON coverage report...${NC}"
        cargo llvm-cov $COMMON_OPTS --ignore-filename-regex "$IGNORE_REGEX" --json --output-path coverage.json
        echo -e "${GREEN}Coverage report written to coverage.json${NC}"
        ;;
    clean)
        echo -e "${YELLOW}Cleaning coverage data...${NC}"
        cargo llvm-cov clean --workspace
        echo -e "${GREEN}Coverage data cleaned${NC}"
        ;;
    *)
        echo "Usage: $0 {summary|html|lcov|json|clean}"
        echo ""
        echo "Commands:"
        echo "  summary  - Display coverage summary in terminal (default)"
        echo "  html     - Generate HTML report and open in browser"
        echo "  lcov     - Generate lcov.info for CI/Codecov"
        echo "  json     - Generate JSON coverage report"
        echo "  clean    - Clean previous coverage data"
        exit 1
        ;;
esac
