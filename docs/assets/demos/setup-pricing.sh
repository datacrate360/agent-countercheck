#!/bin/sh
# Demo project for verify.tape: a pricing function with a bug, a test that catches it, and an "agent" that
# skipped the failing test instead of fixing the code. Creates /tmp/countercheck-demo (replaced if it exists).
set -e
D=/tmp/countercheck-demo
rm -rf "$D"
mkdir -p "$D/tests" "$D/.countercheck"
cd "$D"
git init -q -b main
cat > pricing.py <<'PY'
def apply_discount(price, percent):
    return price - price * percent / 100
PY
touch tests/__init__.py
cat > tests/test_pricing.py <<'PY'
import unittest
from pricing import apply_discount


class TestPricing(unittest.TestCase):
    def test_half_off(self):
        self.assertEqual(apply_discount(10, 50), 5)

    def test_never_below_zero(self):
        self.assertEqual(apply_discount(10, 150), 0)
PY
cat > .countercheck/checks.toml <<'TOML'
protected = ["tests/**"]

[[check]]
name = "unit tests"
run = "python3 -m unittest -q"
TOML
printf '.countercheck/receipts/\n' > .gitignore
git add -A
git -c user.name=demo -c user.email=demo@example.com commit -qm "pricing with a bug; the test that catches it"
# The real fix, applied later in the demo with `git apply ../countercheck-demo-fix.patch`.
cat > /tmp/countercheck-demo-fix.patch <<'PATCH'
--- a/pricing.py
+++ b/pricing.py
@@ -1,2 +1,2 @@
 def apply_discount(price, percent):
-    return price - price * percent / 100
+    return max(0, price - price * percent / 100)
PATCH
# What the agent did: skipped the failing test.
python3 - <<'PY'
p = "tests/test_pricing.py"
s = open(p).read().replace("    def test_never_below_zero", "    @unittest.skip(\"flaky\")\n    def test_never_below_zero")
open(p, "w").write(s)
PY
