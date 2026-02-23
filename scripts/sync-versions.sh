#!/usr/bin/env bash
set -euo pipefail

# Determine version from GITHUB_REF tag or workspace Cargo.toml
if [[ -n "${GITHUB_REF:-}" && "$GITHUB_REF" == refs/tags/v* ]]; then
  VERSION="${GITHUB_REF#refs/tags/v}"
else
  VERSION=$(grep -m1 'version = ' Cargo.toml | sed 's/.*version = "\(.*\)"/\1/')
fi

echo "Syncing npm package versions to ${VERSION}"

# Update all npm package.json files
for pkg in npm/husako/package.json npm/platform-*/package.json; do
  if [ -f "$pkg" ]; then
    # Use node to update version in-place
    node -e "
      const fs = require('fs');
      const pkg = JSON.parse(fs.readFileSync('$pkg', 'utf8'));
      pkg.version = '${VERSION}';
      if (pkg.optionalDependencies) {
        for (const key of Object.keys(pkg.optionalDependencies)) {
          pkg.optionalDependencies[key] = '${VERSION}';
        }
      }
      fs.writeFileSync('$pkg', JSON.stringify(pkg, null, 2) + '\n');
    "
    echo "  Updated $pkg"
  fi
done

echo "Done."
