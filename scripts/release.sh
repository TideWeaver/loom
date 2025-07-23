#!/bin/bash
# Helper script for creating releases

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Get the current version from Cargo.toml
CURRENT_VERSION=$(grep "^version" loom/Cargo.toml | head -1 | cut -d'"' -f2)

echo -e "${GREEN}Data-Loom Release Helper${NC}"
echo -e "Current version: ${YELLOW}$CURRENT_VERSION${NC}"
echo ""

# Check for uncommitted changes
if [[ -n $(git status -s) ]]; then
    echo -e "${RED}Error: You have uncommitted changes${NC}"
    echo "Please commit or stash your changes before creating a release"
    exit 1
fi

# Get the new version
read -p "Enter the new version (without 'v' prefix): " NEW_VERSION

if [[ -z "$NEW_VERSION" ]]; then
    echo -e "${RED}Error: Version cannot be empty${NC}"
    exit 1
fi

# Update version in Cargo.toml files
echo -e "${GREEN}Updating version to $NEW_VERSION...${NC}"

# Update library version
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" loom/Cargo.toml && rm loom/Cargo.toml.bak

# Update CLI version
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" loom-cli/Cargo.toml && rm loom-cli/Cargo.toml.bak

# Update Cargo.lock
cargo update -w

# Commit the version change
git add -A
git commit -m "chore: bump version to $NEW_VERSION"

# Create and push tag
TAG="v$NEW_VERSION"
echo -e "${GREEN}Creating tag $TAG...${NC}"
git tag -a "$TAG" -m "Release $TAG"

echo -e "${GREEN}Ready to release!${NC}"
echo ""
echo "To push the release, run:"
echo -e "  ${YELLOW}git push origin main${NC}"
echo -e "  ${YELLOW}git push origin $TAG${NC}"
echo ""
echo "GitHub Actions will automatically:"
echo "  - Build binaries for all platforms"
echo "  - Create a GitHub release"
echo "  - Upload artifacts with checksums"
echo ""
echo "For cargo-dist release, use tag: dist-v$NEW_VERSION"