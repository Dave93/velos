# Velos demo GIF
# Run: vhs demo.tape
# Output: demo.gif

Output demo.gif

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 650
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms
Set Padding 20

# Start with a clean slate
Hide
Type `for name in $(velos list --json 2>/dev/null | python3 -c "import sys,json; [print(p['name']) for p in json.loads(sys.stdin.read())]" 2>/dev/null); do velos stop "$name" 2>/dev/null; velos delete "$name" 2>/dev/null; done; sleep 0.5; clear`
Enter
Sleep 2s
Show

# Start processes from example configs
Type "velos start --config examples/node-web-server/velos.toml"
Enter
Sleep 2s

Type "velos start --config examples/python-fastapi/velos.toml"
Enter
Sleep 2s

Type "velos start --config examples/rust-binary/velos.toml"
Enter
Sleep 2s

# Show process list
Type "velos list"
Enter
Sleep 3s

# Show AI-friendly compact output
Type "velos list --ai"
Enter
Sleep 3s

# Smart log summary
Type "velos logs web --summary"
Enter
Sleep 3s

# Open TUI dashboard
Type "velos monit"
Enter
Sleep 5s

Down
Sleep 1s
Down
Sleep 1s
Up
Sleep 1s

Type "q"
Sleep 1s
