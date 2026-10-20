# Records docs/assets/demos/verify.gif with the installed countercheck: vhs docs/assets/demos/verify.tape
Output docs/assets/demos/verify.gif
Require countercheck
Set Shell "bash"
Set FontSize 15
Set Width 1500
Set Height 900
Set Padding 24
Set Theme "Catppuccin Mocha"
Set TypingSpeed 35ms

Hide
Type "sh docs/assets/demos/setup-pricing.sh && cd /tmp/countercheck-demo && export PS1='$ ' && clear"
Enter
Sleep 1s
Show

Type "# The agent: 'Fixed the discount bug. All tests pass.' Its own run:"
Enter
Type "python3 -m unittest -q 2>&1 | tail -1"
Enter
Sleep 1.5s
Type "git diff --stat"
Enter
Sleep 2s
Type "countercheck verify --claim 'Fixed the discount bug. All tests pass.'"
Enter
Sleep 4s
Type "# Sent back. The agent fixes the code instead:"
Enter
Type "git checkout -q tests && git apply ../countercheck-demo-fix.patch"
Enter
Type "git diff --stat"
Enter
Sleep 1.5s
Type "countercheck verify --claim 'Fixed the discount bug. All tests pass.' | head -6"
Enter
Sleep 7s
