#!/bin/sh
# Copy this example into a fresh git repository and verify it.
# Usage: sh examples/data-migration/run-example.sh [path/to/countercheck]
set -e
COUNTERCHECK=${1:-countercheck}
HERE=$(cd "$(dirname "$0")" && pwd)
DIR=$(mktemp -d)
cp -R "$HERE/." "$DIR/"
rm -f "$DIR/run-example.sh"
cd "$DIR"
git init -q -b main
git -c user.email=example@example.com -c user.name=example add -A
git -c user.email=example@example.com -c user.name=example commit -qm "base: legacy data, checks, first conversion"
echo "example repository: $DIR"
"$COUNTERCHECK" verify --sandbox none --claim "Migration done, all tenants reconcile." || true
