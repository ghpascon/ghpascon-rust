#!/usr/bin/env bash
set -e

cd "$(dirname "$0")/.."

echo "==> Running formatter..."
cargo fmt --check

echo "==> Running tests..."
cargo test

echo ""
echo "All checks passed."
echo ""

# --- bump version ---
current=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $current"

IFS='.' read -r major minor patch <<< "$current"

echo ""
echo "Bump type?"
echo "  1) patch  ($major.$minor.$((patch + 1)))"
echo "  2) minor  ($major.$((minor + 1)).0)"
echo "  3) major  ($((major + 1)).0.0)"
read -rp "Choice [1/2/3]: " bump_choice

case "$bump_choice" in
    1) new_version="$major.$minor.$((patch + 1))"; bump_label="patch" ;;
    2) new_version="$major.$((minor + 1)).0";      bump_label="minor" ;;
    3) new_version="$((major + 1)).0.0";           bump_label="major" ;;
    *) echo "Invalid choice."; exit 1 ;;
esac

echo ""
echo "New version: $new_version"

# update Cargo.toml
sed -i "s/^version = \"$current\"/version = \"$new_version\"/" Cargo.toml

# regenerate Cargo.lock
cargo check -q

# --- commit ---
echo ""
read -rp "Commit message: " commit_msg

git add .
git commit -m "$bump_label(v$new_version): $commit_msg"
git tag "v$new_version"
git push && git push --tags

echo ""
echo "==> Released v$new_version"
