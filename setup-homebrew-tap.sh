#!/bin/bash
# Helper script to set up a Homebrew tap for sshdb

set -e

TAP_NAME="ruphy/sshdb"
TAP_REPO="homebrew-sshdb"
VERSION="0.15.0"

echo "🚀 Setting up Homebrew tap for sshdb"
echo ""

# Check if GitHub release exists
echo "📦 Step 1: Checking for GitHub release..."
if ! curl -s -o /dev/null -w "%{http_code}" "https://github.com/ruphy/sshdb/releases/tag/v${VERSION}" | grep -q "200\|404"; then
    echo "⚠️  Warning: GitHub release v${VERSION} may not exist yet"
    echo "   Create it at: https://github.com/ruphy/sshdb/releases/new"
fi

# Get SHA256
echo ""
echo "🔐 Step 2: Getting SHA256 hash..."
TARBALL_URL="https://github.com/ruphy/sshdb/archive/refs/tags/v${VERSION}.tar.gz"
SHA256=$(curl -sL "$TARBALL_URL" | shasum -a 256 | awk '{print $1}')

if [ -z "$SHA256" ] || [ "$SHA256" = "" ]; then
    echo "❌ Failed to get SHA256. Make sure the release exists."
    exit 1
fi

echo "   SHA256: $SHA256"

# Update formula
echo ""
echo "📝 Step 3: Updating formula with SHA256..."
sed -i '' "s/REPLACE_WITH_RELEASE_SHA256/$SHA256/" sshdb.rb
echo "   ✓ Formula updated"

# Create tap directory structure
echo ""
echo "📁 Step 4: Creating tap structure..."
TAP_DIR="$HOME/$TAP_REPO"
mkdir -p "$TAP_DIR/Formula"
cp sshdb.rb "$TAP_DIR/Formula/"

echo ""
echo "✅ Setup complete!"
echo ""
echo "Next steps:"
echo "1. Create a GitHub repository named '$TAP_REPO'"
echo "2. Run these commands:"
echo "   cd $TAP_DIR"
echo "   git init"
echo "   git add Formula/sshdb.rb"
echo "   git commit -m 'Add sshdb formula'"
echo "   git remote add origin https://github.com/$TAP_NAME.git"
echo "   git branch -M main"
echo "   git push -u origin main"
echo ""
echo "Then users can install with:"
echo "   brew tap $TAP_NAME"
echo "   brew install sshdb"

