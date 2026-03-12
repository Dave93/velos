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
Type "velos kill --all 2>/dev/null; sleep 0.5; clear"
Enter
Sleep 1s
Show

# Start processes
Type "velos start examples/node-web-server/velos.toml --name api"
Enter
Sleep 2s

Type "velos start examples/python-fastapi/velos.toml --name worker"
Enter
Sleep 2s

Type "velos start examples/rust-binary/velos.toml --name metrics"
Enter
Sleep 2s

# Show process list (normal)
Type "velos list"
Enter
Sleep 3s

# Show AI-friendly compact output
Type "velos list --ai"
Enter
Sleep 3s

# Smart log summary
Type "velos logs api --summary"
Enter
Sleep 3s

# Show AI crash commands (as if there's a crash record)
Type "velos ai list"
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
