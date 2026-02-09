#!/bin/sh
# Setup git hooks for the rec project
# Run this after cloning: ./scripts/setup-hooks.sh

set -e

HOOKS_DIR=".git/hooks"
SCRIPT_DIR="$(dirname "$0")"

echo "Installing git hooks..."

# Pre-commit hook
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/bin/sh
set -e

echo "Running cargo fmt..."
cargo fmt

echo "Running cargo clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# Stage any formatting changes
git add -u
EOF

chmod +x "$HOOKS_DIR/pre-commit"

echo "Done! Git hooks installed."
