#!/usr/bin/env bash
# Release script for rec
# Usage: ./scripts/release.sh <version>
# Example: ./scripts/release.sh 0.1.0

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Check if version argument is provided
if [[ $# -ne 1 ]]; then
    echo -e "${RED}Error: Version argument required${NC}"
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.0"
    exit 1
fi

VERSION="$1"

# Validate version format (semver)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo -e "${RED}Error: Invalid version format '$VERSION'${NC}"
    echo "Version must be in semver format: X.Y.Z (e.g., 0.1.0, 1.0.0)"
    exit 1
fi

# Check for uncommitted changes
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo -e "${RED}Error: Uncommitted changes detected${NC}"
    echo "Please commit or stash your changes before releasing."
    exit 1
fi

# Check if we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [[ "$CURRENT_BRANCH" != "main" ]]; then
    echo -e "${YELLOW}Warning: Not on main branch (current: $CURRENT_BRANCH)${NC}"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check if tag already exists
if git rev-parse "$VERSION" >/dev/null 2>&1; then
    echo -e "${RED}Error: Tag '$VERSION' already exists${NC}"
    exit 1
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo -e "${GREEN}Current version: $CURRENT_VERSION${NC}"
echo -e "${GREEN}New version: $VERSION${NC}"
echo ""

# Get the last tag or use initial commit
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || git rev-list --max-parents=0 HEAD)

# Generate changelog from conventional commits
generate_changelog() {
    local from="$1"
    local version="$2"
    local date=$(date +%Y-%m-%d)
    
    echo "## [$version] - $date"
    echo ""
    
    # Get commits grouped by type
    local has_content=false
    
    # Features
    local feats=$(git log "$from"..HEAD --pretty=format:"%s" | grep -E "^feat(\(.+\))?:" | sed 's/^feat\(([^)]*)\)\?: /- /' | sed 's/^feat: /- /')
    if [[ -n "$feats" ]]; then
        echo "### Added"
        echo "$feats"
        echo ""
        has_content=true
    fi
    
    # Fixes
    local fixes=$(git log "$from"..HEAD --pretty=format:"%s" | grep -E "^fix(\(.+\))?:" | sed 's/^fix\(([^)]*)\)\?: /- /' | sed 's/^fix: /- /')
    if [[ -n "$fixes" ]]; then
        echo "### Fixed"
        echo "$fixes"
        echo ""
        has_content=true
    fi
    
    # Performance
    local perfs=$(git log "$from"..HEAD --pretty=format:"%s" | grep -E "^perf(\(.+\))?:" | sed 's/^perf\(([^)]*)\)\?: /- /' | sed 's/^perf: /- /')
    if [[ -n "$perfs" ]]; then
        echo "### Performance"
        echo "$perfs"
        echo ""
        has_content=true
    fi
    
    # Refactor
    local refactors=$(git log "$from"..HEAD --pretty=format:"%s" | grep -E "^refactor(\(.+\))?:" | sed 's/^refactor\(([^)]*)\)\?: /- /' | sed 's/^refactor: /- /')
    if [[ -n "$refactors" ]]; then
        echo "### Changed"
        echo "$refactors"
        echo ""
        has_content=true
    fi
    
    # Docs
    local docs=$(git log "$from"..HEAD --pretty=format:"%s" | grep -E "^docs?(\(.+\))?:" | sed 's/^docs\?\(([^)]*)\)\?: /- /' | sed 's/^docs\?: /- /')
    if [[ -n "$docs" ]]; then
        echo "### Documentation"
        echo "$docs"
        echo ""
        has_content=true
    fi
    
    if [[ "$has_content" == false ]]; then
        echo "### Changed"
        echo "- Initial release"
        echo ""
    fi
}

# Show what will be in changelog
echo -e "${YELLOW}Changes to be included in this release:${NC}"
generate_changelog "$LAST_TAG" "$VERSION"
echo ""

# Confirm release
read -p "Release version $VERSION? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

echo ""
echo -e "${YELLOW}Updating version in Cargo.toml...${NC}"
sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$VERSION\"/" Cargo.toml

echo -e "${YELLOW}Updating Cargo.lock...${NC}"
cargo check --quiet 2>/dev/null || true

echo -e "${YELLOW}Generating CHANGELOG.md...${NC}"
# Create new changelog content
{
    echo "# Changelog"
    echo ""
    echo "All notable changes to this project will be documented in this file."
    echo ""
    echo "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),"
    echo "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)."
    echo ""
    generate_changelog "$LAST_TAG" "$VERSION"
} > CHANGELOG.md

echo -e "${YELLOW}Creating release commit...${NC}"
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release $VERSION"

echo -e "${YELLOW}Creating tag...${NC}"
git tag "$VERSION" -m "Release $VERSION"

echo -e "${YELLOW}Pushing to origin...${NC}"
git push origin main
git push origin "$VERSION"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Released version $VERSION successfully!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "GitHub Actions will now build binaries and create the release."
echo "Check: https://github.com/zeybek/rec/actions"
echo ""

# Publish to crates.io
read -p "Publish to crates.io? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Publishing to crates.io...${NC}"
    cargo publish
    echo -e "${GREEN}Published to crates.io!${NC}"
fi
